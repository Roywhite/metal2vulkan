//! Real anonymous mappings used by the RAM scheduling tests.

pub(super) fn map(length: u64) -> *mut std::ffi::c_void {
    // SAFETY: request a fresh writable mapping with the test's original flags.
    let at = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            length as usize,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANON | libc::MAP_NORESERVE,
            -1,
            0,
        )
    };
    assert_ne!(at, libc::MAP_FAILED);
    at
}

pub(super) unsafe fn free(at: *mut std::ffi::c_void, length: u64) {
    // SAFETY: the caller has dropped every holder of this test mapping.
    assert_eq!(unsafe { libc::munmap(at, length as usize) }, 0);
}
