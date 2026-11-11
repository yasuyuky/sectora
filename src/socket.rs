use crate::structs::SocketConfig;
use nix::errno::Errno;
use nix::fcntl::{AtFlags, OFlag, openat};
use nix::sys::stat::{Mode, SFlag, fstatat, mkdirat};
use nix::unistd::{UnlinkatFlags, geteuid, unlinkat};
use std::fs::{self, File};
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixDatagram;
use std::path::{Component, Path};

fn trusted_parent(path: &Path) -> io::Result<File> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid socket path"));
    }
    let mut dir = File::open("/")?;
    for component in path.parent().unwrap().components() {
        match component {
            Component::RootDir => continue,
            Component::Normal(name) => {
                check_parent(&dir)?;
                dir = File::from(openat(&dir,
                                        name,
                                        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                                        Mode::empty())?);
            }
            _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid socket parent")),
        }
    }
    check_parent(&dir)?;
    Ok(dir)
}

fn check_parent(dir: &File) -> io::Result<()> {
    let metadata = dir.metadata()?;
    if (metadata.uid() != 0 && metadata.uid() != geteuid().as_raw())
       || (metadata.mode() & 0o022 != 0 && metadata.mode() & 0o1000 == 0)
    {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "untrusted socket parent"));
    }
    Ok(())
}

pub fn bind(config: &SocketConfig) -> io::Result<UnixDatagram> {
    let path = Path::new(&config.socket_dir);
    let parent = trusted_parent(path)?;
    let name = path.file_name().unwrap();
    match mkdirat(&parent, name, Mode::S_IRWXU) {
        Ok(()) | Err(Errno::EEXIST) => {}
        Err(err) => return Err(err.into()),
    }
    let dir = File::from(openat(&parent,
                                name,
                                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                                Mode::empty())?);
    if dir.metadata()?.uid() != geteuid().as_raw() {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "untrusted socket directory owner"));
    }
    // Use the opened directory, and prevent clients from replacing each other's sockets.
    dir.set_permissions(fs::Permissions::from_mode(0o1777))?;

    let path = Path::new(&config.socket_path);
    let parent = trusted_parent(path)?;
    let name = path.file_name().unwrap();
    match fstatat(&parent, name, AtFlags::AT_SYMLINK_NOFOLLOW) {
        Ok(stat) => {
            if stat.st_uid != geteuid().as_raw() || SFlag::from_bits_truncate(stat.st_mode) != SFlag::S_IFSOCK {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "untrusted daemon socket"));
            }
            unlinkat(&parent, name, UnlinkatFlags::NoRemoveDir)?;
        }
        Err(Errno::ENOENT) => {}
        Err(err) => return Err(err.into()),
    }
    let socket = UnixDatagram::bind(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o666))?;
    Ok(socket)
}
