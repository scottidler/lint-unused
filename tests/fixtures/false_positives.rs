// Fixture: things that should NOT be flagged

fn main() {
    // Bare underscore — proper discard
    let _ = 42;

    // Numeric suffix — not a suppressed warning
    let _0 = 1;
    let _1 = 2;

    // Normal variables (no underscore prefix)
    let result = 42;
    let _ = result;

    // _variable inside a string literal
    let s = "let _foo = 1";
    let _ = s;

    // _variable inside a raw string literal
    let r = r#"let _bar = 2"#;
    let _ = r;

    // _variable inside a byte string
    let b = b"_baz";
    let _ = b;

    // _variable inside a comment — not parsed as code
    // let _commented = 42;

    /* let _block_comment = 42; */

    /// Doc comment with _doc_var reference
    fn documented() {}
    documented();

    // _variable inside a macro invocation — opaque to syn
    println!("_macro_var");
    format!("let _fmt = {}", 1);
}
