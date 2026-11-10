use std::{io, path::Path};

use nix::{errno::Errno, sys::statvfs::statvfs};

/// Free bytes on the filesystem that holds `path`, or its nearest ancestor that exists.
/// The path resolves in the calling thread's mount namespace, so call this from inside
/// `spawn_in_host_ns` to measure the host.
pub fn available_space(path: &Path) -> io::Result<u64> {
    for candidate in path.ancestors() {
        match statvfs(candidate) {
            Ok(stat) => return Ok(stat.blocks_available() * stat.fragment_size()),
            Err(Errno::ENOENT) => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(io::ErrorKind::NotFound.into())
}

#[cfg(test)]
mod tests {
    use super::available_space;
    use std::path::Path;

    #[test]
    fn missing_path_uses_nearest_existing_ancestor() {
        let dir = std::env::temp_dir();
        let missing = dir.join("edera-check-no-such-dir/nested");
        assert!(!missing.exists());

        let stat = nix::sys::statvfs::statvfs(&dir).unwrap();
        // Free space moves while the test runs, so compare against the filesystem size.
        let total = stat.blocks() * stat.fragment_size();
        let avail = available_space(&missing).unwrap();
        assert!(avail > 0 && avail <= total);
    }

    #[test]
    fn relative_path_with_no_existing_ancestor_is_not_found() {
        let err = available_space(Path::new("edera-check-no-such-relative-path")).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }
}
