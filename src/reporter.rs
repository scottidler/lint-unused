use crate::parser::Finding;
use colored::*;
use std::collections::HashSet;
use std::io::Write;

/// Report findings in human-readable compiler-style format.
pub fn report_human(findings: &[Finding], out: &mut dyn Write) -> std::io::Result<()> {
    for finding in findings {
        writeln!(
            out,
            "{}:{}:{}: underscore-prefixed binding {} ({})",
            finding.file.display(),
            finding.line,
            finding.column,
            format!("`{}`", finding.name).yellow(),
            finding.kind,
        )?;
    }

    if !findings.is_empty() {
        let file_count = findings.iter().map(|f| &f.file).collect::<HashSet<_>>().len();
        writeln!(
            out,
            "\nFound {} underscore-prefixed {} in {} {}",
            findings.len().to_string().red(),
            if findings.len() == 1 { "binding" } else { "bindings" },
            file_count,
            if file_count == 1 { "file" } else { "files" },
        )?;
    }

    Ok(())
}

/// Report findings in JSON format.
pub fn report_json(findings: &[Finding], out: &mut dyn Write) -> std::io::Result<()> {
    let file_count = findings.iter().map(|f| &f.file).collect::<HashSet<_>>().len();

    let json_findings: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "file": f.file.display().to_string(),
                "line": f.line,
                "column": f.column,
                "name": f.name,
                "kind": f.kind.to_string(),
            })
        })
        .collect();

    let output = serde_json::json!({
        "findings": json_findings,
        "summary": {
            "total": findings.len(),
            "files": file_count,
        }
    });

    writeln!(out, "{}", serde_json::to_string_pretty(&output).unwrap_or_default())?;
    Ok(())
}

/// Report in quiet mode: just the count.
pub fn report_quiet(findings: &[Finding], out: &mut dyn Write) -> std::io::Result<()> {
    writeln!(out, "{}", findings.len())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{BindingKind, Finding};
    use std::path::PathBuf;

    fn make_finding(name: &str, kind: BindingKind) -> Finding {
        Finding {
            file: PathBuf::from("src/main.rs"),
            line: 12,
            column: 9,
            name: name.to_string(),
            kind,
        }
    }

    #[test]
    fn test_human_report_single() {
        let findings = vec![make_finding("_result", BindingKind::Let)];
        let mut buf = Vec::new();
        report_human(&findings, &mut buf).expect("write");
        let output = String::from_utf8(buf).expect("utf8");
        assert!(output.contains("src/main.rs:12:9"));
        assert!(output.contains("_result"));
        assert!(output.contains("(let)"));
        assert!(output.contains("Found 1"));
    }

    #[test]
    fn test_json_report() {
        let findings = vec![make_finding("_result", BindingKind::Let)];
        let mut buf = Vec::new();
        report_json(&findings, &mut buf).expect("write");
        let output = String::from_utf8(buf).expect("utf8");
        let parsed: serde_json::Value = serde_json::from_str(&output).expect("json parse");
        assert_eq!(parsed["summary"]["total"], 1);
        assert_eq!(parsed["findings"][0]["name"], "_result");
    }

    #[test]
    fn test_quiet_report() {
        let findings = vec![
            make_finding("_a", BindingKind::Let),
            make_finding("_b", BindingKind::FnParam),
        ];
        let mut buf = Vec::new();
        report_quiet(&findings, &mut buf).expect("write");
        let output = String::from_utf8(buf).expect("utf8");
        assert_eq!(output.trim(), "2");
    }

    #[test]
    fn test_empty_findings() {
        let findings: Vec<Finding> = vec![];
        let mut buf = Vec::new();
        report_human(&findings, &mut buf).expect("write");
        let output = String::from_utf8(buf).expect("utf8");
        assert!(output.is_empty());
    }
}
