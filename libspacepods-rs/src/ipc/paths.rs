//! Where the daemon socket lives and how its directory is secured.
//!
//! The socket used to live at `/tmp/spacepods.sock` with mode `0666`, which let any
//! local user drive the earbuds and let another user squat the path. It now lives in
//! the per-user runtime directory (`$XDG_RUNTIME_DIR/spacepods/`), which is `0700` and
//! owned by the user. Permission and ownership checks happen here so both the daemon
//! and every client resolve the same path the same way.

use crate::{Error, Result};
use std::fs::{self, DirBuilder};
use std::io::ErrorKind;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Environment variable that overrides the socket path (tests, unusual setups).
pub const SOCKET_ENV: &str = "SPACEPODS_SOCKET";

/// Name of the directory under the runtime dir that holds the socket.
const RUNTIME_SUBDIR: &str = "spacepods";
/// Name of the socket file.
const SOCKET_FILE: &str = "spacepods.sock";

/// The current user's uid.
pub fn current_uid() -> u32 {
    // SAFETY: getuid() has no preconditions and cannot fail.
    unsafe { libc::getuid() }
}

/// Directory that holds the daemon socket.
///
/// Uses `$XDG_RUNTIME_DIR` when set, otherwise `/run/user/<uid>`.
pub fn runtime_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", current_uid())));
    base.join(RUNTIME_SUBDIR)
}

/// The socket path used when no explicit path is given.
///
/// `SPACEPODS_SOCKET` wins if set. On Windows this returns the named pipe name, which
/// is kept only so the crate still compiles there; the Unix socket server is Unix-only.
pub fn default_socket_path() -> PathBuf {
    if let Some(p) = std::env::var_os(SOCKET_ENV).filter(|v| !v.is_empty()) {
        return PathBuf::from(p);
    }
    #[cfg(unix)]
    {
        runtime_dir().join(SOCKET_FILE)
    }
    #[cfg(windows)]
    {
        PathBuf::from(r"\\.\pipe\spacepods")
    }
}

/// Create the socket directory if needed and check that it is safe to use.
///
/// The directory must be a real directory (not a symlink), owned by the current user.
/// Its permissions are tightened to `0700` if they are looser.
pub fn ensure_runtime_dir(dir: &Path) -> Result<()> {
    match DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
        Err(e) => return Err(Error::Io(e)),
    }

    let meta = fs::symlink_metadata(dir)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(Error::UnsafeSocketDir {
            path: dir.to_path_buf(),
            reason: "not a real directory".into(),
        });
    }
    if meta.uid() != current_uid() {
        return Err(Error::UnsafeSocketDir {
            path: dir.to_path_buf(),
            reason: "owned by another user".into(),
        });
    }
    if meta.permissions().mode() & 0o077 != 0 {
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Set the socket file to `0600` so only the owner can connect.
pub fn secure_socket_file(path: &Path) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

/// Whether a socket file at `path` belongs to the current user.
pub fn is_owned_by_current_user(path: &Path) -> Result<bool> {
    Ok(fs::symlink_metadata(path)?.uid() == current_uid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_dir_is_created_0700() {
        let base = std::env::temp_dir().join(format!("spacepods-test-{}", std::process::id()));
        let dir = base.join("spacepods");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();

        ensure_runtime_dir(&dir).unwrap();
        let mode = fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);

        // A looser mode is tightened, not accepted.
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        ensure_runtime_dir(&dir).unwrap();
        let mode = fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn symlinked_runtime_dir_is_refused() {
        let base = std::env::temp_dir().join(format!("spacepods-link-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let target = base.join("real");
        fs::create_dir_all(&target).unwrap();
        let link = base.join("spacepods");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(matches!(
            ensure_runtime_dir(&link),
            Err(Error::UnsafeSocketDir { .. })
        ));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn env_override_wins() {
        // Set and clear inside one test; no other test reads this variable.
        // SAFETY: this crate's tests do not read SOCKET_ENV concurrently.
        unsafe { std::env::set_var(SOCKET_ENV, "/custom/path.sock") };
        assert_eq!(default_socket_path(), PathBuf::from("/custom/path.sock"));
        unsafe { std::env::remove_var(SOCKET_ENV) };
    }
}
