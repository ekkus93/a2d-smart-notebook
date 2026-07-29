//! Platform-specific primitives required by the v0.1 asset filesystem commit contract.
//!
//! Android app-private storage and ordinary Linux local filesystems are the only currently
//! validated targets. Apple targets deliberately fail with `Unsupported` until their no-replace,
//! directory-synchronization, data-protection, and hardware-flush behavior is validated. Callers
//! must never replace these errors with `rename` or a flush-only fallback.

use std::io;
use std::path::Path;

#[cfg(any(target_os = "android", target_os = "linux"))]
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
