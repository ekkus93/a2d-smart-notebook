//! Platform-specific primitives required by the v0.1 asset filesystem commit contract.
//!
//! Android app-private storage and ordinary Linux local filesystems are the only currently
//! validated targets. Apple targets deliberately fail with `Unsupported` until their no-replace,
//! directory-synchronization, data-protection, and hardware-flush behavior is validated. Callers
//! must never replace these errors with a replacing rename or a flush-only fallback.

use std::io;
use std::path::Path;

#[cfg(target_os = "android")]
pub(super) fn finalize_no_replace(temp_path: &Path, final_path: &Path) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let temp = CString::new(temp_path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "temp path contains NUL"))?;
    let final_path = CString::new(final_path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "final path contains NUL"))?;
    // Android's app-private filesystem can reject hard-link creation after the immutable-original
    // permission bit has been applied. renameat2(RENAME_NOREPLACE) provides the same required
    // same-filesystem atomic no-replace finalization without creating a second hard link.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            temp.as_ptr(),
            libc::AT_FDCWD,
            final_path.as_ptr(),
            libc::RENAME_NOREPLACE as u32,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
pub(super) fn finalize_no_replace(temp_path: &Path, final_path: &Path) -> io::Result<()> {
    std::fs::hard_link(temp_path, final_path)
}

#[cfg(not(any(target_os = "android", target_os = "linux")))]
pub(super) fn finalize_no_replace(_temp_path: &Path, _final_path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "asset no-replace finalization is validated only for Android and Linux local filesystems",
    ))
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub(super) fn sync_directory(path: &Path) -> io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

#[cfg(not(any(target_os = "android", target_os = "linux")))]
pub(super) fn sync_directory(_path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "asset directory synchronization is validated only for Android and Linux local filesystems",
    ))
}
