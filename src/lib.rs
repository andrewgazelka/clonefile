//! Thin Rust wrapper around macOS `clonefile(2)`, `clonefileat(2)`, and
//! `fclonefileat(2)`.
//!
//! These syscalls ask APFS to create a copy-on-write clone: the destination
//! shares data blocks with the source until one side writes, at which point
//! only the modified blocks diverge. Cloning a 5 GiB directory takes
//! milliseconds and zero additional disk until you mutate it.
//!
//! # Caveats (from `man 2 clonefile`)
//!
//! - **Apple explicitly discourages cloning directories.** The man page says
//!   "the use of clonefile(2) to clone directory hierarchies is strongly
//!   discouraged. Use copyfile(3) instead." In practice this is fine for the
//!   short-lived agent sandbox use case this crate targets — see the README.
//! - APFS only. `ENOTSUP` on other filesystems, `EXDEV` across volumes.
//! - Destination must not already exist (`EEXIST` otherwise).
//! - Subsequent overwrites of shared blocks can fail with `ENOSPC` because
//!   storage isn't pre-allocated.
//! - `setuid`/`setgid` bits are stripped from regular files.
//! - Ownership is reset unless the caller is privileged (or you pass
//!   `Flags::NO_OWNER_COPY`).
//! - POSIX-conforming applications cannot use this — it's Darwin-only.

#![cfg(target_os = "macos")]

use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::path::Path;

bitflags::bitflags! {
    /// Flags for the `clonefile` family. Values mirror `<sys/clonefile.h>`.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub struct Flags: u32 {
        const NOFOLLOW        = 0x0001;
        const NO_OWNER_COPY   = 0x0002;
        const ACL             = 0x0004;
        const NOFOLLOW_ANY    = 0x0008;
        const RESOLVE_BENEATH = 0x0010;
    }
}

unsafe extern "C" {
    fn clonefile(src: *const libc::c_char, dst: *const libc::c_char, flags: u32) -> libc::c_int;
    fn clonefileat(
        src_dirfd: libc::c_int,
        src: *const libc::c_char,
        dst_dirfd: libc::c_int,
        dst: *const libc::c_char,
        flags: u32,
    ) -> libc::c_int;
    fn fclonefileat(
        srcfd: libc::c_int,
        dst_dirfd: libc::c_int,
        dst: *const libc::c_char,
        flags: u32,
    ) -> libc::c_int;
}

fn cpath(p: &Path) -> io::Result<CString> {
    use std::os::unix::ffi::OsStrExt;
    CString::new(p.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL byte"))
}

fn check(rc: libc::c_int) -> io::Result<()> {
    if rc == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

/// Clone `src` to `dst`. If `src` is a directory, the hierarchy is cloned
/// recursively (see crate-level caveats).
pub fn clone<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q, flags: Flags) -> io::Result<()> {
    let src = cpath(src.as_ref())?;
    let dst = cpath(dst.as_ref())?;
    // SAFETY: pointers are valid for the duration of the call.
    check(unsafe { clonefile(src.as_ptr(), dst.as_ptr(), flags.bits()) })
}

/// `clonefileat(2)`. Pass `None` for `AT_FDCWD`.
pub fn clone_at<P: AsRef<Path>, Q: AsRef<Path>>(
    src_dirfd: Option<RawFd>,
    src: P,
    dst_dirfd: Option<RawFd>,
    dst: Q,
    flags: Flags,
) -> io::Result<()> {
    let src = cpath(src.as_ref())?;
    let dst = cpath(dst.as_ref())?;
    let sfd = src_dirfd.unwrap_or(libc::AT_FDCWD);
    let dfd = dst_dirfd.unwrap_or(libc::AT_FDCWD);
    // SAFETY: pointers are valid for the duration of the call.
    check(unsafe { clonefileat(sfd, src.as_ptr(), dfd, dst.as_ptr(), flags.bits()) })
}

/// `fclonefileat(2)`. Source is identified by an open file descriptor.
pub fn fclone_at<F: AsRawFd, Q: AsRef<Path>>(
    src: &F,
    dst_dirfd: Option<RawFd>,
    dst: Q,
    flags: Flags,
) -> io::Result<()> {
    let dst = cpath(dst.as_ref())?;
    let dfd = dst_dirfd.unwrap_or(libc::AT_FDCWD);
    // SAFETY: borrow of `src` keeps the fd alive; `dst` pointer is valid.
    check(unsafe { fclonefileat(src.as_raw_fd(), dfd, dst.as_ptr(), flags.bits()) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmpdir() -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!("clonefile-test-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn clones_a_regular_file() {
        let dir = tmpdir();
        let src = dir.join("a");
        let dst = dir.join("b");
        fs::write(&src, b"hello").unwrap();
        clone(&src, &dst, Flags::empty()).unwrap();
        assert_eq!(fs::read(&dst).unwrap(), b"hello");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clones_a_directory_tree() {
        let dir = tmpdir();
        let src = dir.join("src");
        let dst = dir.join("dst");
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::write(src.join("nested/file"), b"x").unwrap();
        clone(&src, &dst, Flags::empty()).unwrap();
        assert_eq!(fs::read(dst.join("nested/file")).unwrap(), b"x");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn destination_must_not_exist() {
        let dir = tmpdir();
        let src = dir.join("a");
        let dst = dir.join("b");
        fs::write(&src, b"").unwrap();
        fs::write(&dst, b"").unwrap();
        let err = clone(&src, &dst, Flags::empty()).unwrap_err();
        assert_eq!(err.raw_os_error(), Some(libc::EEXIST));
        fs::remove_dir_all(&dir).ok();
    }
}
