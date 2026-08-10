// SPDX-License-Identifier: GPL-3.0-or-later
//! Unix implementations (design-02 §3.2 step 3, §3.4 step 6).
//!
//! **UNVERIFIED BY COMPILATION** - see `platform/mod.rs`'s module doc
//! comment. Every `libc` function/constant referenced here was confirmed to
//! exist with this exact signature by reading the vendored `libc` 0.2.189
//! crate source for this task (`renameat2` and `RENAME_NOREPLACE` in
//! `unix/linux_like/linux/mod.rs`; `renamex_np` and `RENAME_EXCL` in
//! `unix/bsd/apple/mod.rs`; `statvfs`/`setsid`/`kill`/`killpg` in
//! `unix/mod.rs`), not guessed - but no Unix Rust target is available on
//! this Windows development machine to actually build or run it.

use std::ffi::{CString, OsStr};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::RenameError;

fn to_cstring(s: &OsStr) -> io::Result<CString> {
    CString::new(s.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains a NUL byte"))
}

/// No-replace atomic rename (design-02 §3.4 step 6): Linux `renameat2`
/// with `RENAME_NOREPLACE`; macOS `renamex_np` with `RENAME_EXCL`; any
/// other Unix falls back to `link` + `unlink` of the source, "which is also
/// atomic-no-replace" (design-02's own documented fallback for platforms
/// where neither syscall exists).
pub(crate) fn rename_no_replace(src: &Path, dst: &Path) -> Result<(), RenameError> {
    #[cfg(target_os = "linux")]
    {
        rename_no_replace_linux(src, dst)
    }
    #[cfg(target_os = "macos")]
    {
        rename_no_replace_macos(src, dst)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        rename_no_replace_link_fallback(src, dst)
    }
}

#[cfg(target_os = "linux")]
fn rename_no_replace_linux(src: &Path, dst: &Path) -> Result<(), RenameError> {
    let csrc = to_cstring(src.as_os_str()).map_err(RenameError::Io)?;
    let cdst = to_cstring(dst.as_os_str()).map_err(RenameError::Io)?;
    // SAFETY: both C strings are valid, NUL-terminated, and outlive the
    // call. AT_FDCWD + absolute paths means the dirfd arguments are
    // ignored by the kernel (both `src`/`dst` are always absolute in this
    // codebase - see engine::command_compiler's T-4 invariant, which every
    // publication-time path already satisfies).
    let ret = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            csrc.as_ptr(),
            libc::AT_FDCWD,
            cdst.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if ret == 0 {
        Ok(())
    } else {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EEXIST) {
            Err(RenameError::AlreadyExists)
        } else {
            Err(RenameError::Io(err))
        }
    }
}

#[cfg(target_os = "macos")]
fn rename_no_replace_macos(src: &Path, dst: &Path) -> Result<(), RenameError> {
    let csrc = to_cstring(src.as_os_str()).map_err(RenameError::Io)?;
    let cdst = to_cstring(dst.as_os_str()).map_err(RenameError::Io)?;
    // SAFETY: both C strings are valid, NUL-terminated, and outlive the
    // call.
    let ret = unsafe { libc::renamex_np(csrc.as_ptr(), cdst.as_ptr(), libc::RENAME_EXCL) };
    if ret == 0 {
        Ok(())
    } else {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EEXIST) {
            Err(RenameError::AlreadyExists)
        } else {
            Err(RenameError::Io(err))
        }
    }
}

/// design-02 §3.4 step 6's documented fallback ("where the syscall is
/// unsupported (exotic FS): `link(temp, final)` + `unlink(temp)`, which is
/// also atomic-no-replace"): `link()` fails with `EEXIST` if `dst` already
/// exists, so the create-the-new-name half is already no-replace; only
/// after that succeeds do we remove the old name.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn rename_no_replace_link_fallback(src: &Path, dst: &Path) -> Result<(), RenameError> {
    match std::fs::hard_link(src, dst) {
        Ok(()) => {
            std::fs::remove_file(src).map_err(RenameError::Io)?;
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(RenameError::AlreadyExists),
        Err(e) => Err(RenameError::Io(e)),
    }
}

/// `statvfs`-based free-space query (design-02 §3.2 step 8's Unix half:
/// "`GetDiskFreeSpaceExW` / `statvfs`"). Uses `f_bavail` (blocks available
/// to an unprivileged caller, mirroring Windows'
/// `lpFreeBytesAvailableToCaller`) times the fragment size `f_frsize`.
pub(crate) fn disk_free_bytes(dir: &Path) -> io::Result<u64> {
    let cdir = to_cstring(dir.as_os_str())?;
    // SAFETY: `stat_buf` is zero-initialized and large enough (it is the
    // real `libc::statvfs` type); `statvfs` only reads `cdir` (valid,
    // NUL-terminated) and writes `stat_buf`.
    unsafe {
        let mut stat_buf: libc::statvfs = std::mem::zeroed();
        let ret = libc::statvfs(cdir.as_ptr(), &mut stat_buf);
        if ret != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok((stat_buf.f_frsize as u64).saturating_mul(stat_buf.f_bavail as u64))
    }
}

/// Unix paths are arbitrary byte sequences interpreted as UTF-8 by
/// convention, not translated through a fixed "active code page" the way
/// Windows' ANSI APIs are - design-02's D-3 rationale is Windows-specific
/// (`fopen`/`access` under the process ACP, T-10); "macOS is unaffected -
/// paths are UTF-8 natively" per design-02 §7 item 1, and the same holds
/// for Linux in any UTF-8 locale, which is the overwhelming default today.
/// Always representable here; the real gate for non-UTF-8-locale Unix
/// systems (a narrow, decreasingly common case) is left as a known gap
/// rather than guessed at.
pub(crate) fn is_acp_representable(_s: &OsStr) -> bool {
    true
}

/// `fsync` on the temp file (design-02 §3.4 step 6 Unix branch: "`fsync
/// (temp_fd)`").
pub(crate) fn sync_file(path: &Path) -> io::Result<()> {
    let file = std::fs::OpenOptions::new().write(true).open(path)?;
    file.sync_all()
}

/// `fsync` the containing directory's fd (design-02 §3.4 step 6 Unix
/// branch: "then `fsync` the directory fd") so the new directory entry
/// itself is durable, not just the file content.
pub(crate) fn sync_dir(dir: &Path) -> io::Result<()> {
    let dir_file = std::fs::File::open(dir)?;
    dir_file.sync_all()
}
