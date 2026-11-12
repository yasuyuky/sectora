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

#[cfg(test)]
mod tests {
    use super::*;
    use nix::unistd::{Uid, chown, mkdtemp};
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use std::time::Duration;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let parent = std::env::temp_dir().canonicalize().unwrap();
            Self(mkdtemp(&parent.join("socket.XXXXXX")).unwrap())
        }

        fn config(&self) -> SocketConfig {
            SocketConfig { socket_dir: self.0.join("clients").to_str().unwrap().to_owned(),
                           socket_path: self.0.join("daemon").to_str().unwrap().to_owned() }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); }
    }

    #[test]
    fn creates_sticky_directory_and_exchanges_datagrams_after_restart() {
        let temp = TempDir::new();
        let config = temp.config();
        drop(bind(&config).unwrap());
        // Reproduce the legacy mode to verify startup restores the sticky bit.
        // skipcq: RS-A1001
        fs::set_permissions(&config.socket_dir, fs::Permissions::from_mode(0o777)).unwrap();
        let server = bind(&config).unwrap();
        let client = UnixDatagram::bind(Path::new(&config.socket_dir).join("client")).unwrap();
        server.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        client.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        client.connect(&config.socket_path).unwrap();
        client.send(b"request").unwrap();
        let mut buf = [0; 16];
        let (count, sender) = server.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..count], b"request");
        server.send_to(b"reply", sender.as_pathname().unwrap()).unwrap();
        let count = client.recv(&mut buf).unwrap();
        assert_eq!(&buf[..count], b"reply");
        assert_eq!(fs::metadata(&config.socket_dir).unwrap().mode() & 0o7777, 0o1777);
        assert_eq!(fs::metadata(&config.socket_path).unwrap().mode() & 0o777, 0o666);
    }

    #[test]
    fn rejects_symlinked_directory_without_changing_its_target() {
        let temp = TempDir::new();
        let config = temp.config();
        let outside = temp.0.join("outside");
        // The parent exists; unexpected pre-existing targets should fail this test.
        // skipcq: RS-W1032
        fs::create_dir(&outside).unwrap();
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o700)).unwrap();
        symlink(&outside, &config.socket_dir).unwrap();
        assert!(bind(&config).is_err());
        assert_eq!(fs::metadata(&outside).unwrap().mode() & 0o777, 0o700);
        assert!(!Path::new(&config.socket_path).exists());
        fs::remove_file(&config.socket_dir).unwrap();
        fs::write(&config.socket_dir, "keep").unwrap();
        assert!(bind(&config).is_err());
        assert_eq!(fs::read_to_string(&config.socket_dir).unwrap(), "keep");
    }

    #[test]
    fn rejects_untrusted_parents_and_owners() {
        // Keep unsafe fixtures behind a private directory, including during ownership changes.
        let outer = TempDir::new();
        let temp = TempDir(mkdtemp(&outer.0.join("parent.XXXXXX")).unwrap());
        let config = temp.config();
        // Verify rejection of a world-writable parent without the sticky bit.
        // skipcq: RS-A1001
        fs::set_permissions(&temp.0, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(bind(&config).is_err());
        assert!(!Path::new(&config.socket_dir).exists());
        fs::set_permissions(&temp.0, fs::Permissions::from_mode(0o1777)).unwrap();
        drop(bind(&config).unwrap());
        fs::remove_file(&config.socket_path).unwrap();

        if geteuid().is_root() {
            chown(Path::new(&config.socket_dir), Some(Uid::from_raw(65534)), None).unwrap();
            assert!(bind(&config).is_err());
            assert_eq!(fs::metadata(&config.socket_dir).unwrap().uid(), 65534);
            chown(&temp.0, Some(Uid::from_raw(65534)), None).unwrap();
            assert!(bind(&config).is_err());
            chown(&temp.0, Some(geteuid()), None).unwrap();
            chown(Path::new(&config.socket_dir), Some(geteuid()), None).unwrap();
            drop(bind(&config).unwrap());
            chown(Path::new(&config.socket_path), Some(Uid::from_raw(65534)), None).unwrap();
            assert!(bind(&config).is_err());
            assert_eq!(fs::metadata(&config.socket_path).unwrap().uid(), 65534);
        }
    }

    #[test]
    fn rejects_symlinked_parent_and_preserves_unexpected_socket_entries() {
        let temp = TempDir::new();
        let config = temp.config();
        let alias = temp.0.join("alias");
        symlink(&temp.0, &alias).unwrap();
        let mut aliased = config.clone();
        aliased.socket_dir = alias.join("clients").to_str().unwrap().to_owned();
        assert!(bind(&aliased).is_err());

        fs::write(&config.socket_path, "keep").unwrap();
        assert!(bind(&config).is_err());
        assert_eq!(fs::read_to_string(&config.socket_path).unwrap(), "keep");
        fs::remove_file(&config.socket_path).unwrap();
        symlink(temp.0.join("missing"), &config.socket_path).unwrap();
        assert!(bind(&config).is_err());
        assert!(fs::symlink_metadata(&config.socket_path).unwrap()
                                                         .file_type()
                                                         .is_symlink());
    }
}
