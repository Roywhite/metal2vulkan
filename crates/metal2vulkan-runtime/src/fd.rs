//! Owned file descriptors used by Vulkan's optional FD extensions.
//!
//! Unix uses the standard descriptor owners. Windows descriptors belong to
//! the UCRT, shared with a GNU/UCRT host. A Windows Vulkan driver still has
//! to advertise the relevant FD extension before any Vulkan FD call is made.

#[cfg(unix)]
pub use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd};

#[cfg(windows)]
pub(crate) use windows::descriptor_limit;

/// The process's current descriptor limit used to bound retained imports.
#[cfg(unix)]
pub(crate) fn descriptor_limit() -> u64 {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: limit is a writable rlimit and the query changes no process limit.
    match unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } {
        0 => limit.rlim_cur,
        _ => 0,
    }
}
