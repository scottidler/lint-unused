use std::path::Path;
use std::process::Command;

fn cargo_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO"));
    cmd.args(["run", "--quiet", "--"]);
    cmd
}

fn fixture(name: &str) -> String {
    Path::new("tests/fixtures").join(name).display().to_string()
}

#[test]
fn all_binding_kinds_detected() {
    let output = cargo_bin()
        .args(["--no-default-filters", &fixture("all_binding_kinds.rs")])
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // All 11 _variable bindings should be detected
    assert!(!output.status.success(), "should exit 1 when findings exist");

    // Check each binding kind is found
    assert!(stdout.contains("_param"), "should find fn param _param");
    assert!(stdout.contains("_result"), "should find let _result");
    assert!(stdout.contains("_counter"), "should find let mut _counter");
    assert!(stdout.contains("_x"), "should find closure param _x");
    assert!(stdout.contains("_item"), "should find for loop _item");
    assert!(stdout.contains("_val"), "should find match arm _val");
    assert!(stdout.contains("_inner"), "should find if-let _inner");
    assert!(stdout.contains("_elem"), "should find while-let _elem");
    assert!(stdout.contains("_a"), "should find destructuring _a");
    assert!(stdout.contains("_b"), "should find destructuring _b");
    assert!(stdout.contains("_c"), "should find destructuring _c");
}

#[test]
fn false_positives_not_flagged() {
    let output = cargo_bin()
        .args(["--no-default-filters", &fixture("false_positives.rs")])
        .output()
        .expect("failed to run");

    assert!(
        output.status.success(),
        "should exit 0 — no findings in false_positives.rs\nstdout: {}",
        String::from_utf8_lossy(&output.stdout),
    );
}

#[test]
fn drop_guards_allowed_by_default() {
    let output = cargo_bin()
        .arg(fixture("drop_guards.rs"))
        .output()
        .expect("failed to run");

    assert!(
        output.status.success(),
        "should exit 0 — all drop guards allowed by default\nstdout: {}",
        String::from_utf8_lossy(&output.stdout),
    );
}

#[test]
fn drop_guards_flagged_without_defaults() {
    let output = cargo_bin()
        .args(["--no-default-filters", &fixture("drop_guards.rs")])
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(!output.status.success(), "should exit 1 with --no-default-filters");
    assert!(stdout.contains("_guard"), "should flag _guard without defaults");
    assert!(stdout.contains("_lock"), "should flag _lock without defaults");
}

#[test]
fn empty_file_no_findings() {
    let output = cargo_bin().arg(fixture("empty.rs")).output().expect("failed to run");

    assert!(output.status.success(), "empty file should exit 0");
}

#[test]
fn syntax_error_skipped_with_warning() {
    let output = cargo_bin()
        .arg(fixture("syntax_error.rs"))
        .output()
        .expect("failed to run");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should warn about parse failure, not crash
    assert!(
        stderr.contains("warning:"),
        "should emit a warning for syntax error\nstderr: {stderr}"
    );
    // Should still exit 0 (no findings, just a warning)
    assert!(output.status.success(), "syntax error file should not cause exit 2");
}

#[test]
fn mixed_findings_with_default_filters() {
    let output = cargo_bin().arg(fixture("mixed.rs")).output().expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(!output.status.success(), "should exit 1 — some findings not allowed");
    assert!(stdout.contains("_result"), "should flag _result");
    assert!(stdout.contains("_unused_value"), "should flag _unused_value");
    assert!(!stdout.contains("_guard"), "_guard should be allowed by default");
    assert!(!stdout.contains("_lock"), "_lock should be allowed by default");
}

#[test]
fn verbose_shows_allowed() {
    let output = cargo_bin()
        .args(["--verbose", &fixture("mixed.rs")])
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("[allowed]"), "--verbose should show allowed findings");
    assert!(stdout.contains("_guard"), "--verbose should show allowed _guard");
}

#[test]
fn quiet_mode_outputs_count() {
    let output = cargo_bin()
        .args(["--quiet", "--no-default-filters", &fixture("all_binding_kinds.rs")])
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let count: usize = stdout.trim().parse().expect("quiet mode should output a number");
    assert!(count > 0, "should find bindings");
}

#[test]
fn json_format_valid() {
    let output = cargo_bin()
        .args([
            "--format",
            "json",
            "--no-default-filters",
            &fixture("all_binding_kinds.rs"),
        ])
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");

    assert!(parsed["findings"].is_array(), "should have findings array");
    assert!(
        parsed["summary"]["total"].as_u64().unwrap_or(0) > 0,
        "should have findings"
    );
}

#[test]
fn exit_code_0_no_findings() {
    let output = cargo_bin().arg(fixture("empty.rs")).output().expect("failed to run");

    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn exit_code_1_findings_detected() {
    let output = cargo_bin()
        .args(["--no-default-filters", &fixture("all_binding_kinds.rs")])
        .output()
        .expect("failed to run");

    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn config_file_flag() {
    // Create a temp config that allows _result
    let dir = std::env::temp_dir().join("lint_unused_test_config");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create dir");
    let config = dir.join("test-config.yml");
    std::fs::write(
        &config,
        "paths: []\nexclude_paths: []\nallow_names:\n  - _result\n  - _unused_value\nallow_patterns: []\n",
    )
    .expect("write config");

    let output = cargo_bin()
        .args(["--config", config.to_str().unwrap(), &fixture("mixed.rs")])
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // _result and _unused_value should be allowed, _guard and _lock still allowed by defaults
    assert!(
        output.status.success(),
        "all findings should be allowed via config + defaults\nstdout: {stdout}",
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn multiple_paths() {
    let output = cargo_bin()
        .args([
            "--no-default-filters",
            &fixture("all_binding_kinds.rs"),
            &fixture("mixed.rs"),
        ])
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should find findings from both files
    assert!(stdout.contains("all_binding_kinds.rs"), "should include first file");
    assert!(stdout.contains("mixed.rs"), "should include second file");
}
