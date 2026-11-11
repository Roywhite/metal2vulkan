//! Demand-zero Windows pages used by the RAM scheduling tests.

pub(super) fn map(length: u64) -> *mut std::ffi::c_void {
    crate::windows_memory::map_pages(length)
        .unwrap()
        .as_ptr()
        .cast()
}

pub(super) unsafe fn free(at: *mut std::ffi::c_void, _length: u64) {
    // SAFETY: the caller dropped every RAM holder and owns this complete allocation.
    crate::windows_memory::unmap_pages(std::ptr::NonNull::new(at.cast()).unwrap());
}
