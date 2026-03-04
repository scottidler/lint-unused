use crate::parser::Finding;

/// Result of filtering: findings split into reported and allowed.
pub struct FilterResult {
    pub reported: Vec<Finding>,
    pub allowed: Vec<(Finding, String)>, // (finding, reason)
}

/// Filter findings against allow-lists. Returns split results.
pub fn filter_findings(findings: Vec<Finding>, allow_names: &[String], allow_patterns: &[String]) -> FilterResult {
    let compiled_patterns: Vec<(regex::Regex, &String)> = allow_patterns
        .iter()
        .filter_map(|p| regex::Regex::new(p).ok().map(|re| (re, p)))
        .collect();

    let mut reported = Vec::new();
    let mut allowed = Vec::new();

    for finding in findings {
        if let Some(matched_name) = allow_names.iter().find(|n| **n == finding.name) {
            allowed.push((finding, format!("matches allow_names: {}", matched_name)));
        } else if let Some((_, pattern)) = compiled_patterns.iter().find(|(re, _)| re.is_match(&finding.name)) {
            allowed.push((finding, format!("matches allow_patterns: {}", pattern)));
        } else {
            reported.push(finding);
        }
    }

    FilterResult { reported, allowed }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{BindingKind, Finding};
    use std::path::PathBuf;

    fn make_finding(name: &str) -> Finding {
        Finding {
            file: PathBuf::from("test.rs"),
            line: 1,
            column: 1,
            name: name.to_string(),
            kind: BindingKind::Let,
        }
    }

    #[test]
    fn test_no_filters() {
        let findings = vec![make_finding("_foo"), make_finding("_bar")];
        let result = filter_findings(findings, &[], &[]);
        assert_eq!(result.reported.len(), 2);
        assert!(result.allowed.is_empty());
    }

    #[test]
    fn test_allow_names() {
        let findings = vec![make_finding("_guard"), make_finding("_foo"), make_finding("_lock")];
        let allow_names = vec!["_guard".to_string(), "_lock".to_string()];
        let result = filter_findings(findings, &allow_names, &[]);
        assert_eq!(result.reported.len(), 1);
        assert_eq!(result.reported[0].name, "_foo");
        assert_eq!(result.allowed.len(), 2);
    }

    #[test]
    fn test_allow_patterns() {
        let findings = vec![
            make_finding("_drop_guard"),
            make_finding("_foo"),
            make_finding("_my_guard"),
        ];
        let allow_patterns = vec!["_drop_.*".to_string(), "_.*guard".to_string()];
        let result = filter_findings(findings, &[], &allow_patterns);
        assert_eq!(result.reported.len(), 1);
        assert_eq!(result.reported[0].name, "_foo");
        assert_eq!(result.allowed.len(), 2);
    }

    #[test]
    fn test_names_take_priority_over_patterns() {
        let findings = vec![make_finding("_guard")];
        let allow_names = vec!["_guard".to_string()];
        let allow_patterns = vec!["_guard".to_string()];
        let result = filter_findings(findings, &allow_names, &allow_patterns);
        assert_eq!(result.allowed.len(), 1);
        assert!(result.allowed[0].1.contains("allow_names"));
    }
}
