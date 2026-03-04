// Fixture: file with syntax error — should be skipped with a warning

fn main() {
    let _valid = 42;
    this is not valid rust syntax <<<>>>
}
