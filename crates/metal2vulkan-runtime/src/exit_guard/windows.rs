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
    // The latest registration closes admission and drains once. Earlier UCRT callbacks
    // must not each wait for the same worker after shutdown has already begun.
    if super::GUARD.is_closed() {
        return;
    }
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

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::sync::atomic::Ordering;
    use std::sync::Barrier;
    use std::time::Duration;

    #[test]
    fn repeated_registrations_wait_for_a_worker_once() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "exit_guard::windows::tests::exit_child",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(stderr.matches("the exit has waited").count(), 1, "{stderr}");
    }

    // The parent runs this in its own process so the real CRT exit callbacks can close the guard.
    #[test]
    #[ignore = "child process for the UCRT shutdown test"]
    fn exit_child() {
        use super::super::{GUARD, REGISTERED};
        use std::sync::Arc;

        REGISTERED.store(std::process::id(), Ordering::SeqCst);
        for _ in 0..3 {
            super::install();
        }
        let entered = Arc::new(Barrier::new(2));
        let worker = Arc::clone(&entered);
        std::thread::spawn(move || {
            let _busy = GUARD.enter().unwrap();
            worker.wait();
            std::thread::sleep(Duration::from_secs(30));
        });
        entered.wait();
    }
}
