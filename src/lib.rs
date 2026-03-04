pub mod discovery;
pub mod filter;
pub mod parser;
pub mod reporter;

use parser::Finding;
use std::path::PathBuf;

/// Lint result containing findings and any warnings.
pub struct LintResult {
    pub findings: Vec<Finding>,
    pub warnings: Vec<String>,
}

/// Main entry point: discover files, parse them, and collect findings.
pub fn lint_files(paths: &[PathBuf], exclude_paths: &[String]) -> LintResult {
    let files = discovery::discover_rs_files(paths, exclude_paths);
    let mut findings = Vec::new();
    let mut warnings = Vec::new();

    for file in &files {
        match parser::parse_file(file) {
            Ok(file_findings) => findings.extend(file_findings),
            Err(e) => warnings.push(e),
        }
    }

    LintResult { findings, warnings }
}
