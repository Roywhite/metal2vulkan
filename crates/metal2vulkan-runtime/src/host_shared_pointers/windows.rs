//! Shared section pages used to qualify Windows host-pointer imports.

use std::ffi::c_void;
use std::ptr::NonNull;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileMappingW(
        file: *mut c_void,
        attributes: *const c_void,
        protect: u32,
        high: u32,
        low: u32,
        name: *const u16,
    ) -> *mut c_void;
    fn MapViewOfFile(
        mapping: *mut c_void,
        access: u32,
        high: u32,
        low: u32,
        length: usize,
    ) -> *mut c_void;
    fn UnmapViewOfFile(address: *const c_void) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

/// A writable shared section view, released after the qualification's GPU import.
pub(super) struct SharedPages {
    pub(super) at: NonNull<u8>,
}

impl SharedPages {
    /// Makes real section-backed pages, matching the host's shared RAM kind.
    pub(super) fn new(length: u64) -> Option<Self> {
        let bytes = usize::try_from(length).ok()?;
        // SAFETY: INVALID_HANDLE_VALUE requests an unnamed page-file section;
        // all optional pointers are null and PAGE_READWRITE grants the test's access.
        let section = unsafe {
            CreateFileMappingW(
                -1isize as *mut c_void,
                std::ptr::null(),
                4,
                (length >> 32) as u32,
                length as u32,
                std::ptr::null(),
            )
        };
        if section.is_null() {
            return None;
        }
        // SAFETY: the section owns at least bytes bytes. FILE_MAP_ALL_ACCESS
        // maps readable/writable pages and retains its own reference to the section.
        let mapped = unsafe { MapViewOfFile(section, 0x000f001f, 0, 0, bytes) };
        // SAFETY: the new view retains the section independently of this handle.
        assert_ne!(unsafe { CloseHandle(section) }, 0, "section close failed");
        NonNull::new(mapped.cast()).map(|at| Self { at })
    }
}

impl Drop for SharedPages {
    fn drop(&mut self) {
        // SAFETY: this owner holds the live view and the import has already ended.
        assert_ne!(
            unsafe { UnmapViewOfFile(self.at.as_ptr().cast()) },
            0,
            "shared-pointer qualification view could not be unmapped"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The qualification uses actual zero-filled shared memory with writable
    /// pages through the last byte of the requested range.
    #[test]
    fn qualification_pages_are_zeroed_and_writable() {
        let memory = SharedPages::new(8192).unwrap();
        // SAFETY: the owned view holds all 8192 writable bytes.
        let bytes = unsafe { std::slice::from_raw_parts_mut(memory.at.as_ptr(), 8192) };
        assert!(bytes.iter().all(|&value| value == 0));
        bytes[0] = 0x37;
        bytes[8191] = 0x5a;
        assert_eq!((bytes[0], bytes[8191]), (0x37, 0x5a));
    }
}
