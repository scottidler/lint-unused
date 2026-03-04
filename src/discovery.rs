use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Discover all `.rs` files in the given paths, excluding paths matching exclude patterns.
pub fn discover_rs_files(paths: &[PathBuf], exclude_patterns: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for path in paths {
        if path.is_file() {
            if path.extension().is_some_and(|ext| ext == "rs") && !is_excluded(path, exclude_patterns) {
                files.push(path.clone());
            }
        } else if path.is_dir() {
            for entry in WalkDir::new(path)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let entry_path = entry.path();
                if entry_path.is_file()
                    && entry_path.extension().is_some_and(|ext| ext == "rs")
                    && !is_excluded(entry_path, exclude_patterns)
                {
                    files.push(entry_path.to_path_buf());
                }
            }
        }
    }

    files.sort();
    files
}

/// Check if a path matches any of the exclude glob patterns.
fn is_excluded(path: &Path, exclude_patterns: &[String]) -> bool {
    let path_str = path.to_string_lossy();
    for pattern in exclude_patterns {
        // Simple glob matching: support ** and *
        if glob_matches(pattern, &path_str) {
            return true;
        }
    }
    false
}

/// Simple glob matching supporting `*` (single segment) and `**` (any segments).
fn glob_matches(pattern: &str, path: &str) -> bool {
    // Convert glob to regex
    let mut regex_str = String::from("(?:^|/)");
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    // Skip optional following /
                    if chars.peek() == Some(&'/') {
                        chars.next();
                    }
                    regex_str.push_str(".*");
                } else {
                    regex_str.push_str("[^/]*");
                }
            }
            '.' => regex_str.push_str("\\."),
            '?' => regex_str.push('.'),
            c => regex_str.push(c),
        }
    }

    regex::Regex::new(&regex_str).ok().is_some_and(|re| re.is_match(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_discover_rs_files_in_dir() {
        let dir = std::env::temp_dir().join("lint_unused_test_discover");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).expect("create dir");
        fs::write(dir.join("foo.rs"), "fn main() {}").expect("write");
        fs::write(dir.join("sub/bar.rs"), "fn bar() {}").expect("write");
        fs::write(dir.join("readme.txt"), "hello").expect("write");

        let files = discover_rs_files(std::slice::from_ref(&dir), &[]);
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.extension().unwrap() == "rs"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_excludes_patterns() {
        let dir = std::env::temp_dir().join("lint_unused_test_exclude");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("generated")).expect("create dir");
        fs::write(dir.join("good.rs"), "fn good() {}").expect("write");
        fs::write(dir.join("generated/bad.rs"), "fn bad() {}").expect("write");

        let files = discover_rs_files(std::slice::from_ref(&dir), &["generated/**".to_string()]);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("good.rs"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_single_file() {
        let dir = std::env::temp_dir().join("lint_unused_test_single");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        let file = dir.join("single.rs");
        fs::write(&file, "fn single() {}").expect("write");

        let files = discover_rs_files(&[file], &[]);
        assert_eq!(files.len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_glob_matches() {
        assert!(glob_matches("generated/**", "src/generated/foo.rs"));
        assert!(glob_matches("*.rs", "foo.rs"));
        assert!(!glob_matches("generated/**", "src/main.rs"));
    }
}
