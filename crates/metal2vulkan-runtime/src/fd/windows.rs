//! UCRT descriptor ownership. These integers are CRT descriptors, never Win32 HANDLEs.

use std::marker::PhantomData;
use std::mem::ManuallyDrop;

#[link(name = "ucrt")]
unsafe extern "C" {
    fn _close(fd: i32) -> i32;
    fn _getmaxstdio() -> i32;
    fn _dup(fd: i32) -> i32;
    fn _errno() -> *mut i32;
}

/// UCRT's descriptor limit, in the same CRT that owns the descriptors.
pub(crate) fn descriptor_limit() -> u64 {
    // SAFETY: a read-only query with no pointer arguments.
    let limit = unsafe { _getmaxstdio() };
    assert!(limit > 0, "UCRT returned an invalid descriptor limit");
    limit as u64
}

/// One exclusively owned UCRT descriptor.
#[derive(Debug)]
pub struct OwnedFd(i32);

impl OwnedFd {
    /// Duplicates this descriptor, retaining the same open file independently.
    pub fn try_clone(&self) -> std::io::Result<Self> {
        // SAFETY: this owner keeps the descriptor open for the call.
        let fd = unsafe { _dup(self.0) };
        if fd < 0 {
            // UCRT errors are errno values, not Win32 GetLastError values.
            // SAFETY: UCRT returns the calling thread's live errno storage.
            let error = unsafe { *_errno() };
            return Err(std::io::Error::other(format!(
                "UCRT _dup failed: errno {error}"
            )));
        }
        Ok(Self(fd))
    }
}

/// A descriptor borrowed for the lifetime of its owner.
#[derive(Clone, Copy, Debug)]
pub struct BorrowedFd<'a>(i32, PhantomData<&'a OwnedFd>);

/// Borrows an owner's descriptor without transferring or duplicating it.
pub trait AsFd {
    /// The borrowed descriptor.
    fn as_fd(&self) -> BorrowedFd<'_>;
}

/// Reads a descriptor number while its owner remains live.
pub trait AsRawFd {
    /// The UCRT descriptor number.
    fn as_raw_fd(&self) -> i32;
}

/// Takes ownership of an existing UCRT descriptor.
pub trait FromRawFd {
    /// Adopts `fd`.
    ///
    /// # Safety
    /// `fd` is open and exclusively owned by the caller, which transfers that
    /// ownership here. It must not be a Win32 HANDLE cast to an integer.
    unsafe fn from_raw_fd(fd: i32) -> Self;
}

/// Transfers ownership out to another descriptor owner.
pub trait IntoRawFd {
    /// The descriptor, whose next owner is responsible for closing it.
    fn into_raw_fd(self) -> i32;
}

impl AsFd for OwnedFd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        BorrowedFd(self.0, PhantomData)
    }
}

impl AsRawFd for OwnedFd {
    fn as_raw_fd(&self) -> i32 {
        self.0
    }
}

impl AsRawFd for BorrowedFd<'_> {
    fn as_raw_fd(&self) -> i32 {
        self.0
    }
}

impl FromRawFd for OwnedFd {
    unsafe fn from_raw_fd(fd: i32) -> Self {
        Self(fd)
    }
}

impl IntoRawFd for OwnedFd {
    fn into_raw_fd(self) -> i32 {
        ManuallyDrop::new(self).0
    }
}

impl Drop for OwnedFd {
    fn drop(&mut self) {
        // SAFETY: construction or transfer gave this owner the live descriptor.
        // As with std::os::fd::OwnedFd, Drop cannot return a close error.
        unsafe { _close(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::c_void;
    use std::os::windows::io::IntoRawHandle;

    #[link(name = "ucrt")]
    unsafe extern "C" {
        fn _open_osfhandle(handle: isize, flags: i32) -> i32;
        fn _get_osfhandle(fd: i32) -> isize;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetHandleInformation(handle: *mut c_void, flags: *mut u32) -> i32;
        fn GetFileSizeEx(handle: *mut c_void, size: *mut i64) -> i32;
    }

    fn file(name: &str) -> (OwnedFd, std::path::PathBuf, *mut c_void) {
        let path = std::env::temp_dir().join(format!("m2v-fd-{name}-{}", std::process::id()));
        let file = std::fs::File::create(&path).unwrap();
        let handle = file.into_raw_handle();
        // SAFETY: transfer the newly opened handle into a UCRT descriptor.
        let fd = unsafe { _open_osfhandle(handle as isize, 0x8001) };
        assert!(fd >= 0, "UCRT did not accept the test's file handle");
        // SAFETY: this function just made the descriptor and transfers it.
        (unsafe { OwnedFd::from_raw_fd(fd) }, path, handle)
    }

    /// Borrowing retains the handle, and dropping its one owner releases it.
    #[test]
    fn a_descriptor_owns_its_crt_handle() {
        let (owner, path, handle) = file("owner");
        let borrowed = owner.as_fd();
        assert_eq!(borrowed.as_raw_fd(), owner.as_raw_fd());
        let mut flags = 0;
        // SAFETY: the descriptor owns the test handle and flags is writable.
        assert_ne!(unsafe { GetHandleInformation(handle, &mut flags) }, 0);
        drop(owner);
        // SAFETY: querying a closed handle returns a documented failure.
        assert_eq!(unsafe { GetHandleInformation(handle, &mut flags) }, 0);
        std::fs::remove_file(path).unwrap();
    }

    /// A duplicated descriptor remains usable after the original is closed;
    /// transfer to a raw descriptor preserves that ownership until re-adopted.
    #[test]
    fn a_duplicate_and_a_transferred_descriptor_keep_the_file() {
        let (original, path, _) = file("duplicate");
        let duplicate = original.try_clone().unwrap();
        drop(original);
        let raw = duplicate.into_raw_fd();
        // SAFETY: the transferred raw descriptor remains owned by this test.
        let handle = unsafe { _get_osfhandle(raw) } as *mut c_void;
        let mut bytes = -1;
        // SAFETY: a query of the live duplicate into a writable local.
        assert_ne!(unsafe { GetFileSizeEx(handle, &mut bytes) }, 0);
        assert_eq!(bytes, 0);
        // SAFETY: take back the exclusive ownership transferred above.
        drop(unsafe { OwnedFd::from_raw_fd(raw) });
        std::fs::remove_file(path).unwrap();
    }
}
