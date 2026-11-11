//! UCRT process-exit registration for the shared runtime drain.

#[link(name = "ucrt")]
unsafe extern "C" {
    fn atexit(callback: extern "C" fn()) -> i32;
}

/// Registers the shared bounded shutdown before the CRT destroys driver state.
pub(super) fn install() {
    // SAFETY: callback has the CRT's ABI, lives for the module's lifetime,
    // and parent registration calls this once for this process.
    assert_eq!(
        unsafe { atexit(exit_handler) },
        0,
        "UCRT atexit registration failed"
    );
}

extern "C" fn exit_handler() {
    super::hook(
        &super::GUARD,
        &super::IN_PROCESS,
        super::REGISTERED.load(std::sync::atomic::Ordering::SeqCst),
        std::process::id(),
        super::OWN.with(std::cell::Cell::get),
        super::BOUND,
        super::MAKING_BOUND,
    );
}
