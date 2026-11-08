use nix::dir::Dir;
use nix::errno::Errno;
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::{Mode, mkdirat};
use nix::unistd::{UnlinkatFlags, geteuid, unlinkat};
use std::fs::{File, Metadata};
use std::io::{self, Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};

pub struct Cache {
    dir: File,
}

fn filename(url: &str) -> String {
    let hash: String = openssl::sha::sha256(url.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("{hash}.json")
}

impl Cache {
    pub fn new(path: &Path) -> io::Result<Self> {
        if path.as_os_str().is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty cache directory"));
        }
        let path = std::env::current_dir()?.join(path);
        let uid = geteuid().as_raw();
        let mut dir = File::open("/")?;
        for component in path.components() {
            let name = match component {
                Component::RootDir | Component::CurDir => continue,
                Component::Normal(name) => name,
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "cache directory contains '..'",
                    ));
                }
            };
            let metadata = dir.metadata()?;
            // A trusted sticky parent protects the next component from other users.
            if (metadata.uid() != 0 && metadata.uid() != uid)
                || (metadata.mode() & 0o022 != 0 && metadata.mode() & 0o1000 == 0)
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "untrusted cache parent directory",
                ));
            }
            match mkdirat(&dir, name, Mode::S_IRWXU) {
                Ok(()) | Err(Errno::EEXIST) => {}
                Err(err) => return Err(err.into()),
            }
            dir = File::from(openat(
                &dir,
                name,
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )?);
        }
        let metadata = dir.metadata()?;
        if metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "cache directory must be owned by the daemon user and private (0700)",
            ));
        }
        Ok(Self { dir })
    }

    fn open(&self, url: &str, flags: OFlag) -> io::Result<File> {
        let file = File::from(openat(
            &self.dir,
            filename(url).as_str(),
            flags | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )?);
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.uid() != geteuid().as_raw()
            || metadata.mode() & 0o077 != 0
            || metadata.nlink() != 1
        {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "untrusted cache file"));
        }
        Ok(file)
    }

    pub fn load(&self, url: &str) -> io::Result<(Metadata, String)> {
        let mut file = self.open(url, OFlag::O_RDONLY)?;
        let metadata = file.metadata()?;
        let mut contents = String::default();
        file.read_to_string(&mut contents)?;
        Ok((metadata, contents))
    }

    pub fn store(&self, url: &str, contents: &str) -> io::Result<()> {
        let mut file = self.open(url, OFlag::O_WRONLY | OFlag::O_CREAT)?;
        // Validate ownership and link count before truncating an existing file.
        file.set_len(0)?;
        file.write_all(contents.as_bytes())
    }

    pub fn clear(&self) -> io::Result<()> {
        let mut dir = Dir::openat(
            &self.dir,
            ".",
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC,
            Mode::empty(),
        )?;
        for entry in dir.iter() {
            let entry = entry?;
            let name = entry.file_name();
            let Some(hash) = name.to_bytes().strip_suffix(b".json") else {
                continue;
            };
            if hash.len() == 64 && hash.iter().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b)) {
                // Never recurse into directories or follow links during cleanup.
                unlinkat(&self.dir, name, UnlinkatFlags::NoRemoveDir)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::unistd::{Uid, chown, mkdtemp, mkfifo};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let template = std::env::temp_dir()
                .canonicalize()
                .unwrap()
                .join("sectora-cache-test.XXXXXX");
            Self(mkdtemp(&template).unwrap())
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn stores_updates_and_clears_private_cache() {
        let temp = TempDir::new();
        let path = temp.0.join("cache");
        let cache = Cache::new(&path).unwrap();
        cache.store("https://example.test/users/hunter/keys", "first").unwrap();
        cache
            .store("https://example.test/users/hunter/keys", "updated")
            .unwrap();
        let (metadata, contents) = cache.load("https://example.test/users/hunter/keys").unwrap();
        assert_eq!(contents, "updated");
        assert_eq!(metadata.mode() & 0o777, 0o600);
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o700);
        fs::write(path.join("unrelated"), "keep").unwrap();
        cache.clear().unwrap();
        cache.clear().unwrap();
        assert!(cache.load("https://example.test/users/hunter/keys").is_err());
        assert_eq!(fs::read_to_string(path.join("unrelated")).unwrap(), "keep");
    }

    #[test]
    fn rejects_writable_parents_and_nonprivate_cache_without_repairing_them() {
        let temp = TempDir::new();
        let parent = temp.0.join("shared");
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(Cache::new(&parent.join("cache")).is_err());
        assert!(!parent.join("cache").exists());
        assert!(Cache::new(&parent).is_err());
        assert_eq!(fs::metadata(&parent).unwrap().mode() & 0o777, 0o777);
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o1777)).unwrap();
        assert!(Cache::new(&parent.join("private")).is_ok());
    }

    #[test]
    fn rejects_foreign_owners_even_with_sticky_parents() {
        if !geteuid().is_root() {
            return;
        }
        let temp = TempDir::new();
        let parent = temp.0.join("foreign");
        fs::create_dir(&parent).unwrap();
        chown(&parent, Some(Uid::from_raw(65534)), None).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o1777)).unwrap();
        assert!(Cache::new(&parent.join("cache")).is_err());
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(Cache::new(&parent).is_err());

        let cache = Cache::new(&temp.0.join("cache")).unwrap();
        cache.store("url", "trusted").unwrap();
        chown(
            &temp.0.join("cache").join(filename("url")),
            Some(Uid::from_raw(65534)),
            None,
        )
        .unwrap();
        assert!(cache.load("url").is_err());
        assert!(cache.store("url", "replacement").is_err());
    }

    #[test]
    fn rejects_symlinked_cache_roots_and_ancestors() {
        let temp = TempDir::new();
        symlink(&temp.0, temp.0.join("alias")).unwrap();
        assert!(Cache::new(&temp.0.join("alias")).is_err());
        assert!(Cache::new(&temp.0.join("alias/cache")).is_err());
        assert!(Cache::new(&temp.0.join("../cache")).is_err());
    }

    #[test]
    fn urls_and_glob_characters_cannot_escape_the_cache() {
        let temp = TempDir::new();
        let cache = Cache::new(&temp.0.join("cache[1]*?")).unwrap();
        let urls = [
            "https://example.test/keys",
            "https://example.test/keys?q=1",
            "/../../outside",
            "https://example.test/users/../../../../outside/keys",
            "https://example.test/%2e%2e/keys",
        ];
        for url in urls {
            cache.store(url, url).unwrap();
        }
        for url in urls {
            assert_eq!(cache.load(url).unwrap().1, url);
        }
        fs::write(temp.0.join("outside"), "keep").unwrap();
        cache.clear().unwrap();
        assert_eq!(fs::read_to_string(temp.0.join("outside")).unwrap(), "keep");
        for url in urls {
            assert!(cache.load(url).is_err());
        }
    }

    #[test]
    fn links_cannot_read_overwrite_or_delete_outside_files() {
        let temp = TempDir::new();
        let path = temp.0.join("cache");
        let cache = Cache::new(&path).unwrap();
        let outside = temp.0.join("outside");
        fs::write(&outside, "keep").unwrap();
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).unwrap();
        let entry = path.join(filename("url"));
        for hard_link in [false, true] {
            if hard_link {
                fs::hard_link(&outside, &entry).unwrap();
            } else {
                symlink(&outside, &entry).unwrap();
            }
            assert!(cache.load("url").is_err());
            assert!(cache.store("url", "overwrite").is_err());
            assert_eq!(fs::read_to_string(&outside).unwrap(), "keep");
            cache.clear().unwrap();
            assert_eq!(fs::read_to_string(&outside).unwrap(), "keep");
        }
        symlink(&temp.0, &entry).unwrap();
        cache.clear().unwrap();
        assert_eq!(fs::read_to_string(&outside).unwrap(), "keep");
    }

    #[test]
    fn rejects_fifos_and_directories_without_blocking() {
        let temp = TempDir::new();
        let cache = Cache::new(&temp.0).unwrap();
        let entry = temp.0.join(filename("url"));
        mkfifo(&entry, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();
        assert!(cache.load("url").is_err());
        assert!(cache.store("url", "data").is_err());
        cache.clear().unwrap();
        fs::create_dir(&entry).unwrap();
        assert!(cache.load("url").is_err());
        assert!(cache.store("url", "data").is_err());
        assert!(cache.clear().is_err());
    }

    #[test]
    fn operations_stay_in_the_opened_directory_after_path_replacement() {
        let temp = TempDir::new();
        let path = temp.0.join("cache");
        let cache = Cache::new(&path).unwrap();
        let original = temp.0.join("original");
        fs::rename(&path, &original).unwrap();
        symlink(&temp.0, &path).unwrap();
        cache.store("url", "data").unwrap();
        assert_eq!(cache.load("url").unwrap().1, "data");
        assert!(!temp.0.join(filename("url")).exists());
        assert!(original.join(filename("url")).exists());
        cache.clear().unwrap();
        assert!(!original.join(filename("url")).exists());
    }
}
