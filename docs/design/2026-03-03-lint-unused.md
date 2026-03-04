# Design Document: lint-unused

**Author:** Scott Idler
**Date:** 2026-03-03
**Status:** In Review
**Review Passes Completed:** 5/5

## Summary

`lint-unused` is a Rust CLI tool and library that detects underscore-prefixed variable bindings (`_foo`) used to suppress the compiler's `unused_variable` warning. Neither `clippy` nor `rustfmt` flag this pattern. The tool parses Rust source files using the `syn` crate, walks the AST to find `_variable` bindings, applies context-aware filtering (e.g., drop guards), and reports findings in compiler-style diagnostics suitable for CI.

## Problem Statement

### Background

Rust's compiler warns on unused variables. Developers routinely suppress this warning by prefixing the binding with an underscore: `let _result = do_work();`. This is a code smell — it means "I named this variable, I know I'm not using it, and I'm hiding the warning instead of fixing the root cause." Over time, `_variable` bindings accumulate as dead code that the compiler itself will never flag again.

The bare underscore `_` is Rust's proper discard pattern — it drops the value immediately and clearly communicates intent. The `_name` pattern has exactly one legitimate use: **drop guards**, where a named binding extends a value's lifetime for its `Drop` side effect (e.g., `let _guard = mutex.lock()`).

### Problem

There is no existing tool that flags `_variable` bindings:

- **`rustc`** — treats `_name` as "intentionally unused"; emits no warning
- **`clippy`** — has no lint for this pattern
- **`rustfmt`** — formatting only; doesn't analyze semantics
- **grep-based checks** — fragile; match inside comments, strings, macros, and raw strings; produce false positives

The current `.otto.yml` grep check demonstrates the need but is insufficient for production use:
```bash
grep -rn --include='*.rs' -P '(\blet\s+(mut\s+)?|[(,]\s*|\|\s*|\bfor\s+)_[a-zA-Z]' src/
```

This matches `_variable` patterns inside string literals, doc comments, and macro invocations, producing false positives and requiring manual triage.

### Goals

- Detect `_variable` bindings across all Rust binding positions (let, fn params, closures, for loops, match arms, if-let, while-let, destructuring)
- Parse actual Rust syntax (not regex) to avoid false positives from comments, strings, and macros
- Support context-aware filtering: allow known drop-guard patterns and trait impl params
- Provide configurable allow-lists via `lint-unused.yml`
- Produce compiler-style diagnostics (`file:line:col: message`) for human readability
- Exit non-zero when findings exist, for CI integration
- Be fast enough to run on every commit (target: <1s for 50k LOC projects)

### Non-Goals

- Replacing `clippy` or `rustfmt` — this is a single-purpose lint
- Type-level analysis (resolving whether a type implements `Drop`) — use name-based heuristics instead
- Modifying source code (no auto-fix) — report only
- Supporting non-Rust languages
- IDE integration (LSP, editor plugins) — CI-first tool
- Analyzing macro-generated code — only lint hand-written source

## Proposed Solution

### Overview

A Rust binary (`lint-unused`) backed by a library crate (`lint_unused`) that:

1. Discovers `.rs` files in target paths
2. Parses each file with `syn` into a full AST
3. Walks the AST with a `Visitor` to collect all `_variable` bindings with their span locations
4. Filters findings against allow-lists and context rules
5. Reports diagnostics to stdout (tool errors to stderr) and exits with appropriate code

### Architecture

```
┌─────────────────────────────────────────────────┐
│                   CLI (main.rs)                  │
│  arg parsing · config loading · output control   │
└──────────────────┬──────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────┐
│                Library (lib.rs)                   │
│                                                   │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────┐ │
│  │  Discovery   │  │   Parser     │  │ Reporter│ │
│  │  find .rs    │→ │  syn + Visit │→ │ format  │ │
│  │  files       │  │  collect     │  │ output  │ │
│  └─────────────┘  │  findings    │  └─────────┘ │
│                    └──────┬───────┘              │
│                    ┌──────▼───────┐              │
│                    │   Filter     │              │
│                    │  allow-list  │              │
│                    │  context     │              │
│                    └──────────────┘              │
└─────────────────────────────────────────────────┘
```

**Modules:**

| Module | File | Responsibility |
|--------|------|----------------|
| `cli` | `src/cli.rs` | Clap argument parsing |
| `config` | `src/config.rs` | YAML config loading with fallback chain |
| `discovery` | `src/discovery.rs` | Walk directories, find `.rs` files, apply path exclusions |
| `parser` | `src/parser.rs` | `syn` parsing + `Visit` trait impl to collect `_variable` bindings |
| `filter` | `src/filter.rs` | Apply allow-lists, drop-guard heuristics, context rules |
| `reporter` | `src/reporter.rs` | Format and emit diagnostics |
| `lib` | `src/lib.rs` | Public API tying modules together; `lint_files()` entry point |

### Data Model

```rust
/// A single finding: an underscore-prefixed binding that should be reviewed
pub struct Finding {
    /// Absolute or relative path to the source file
    pub file: PathBuf,
    /// 1-based line number
    pub line: usize,
    /// 1-based column number
    pub column: usize,
    /// The binding name (e.g., "_result")
    pub name: String,
    /// What kind of binding position this is
    pub kind: BindingKind,
}

/// The syntactic context where the _variable binding appeared.
/// Note: destructuring (e.g., `let (_a, _b) = ...`) is not a separate kind —
/// the outer context (Let, FnParam, etc.) determines the kind, and the visitor
/// recurses into nested patterns automatically.
pub enum BindingKind {
    Let,          // let _foo = ...
    LetMut,       // let mut _foo = ...
    FnParam,      // fn bar(_foo: Type)
    ClosureParam, // |_foo| ...
    ForLoop,      // for _foo in ...
    MatchArm,     // Some(_foo) => ...
    IfLet,        // if let Some(_foo) = ...
    WhileLet,     // while let Some(_foo) = ...
}

/// Configuration loaded from lint-unused.yml
pub struct Config {
    /// Paths to scan (default: ["src/"])
    pub paths: Vec<PathBuf>,
    /// Glob patterns for paths to exclude (e.g., ["src/generated/**"])
    pub exclude_paths: Vec<String>,
    /// Exact variable names to allow (e.g., ["_guard", "_lock"])
    pub allow_names: Vec<String>,
    /// Regex patterns to allow (e.g., ["_drop_.*", "_.*guard"])
    pub allow_patterns: Vec<String>,
}
```

**Config file discovery chain** (same as scaffold convention):
1. Explicit `--config <FILE>` flag (highest priority)
2. `~/.config/lint-unused/lint-unused.yml`
3. `./lint-unused.yml` in current directory
4. Built-in defaults (no allow-list, scan `src/`)

### CLI Design

```
lint-unused [OPTIONS] [PATHS...]

Arguments:
  [PATHS...]  Paths to scan (default: from config or "src/")

Options:
  -c, --config <FILE>    Path to config file
  -v, --verbose          Show allowed/filtered findings too
  -q, --quiet            Only output finding count
      --format <FMT>     Output format: human (default), json
      --no-default-filters  Disable built-in drop-guard filters
  -h, --help             Print help
  -V, --version          Print version
```

**Exit codes:**
- `0` — no findings
- `1` — findings detected
- `2` — tool error (bad config, parse failure, etc.)

### Example Output

**Human format (default):**
```
src/main.rs:12:9: underscore-prefixed binding `_result` (let)
src/handler.rs:45:22: underscore-prefixed binding `_conn` (fn param)
src/handler.rs:67:14: underscore-prefixed binding `_val` (match arm)

Found 3 underscore-prefixed bindings in 2 files
```

**With `--verbose` (shows filtered findings):**
```
src/main.rs:12:9: underscore-prefixed binding `_result` (let)
src/handler.rs:45:22: underscore-prefixed binding `_conn` (fn param)
src/handler.rs:67:14: underscore-prefixed binding `_val` (match arm)
src/server.rs:23:9: [allowed] `_guard` (let) — matches allow_names
src/server.rs:31:9: [allowed] `_lock` (let) — matches allow_names

Found 3 underscore-prefixed bindings in 2 files (2 allowed)
```

**JSON format:**
```json
{
  "findings": [
    {"file": "src/main.rs", "line": 12, "column": 9, "name": "_result", "kind": "let"}
  ],
  "summary": {"total": 1, "files": 1}
}
```

### AST Visitor Design

The core detection uses `syn::visit::Visit` to walk the parsed AST. Rather than implementing 7+ separate `visit_*` methods, we override a single `visit_pat_ident` which is called for **every** identifier pattern in any binding position (let, fn params, closures, for loops, match arms, if-let, while-let, destructuring). This dramatically simplifies the implementation.

To determine `BindingKind` context, we maintain a stack of ancestor node types as we traverse:

```rust
struct UnusedVisitor {
    file: PathBuf,
    source: String,        // original source for span → line/col mapping
    findings: Vec<Finding>,
    context_stack: Vec<BindingContext>,  // tracks what we're inside
}

#[derive(Clone)]
enum BindingContext {
    LetBinding { is_mut: bool },
    FnParam,
    ClosureParam,
    ForLoop,
    MatchArm,
    IfLet,
    WhileLet,
}

impl<'ast> Visit<'ast> for UnusedVisitor {
    fn visit_pat_ident(&mut self, pat_ident: &'ast syn::PatIdent) {
        let name = pat_ident.ident.to_string();
        // Check: starts with _ AND has at least one letter after _
        if name.starts_with('_') && name.len() > 1 && name[1..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            let kind = self.current_binding_kind(pat_ident.mutability.is_some());
            let (line, column) = self.span_to_line_col(pat_ident.ident.span());
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

    // Push/pop context in visit_local, visit_fn_arg, visit_expr_closure,
    // visit_expr_for_loop, visit_arm, etc.
}
```

**Span → line/col mapping:** Rather than depending on `proc-macro2`'s `span-locations` feature (which uses a global mutex and has a runtime cost), we compute line/col from byte offsets using the original source string. This is a simple scan that builds a line-offset table once per file.

**Note on `syn` features:** Requires `syn = { version = "2", features = ["full", "visit"] }` for full AST parsing and the `Visit` trait.

### Built-in Drop Guard Heuristic

Rather than requiring type analysis, use a name-based heuristic. The following `_variable` names are allowed by default (configurable):

```yaml
# Built-in allow-list (can be disabled with --no-default-filters)
allow_names:
  - _guard
  - _lock
  - _handle
  - _permit
  - _subscription
  - _span        # tracing spans
  - _enter       # tracing span enter guards
  - _timer
  - _tempdir
  - _tempfile
  - _dropper
```

### Implementation Plan

**Phase 1 — Core Detection (MVP)**
- Implement `discovery` module: walk dirs, find `.rs` files, respect exclude paths
- Implement `parser` module: `syn` parsing + `UnusedVisitor` for all binding kinds
- Implement `reporter` module: compiler-style human output
- Wire into `main.rs` with basic CLI
- Add unit tests with inline Rust source strings

**Phase 2 — Configuration & Filtering**
- Implement `filter` module with allow-list matching (names, regex patterns)
- Implement `config` module with YAML loading + fallback chain
- Add built-in drop-guard allow-list with `--no-default-filters` flag
- Add integration tests with fixture `.rs` files

**Phase 3 — CI Polish**
- Add JSON output format
- Add `--quiet` mode
- Add exit code handling (0/1/2)
- Update `.otto.yml` to use `lint-unused` instead of grep
- Add `--verbose` to show filtered findings

## Alternatives Considered

### Alternative 1: Enhanced Grep (current approach)
- **Description:** Improve the regex in `.otto.yml` to skip comments and strings
- **Pros:** Zero dependencies, instant to run, already partially working
- **Cons:** Cannot reliably skip multi-line comments, raw strings, macro invocations, or nested patterns; high false-positive rate; cannot distinguish binding contexts
- **Why not chosen:** Fragile by nature; fixing one edge case introduces another. The whole point of this tool is to do what grep cannot.

### Alternative 2: Tree-sitter (`tree-sitter-rust`)
- **Description:** Use tree-sitter's incremental parser with the Rust grammar
- **Pros:** Error-tolerant (parses incomplete/invalid Rust), incremental, very fast
- **Cons:** Heavier C dependency for tree-sitter runtime, less idiomatic in Rust ecosystem, query API is string-based (less type-safe), grammar may lag behind Rust editions
- **Why not chosen:** `syn` is the standard Rust parsing library, maintained by dtolnay, always current with Rust editions, and produces a fully typed AST. For a Rust-specific tool, `syn` is the natural choice.

### Alternative 3: Custom Clippy Lint
- **Description:** Write a custom `clippy` lint plugin using `rustc`'s internal APIs
- **Pros:** Full type information (can actually check `Drop` impls), integrates with existing `cargo clippy` workflow
- **Cons:** Requires nightly Rust, unstable internal APIs break frequently, heavy compilation overhead, can't distribute as a standalone binary, much more complex to implement and maintain
- **Why not chosen:** Too fragile for a standalone tool. Nightly-only + unstable APIs = maintenance burden. A standalone binary is simpler to install, distribute, and run in CI.

## Technical Considerations

### Dependencies

| Crate | Purpose |
|-------|---------|
| `syn` (full, visit) | Rust source parsing + AST visitor |
| `clap` (derive) | CLI argument parsing |
| `serde` + `serde_yaml` | Config file parsing |
| `colored` | Terminal output formatting |
| `eyre` | Error handling |
| `log` + `env_logger` | Logging |
| `walkdir` | Directory traversal |
| `regex` | Allow-pattern matching |
| `rayon` | Parallel file processing (Phase 3+) |

### Performance

- **Target:** <1s for 50k LOC (typical medium Rust project)
- **Strategy:** `syn` parsing is the bottleneck (~1-5ms per file). For a 200-file project, sequential parsing takes ~200ms-1s. This is acceptable for v1.
- **Future:** Add `rayon` for parallel file parsing if needed. Each file is independent — embarrassingly parallel.
- **Memory:** Each file is parsed independently and findings are collected. Peak memory ≈ largest single file's AST + accumulated findings (negligible).

### Security

- Read-only tool — no writes to user source files
- Config file is user-controlled YAML; use `serde_yaml` (safe parser, no arbitrary code execution)
- No network access
- No shell command execution (unlike the grep approach)

### Testing Strategy

**Unit tests:**
- Parser tests: inline Rust source strings → expected findings (test each `BindingKind`)
- Filter tests: findings + config → expected filtered output
- Reporter tests: findings → expected output string

**Integration tests:**
- Fixture `.rs` files in `tests/fixtures/` with known `_variable` patterns
- Run `lint-unused` binary on fixtures, assert exit code and output

**Edge case fixtures:**
- `_variable` inside string literals (should NOT be flagged)
- `_variable` inside comments (should NOT be flagged)
- `_variable` inside doc comments (should NOT be flagged)
- `_variable` inside macro invocations (should NOT be flagged — macros are opaque to `syn`)
- Nested destructuring: `let ((_a, _b), _c) = ...`
- `_` (bare underscore) — should never be flagged
- `_0`, `_1` — numeric suffix, not a suppressed warning (pattern requires `_` + letter)
- Legitimate drop guards: `let _guard = mutex.lock()`

**Error handling edge cases:**
- Files with syntax errors: `syn::parse_file()` returns `Err` — skip the file, emit a warning to stderr, continue processing other files. Do NOT exit with code 2 for parse errors in user code (only for tool errors like bad config).
- Non-UTF-8 `.rs` files: `fs::read_to_string()` returns `Err` — skip with warning.
- Symlinks: Use `walkdir` with `follow_links(false)` to avoid infinite loops from circular symlinks.
- Empty files: Parse succeeds, zero findings — no special handling needed.

### Rollout Plan

1. Implement and test locally
2. Replace grep check in `.otto.yml` with `cargo run -- src/`
3. Install via `cargo install --path .` for use in other projects
4. Eventually publish to crates.io

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `syn` fails on valid Rust (edition mismatch) | Low | Medium | Pin `syn` version, test against Rust edition 2024 features |
| False positives from macro-generated bindings | Medium | Low | `syn` treats macro invocations as opaque — won't inspect inside them |
| Drop-guard heuristic is too broad/narrow | Medium | Low | Make allow-list fully configurable; start conservative (small default list) |
| Performance degrades on very large codebases | Low | Low | Add `rayon` parallelism; `syn` is already fast per-file |
| Users resist removing legitimate `_guard` patterns | High | Low | Clear docs explaining the heuristic; easy config to add exceptions |
| Symlink loops in scanned directories | Low | Medium | Use `walkdir` with `follow_links(false)` |
| `syn` chokes on files with syntax errors | Medium | Low | Skip unparseable files with warning; don't fail the entire run |

## Open Questions

- [ ] Should we support inline suppression comments (e.g., `// lint-unused:allow`)? Adds complexity but is standard for linters.
- [ ] Should the tool also flag `_` (bare underscore) bindings in certain contexts, or strictly only `_name` patterns?
- [ ] Should we add a `--fix` mode that renames `_foo` to `_` automatically? This changes semantics for drop guards.
- [ ] Should test code (`#[test]`, `#[cfg(test)]`) be linted by default? Tests commonly use `_variable` for setup side effects. Consider `--include-tests` flag.
- [ ] Should we upstream this as a `clippy` lint proposal? If accepted, this tool becomes unnecessary — but that process takes months/years and may be rejected.

## References

- [Rust Reference: Identifier Patterns](https://doc.rust-lang.org/reference/patterns.html#identifier-patterns)
- [`syn` crate documentation](https://docs.rs/syn)
- [`syn::visit::Visit` trait](https://docs.rs/syn/latest/syn/visit/trait.Visit.html)
- [Clippy lint list](https://rust-lang.github.io/rust-clippy/master/index.html) — confirms no existing lint for this
