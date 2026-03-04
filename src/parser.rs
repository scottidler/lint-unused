use std::path::{Path, PathBuf};
use syn::visit::Visit;

/// The syntactic context where the _variable binding appeared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingKind {
    Let,
    LetMut,
    FnParam,
    ClosureParam,
    ForLoop,
    MatchArm,
    IfLet,
    WhileLet,
}

impl std::fmt::Display for BindingKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BindingKind::Let => write!(f, "let"),
            BindingKind::LetMut => write!(f, "let mut"),
            BindingKind::FnParam => write!(f, "fn param"),
            BindingKind::ClosureParam => write!(f, "closure param"),
            BindingKind::ForLoop => write!(f, "for loop"),
            BindingKind::MatchArm => write!(f, "match arm"),
            BindingKind::IfLet => write!(f, "if let"),
            BindingKind::WhileLet => write!(f, "while let"),
        }
    }
}

/// A single finding: an underscore-prefixed binding that should be reviewed.
#[derive(Debug, Clone)]
pub struct Finding {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub name: String,
    pub kind: BindingKind,
}

/// Context stack entry for determining BindingKind.
#[derive(Clone, Debug)]
enum BindingContext {
    LetBinding { is_mut: bool },
    FnParam,
    ClosureParam,
    ForLoop,
    MatchArm,
    IfLet,
    WhileLet,
}

/// AST visitor that collects _variable bindings.
struct UnusedVisitor {
    file: PathBuf,
    line_offsets: Vec<usize>,
    findings: Vec<Finding>,
    context_stack: Vec<BindingContext>,
}

impl UnusedVisitor {
    fn new(file: PathBuf, source: &str) -> Self {
        let line_offsets = Self::build_line_offsets(source);
        Self {
            file,
            line_offsets,
            findings: Vec::new(),
            context_stack: Vec::new(),
        }
    }

    fn build_line_offsets(source: &str) -> Vec<usize> {
        let mut offsets = vec![0];
        for (i, c) in source.char_indices() {
            if c == '\n' {
                offsets.push(i + 1);
            }
        }
        offsets
    }

    fn offset_to_line_col(&self, offset: usize) -> (usize, usize) {
        match self.line_offsets.binary_search(&offset) {
            Ok(line) => (line + 1, 1),
            Err(line) => {
                let line = line - 1;
                let col = offset - self.line_offsets[line] + 1;
                (line + 1, col)
            }
        }
    }

    fn current_binding_kind(&self, has_mutability: bool) -> BindingKind {
        if let Some(ctx) = self.context_stack.last() {
            match ctx {
                BindingContext::LetBinding { is_mut } => {
                    if *is_mut || has_mutability {
                        BindingKind::LetMut
                    } else {
                        BindingKind::Let
                    }
                }
                BindingContext::FnParam => BindingKind::FnParam,
                BindingContext::ClosureParam => BindingKind::ClosureParam,
                BindingContext::ForLoop => BindingKind::ForLoop,
                BindingContext::MatchArm => BindingKind::MatchArm,
                BindingContext::IfLet => BindingKind::IfLet,
                BindingContext::WhileLet => BindingKind::WhileLet,
            }
        } else if has_mutability {
            BindingKind::LetMut
        } else {
            BindingKind::Let
        }
    }
}

impl<'ast> Visit<'ast> for UnusedVisitor {
    fn visit_pat_ident(&mut self, pat_ident: &'ast syn::PatIdent) {
        let name = pat_ident.ident.to_string();
        // Check: starts with _ AND has at least one letter after _
        if name.starts_with('_') && name.len() > 1 && name[1..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            let kind = self.current_binding_kind(pat_ident.mutability.is_some());
            let span = pat_ident.ident.span();
            let offset = span.byte_range().start;
            let (line, column) = self.offset_to_line_col(offset);
            self.findings.push(Finding {
                file: self.file.clone(),
                line,
                column,
                name,
                kind,
            });
        }
        syn::visit::visit_pat_ident(self, pat_ident);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        let is_mut = false; // mutability is on PatIdent, detected in visit_pat_ident
        self.context_stack.push(BindingContext::LetBinding { is_mut });
        syn::visit::visit_local(self, local);
        self.context_stack.pop();
    }

    fn visit_fn_arg(&mut self, arg: &'ast syn::FnArg) {
        self.context_stack.push(BindingContext::FnParam);
        syn::visit::visit_fn_arg(self, arg);
        self.context_stack.pop();
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        // Push closure param context for visiting params
        self.context_stack.push(BindingContext::ClosureParam);
        for input in &closure.inputs {
            self.visit_pat(input);
        }
        self.context_stack.pop();

        // Visit the body with no special context
        self.visit_expr(&closure.body);
    }

    fn visit_expr_for_loop(&mut self, for_loop: &'ast syn::ExprForLoop) {
        self.context_stack.push(BindingContext::ForLoop);
        self.visit_pat(&for_loop.pat);
        self.context_stack.pop();

        self.visit_expr(&for_loop.expr);
        self.visit_block(&for_loop.body);
    }

    fn visit_arm(&mut self, arm: &'ast syn::Arm) {
        self.context_stack.push(BindingContext::MatchArm);
        self.visit_pat(&arm.pat);
        self.context_stack.pop();

        if let Some((_, ref expr)) = arm.guard {
            self.visit_expr(expr);
        }
        self.visit_expr(&arm.body);
    }

    fn visit_expr_if(&mut self, expr_if: &'ast syn::ExprIf) {
        // Check if this is an if-let
        if let syn::Expr::Let(ref expr_let) = *expr_if.cond {
            self.context_stack.push(BindingContext::IfLet);
            self.visit_pat(&expr_let.pat);
            self.context_stack.pop();
            self.visit_expr(&expr_let.expr);
        } else {
            self.visit_expr(&expr_if.cond);
        }
        self.visit_block(&expr_if.then_branch);
        if let Some((_, ref else_branch)) = expr_if.else_branch {
            self.visit_expr(else_branch);
        }
    }

    fn visit_expr_while(&mut self, expr_while: &'ast syn::ExprWhile) {
        // Check if this is a while-let
        if let syn::Expr::Let(ref expr_let) = *expr_while.cond {
            self.context_stack.push(BindingContext::WhileLet);
            self.visit_pat(&expr_let.pat);
            self.context_stack.pop();
            self.visit_expr(&expr_let.expr);
        } else {
            self.visit_expr(&expr_while.cond);
        }
        self.visit_block(&expr_while.body);
    }
}

/// Parse a Rust source file and collect all _variable findings.
pub fn parse_file(path: &Path) -> Result<Vec<Finding>, String> {
    let source = std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    parse_source(path, &source)
}

/// Parse Rust source code and collect all _variable findings.
pub fn parse_source(path: &Path, source: &str) -> Result<Vec<Finding>, String> {
    let syntax = syn::parse_file(source).map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;

    let mut visitor = UnusedVisitor::new(path.to_path_buf(), source);
    visitor.visit_file(&syntax);

    Ok(visitor.findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn parse(source: &str) -> Vec<Finding> {
        parse_source(Path::new("test.rs"), source).expect("parse failed")
    }

    #[test]
    fn test_let_binding() {
        let findings = parse("fn main() { let _result = 42; }");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].name, "_result");
        assert_eq!(findings[0].kind, BindingKind::Let);
    }

    #[test]
    fn test_let_mut_binding() {
        let findings = parse("fn main() { let mut _result = 42; }");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].name, "_result");
        assert_eq!(findings[0].kind, BindingKind::LetMut);
    }

    #[test]
    fn test_fn_param() {
        let findings = parse("fn foo(_bar: i32) {}");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].name, "_bar");
        assert_eq!(findings[0].kind, BindingKind::FnParam);
    }

    #[test]
    fn test_closure_param() {
        let findings = parse("fn main() { let f = |_x| _x + 1; }");
        // _x appears as closure param (binding site), not in the body (usage is a different AST node)
        assert!(!findings.is_empty());
        assert_eq!(findings[0].name, "_x");
        assert_eq!(findings[0].kind, BindingKind::ClosureParam);
    }

    #[test]
    fn test_for_loop() {
        let findings = parse("fn main() { for _item in vec![1, 2, 3] {} }");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].name, "_item");
        assert_eq!(findings[0].kind, BindingKind::ForLoop);
    }

    #[test]
    fn test_match_arm() {
        let findings = parse(
            r#"fn main() {
                match Some(1) {
                    Some(_val) => {},
                    None => {},
                }
            }"#,
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].name, "_val");
        assert_eq!(findings[0].kind, BindingKind::MatchArm);
    }

    #[test]
    fn test_if_let() {
        let findings = parse(
            r#"fn main() {
                if let Some(_val) = Some(1) {}
            }"#,
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].name, "_val");
        assert_eq!(findings[0].kind, BindingKind::IfLet);
    }

    #[test]
    fn test_while_let() {
        let findings = parse(
            r#"fn main() {
                let mut iter = vec![1].into_iter();
                while let Some(_val) = iter.next() {}
            }"#,
        );
        // Should find _val as WhileLet
        let wl: Vec<_> = findings.iter().filter(|f| f.kind == BindingKind::WhileLet).collect();
        assert_eq!(wl.len(), 1);
        assert_eq!(wl[0].name, "_val");
    }

    #[test]
    fn test_bare_underscore_not_flagged() {
        let findings = parse("fn main() { let _ = 42; }");
        assert!(findings.is_empty());
    }

    #[test]
    fn test_numeric_suffix_not_flagged() {
        let findings = parse("fn main() { let _0 = 42; }");
        assert!(findings.is_empty());
    }

    #[test]
    fn test_destructuring() {
        let findings = parse("fn main() { let (_a, _b) = (1, 2); }");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].name, "_a");
        assert_eq!(findings[1].name, "_b");
    }

    #[test]
    fn test_nested_destructuring() {
        let findings = parse("fn main() { let ((_a, _b), _c) = ((1, 2), 3); }");
        assert_eq!(findings.len(), 3);
    }

    #[test]
    fn test_string_literal_not_flagged() {
        let findings = parse(r#"fn main() { let s = "let _foo = 1"; }"#);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_comment_not_flagged() {
        let findings = parse(
            r#"fn main() {
                // let _foo = 1;
                let x = 42;
            }"#,
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn test_line_and_column() {
        let source = "fn main() {\n    let _foo = 42;\n}";
        let findings = parse(source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 2);
        assert_eq!(findings[0].column, 9);
    }
}
