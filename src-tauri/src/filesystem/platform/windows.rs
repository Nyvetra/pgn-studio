// SPDX-License-Identifier: GPL-3.0-or-later
//! Windows implementations (design-02 §3.2 step 3, §3.4 step 6). Compiled
//! and `cargo check`/`clippy`-verified on this development machine.

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS;
use windows_sys::Win32::Globalization::{WideCharToMultiByte, WC_NO_BEST_FIT_CHARS};
use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, MoveFileExW};

use super::RenameError;

fn to_wide_null(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// `MoveFileExW(src, dst, 0)` - **without** `MOVEFILE_REPLACE_EXISTING`
/// (design-02 §3.4 step 6: "Rust's `std::fs::rename` cannot be used here:
/// it always passes `MOVEFILE_REPLACE_EXISTING`"). A concurrently created
/// destination yields `ERROR_ALREADY_EXISTS`, mapped to
/// [`RenameError::AlreadyExists`] rather than a generic I/O error so
/// callers can map it to `OUTPUT_EXISTS` specifically.
pub(crate) fn rename_no_replace(src: &Path, dst: &Path) -> Result<(), RenameError> {
    let wsrc = to_wide_null(src.as_os_str());
    let wdst = to_wide_null(dst.as_os_str());
    // SAFETY: both buffers are NUL-terminated UTF-16 and outlive the call
    // (they are not dropped until this function returns); MoveFileExW does
    // not retain either pointer past the call.
    let ok = unsafe { MoveFileExW(wsrc.as_ptr(), wdst.as_ptr(), 0) };
    if ok != 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(ERROR_ALREADY_EXISTS as i32) {
        Err(RenameError::AlreadyExists)
    } else {
        Err(RenameError::Io(err))
    }
}

/// `GetDiskFreeSpaceExW`'s `lpFreeBytesAvailableToCaller` - the bytes this
/// process could actually write given quotas, which is the number
/// design-02 §3.2 step 8's disk-space check needs (not the raw
/// total-free-on-volume figure, which can overstate what is usable).
pub(crate) fn disk_free_bytes(dir: &Path) -> io::Result<u64> {
    let wdir = to_wide_null(dir.as_os_str());
    let mut free_available_to_caller: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut total_free_bytes: u64 = 0;
    // SAFETY: all three out-pointers are valid, aligned, writable u64
    // locals for the duration of the call.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wdir.as_ptr(),
            &mut free_available_to_caller,
            &mut total_bytes,
            &mut total_free_bytes,
        )
    };
    if ok != 0 {
        Ok(free_available_to_caller)
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Whether `s` round-trips losslessly through the active code page
/// (design-02 §3.2 step 3, Decision D-3): probes with `WideCharToMultiByte`
/// under `WC_NO_BEST_FIT_CHARS` (so the OS cannot silently substitute a
/// "close enough" ANSI character) and checks `lpUsedDefaultChar` - if the
/// OS had to fall back to the default replacement character even once,
/// the path is not representable and must not be handed to the engine
/// (which uses ANSI `fopen`/`access`, T-10).
pub(crate) fn is_acp_representable(s: &OsStr) -> bool {
    const CP_ACP: u32 = 0;
    let wide = to_wide_null(s);
    let mut used_default_char: i32 = 0;
    // SAFETY: `wide` is NUL-terminated and outlives the call;
    // `lpmultibytestr`/`cbmultibyte` are null/0 (size-query mode, valid per
    // the WideCharToMultiByte contract); `lpdefaultchar` null is valid
    // (uses the system default); `used_default_char` is a valid `*mut i32`.
    let len = unsafe {
        WideCharToMultiByte(
            CP_ACP,
            WC_NO_BEST_FIT_CHARS,
            wide.as_ptr(),
            -1,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
            &mut used_default_char,
        )
    };
    len != 0 && used_default_char == 0
}

/// Flushes a file's content to durable storage before its name is changed
/// (design-02 §3.4 step 6: "we additionally `FlushFileBuffers` on the temp
/// file handle before rename so the *content* is durable before the name
/// flips"). `File::sync_all` calls `FlushFileBuffers` on Windows - no raw
/// FFI needed for this half.
pub(crate) fn sync_file(path: &Path) -> io::Result<()> {
    let file = std::fs::OpenOptions::new().write(true).open(path)?;
    file.sync_all()
}

/// Design-02 §3.4 step 6's directory-fsync instruction is written only for
/// the Unix branch ("then `fsync` the directory fd"); the Windows branch's
/// own text stops at the temp-file `FlushFileBuffers` above. No-op here,
/// matching the spec rather than adding an unrequested guarantee.
pub(crate) fn sync_dir(_dir: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_no_replace_succeeds_when_destination_absent() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.pgn");
        let dst = dir.path().join("dst.pgn");
        std::fs::write(&src, b"hello").unwrap();
        rename_no_replace(&src, &dst).unwrap();
        assert!(!src.exists());
        assert_eq!(std::fs::read(&dst).unwrap(), b"hello");
    }

    #[test]
    fn rename_no_replace_fails_without_touching_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.pgn");
        let dst = dir.path().join("dst.pgn");
        std::fs::write(&src, b"new content").unwrap();
        std::fs::write(&dst, b"original content - must survive").unwrap();
        let err = rename_no_replace(&src, &dst).unwrap_err();
        assert!(matches!(err, RenameError::AlreadyExists));
        // Binding safety property: neither file was touched.
        assert_eq!(
            std::fs::read(&dst).unwrap(),
            b"original content - must survive"
        );
        assert_eq!(std::fs::read(&src).unwrap(), b"new content");
    }

    #[test]
    fn disk_free_bytes_returns_a_plausible_value() {
        let dir = tempfile::tempdir().unwrap();
        let free = disk_free_bytes(dir.path()).unwrap();
        assert!(free > 0, "a real, non-full volume must report free bytes");
    }

    #[test]
    fn acp_representable_true_for_plain_ascii() {
        assert!(is_acp_representable(OsStr::new(r"C:\games\out.pgn")));
    }
}
