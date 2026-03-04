// Fixture: legitimate drop-guard patterns (should be allowed by default filters)

use std::sync::Mutex;

fn main() {
    let mutex = Mutex::new(42);

    let _guard = mutex.lock().unwrap();
    let _lock = mutex.lock().unwrap();
    let _handle = 42; // simulated handle
    let _permit = 42; // simulated permit
    let _subscription = 42;
    let _span = 42; // tracing span
    let _enter = 42; // tracing enter guard
    let _timer = 42;
    let _tempdir = 42;
    let _tempfile = 42;
    let _dropper = 42;
}
