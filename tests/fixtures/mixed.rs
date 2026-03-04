// Fixture: mix of flagged and allowed bindings

fn main() {
    // Should be flagged
    let _result = 42;
    let _unused_value = "hello";

    // Should be allowed (default drop-guard names)
    let _guard = 42;
    let _lock = 42;

    // Normal — not flagged
    let _ = 42;
    let normal = 1;
    let _ = normal;
}
