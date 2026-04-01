//! Real link checking for Markdown documentation.
//!
//! Walks all Markdown files in the workspace (docs/, root *.md) and verifies
//! that local file links resolve to existing targets. External URLs (http/https)
//! are skipped. Reports broken links with file path and line number.

use anyhow::{Result, bail};
use regex::Regex;
use std::path::{Path, PathBuf};

/// A single broken link found during checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenLink {
    /// The Markdown file containing the link.
    pub source_file: PathBuf,
    /// 1-based line number where the link appears.
    pub line_number: usize,
    /// The raw link target as written in Markdown.
    pub target: String,
    /// Why the link is broken.
    pub reason: String,
}

impl std::fmt::Display for BrokenLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "  {}:{}: {} ({})",
            self.source_file.display(),
            self.line_number,
            self.target,
            self.reason,
        )
    }
}

/// Collect all Markdown files to check from the workspace root.
pub fn collect_markdown_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    // Root-level *.md files
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext.eq_ignore_ascii_case("md") {
                        files.push(path);
                    }
                }
            }
        }
    }

    // Recursively walk docs/
    let docs_dir = root.join("docs");
    if docs_dir.is_dir() {
        walk_dir_for_md(&docs_dir, &mut files);
    }

    files.sort();
    files
}

/// Recursively collect `.md` files from a directory.
fn walk_dir_for_md(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_dir_for_md(&path, out);
        } else if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case("md") {
                    out.push(path);
                }
            }
        }
    }
}

/// Extract Markdown links from a single line.
///
/// Matches inline links `[text](target)` and returns `(target, fragment)` pairs.
/// Reference-style link definitions `[label]: target` are also matched.
pub fn extract_links(line: &str) -> Vec<String> {
    let mut links = Vec::new();

    // Inline links: [text](target)
    // Use a regex that handles nested brackets minimally.
    let inline_re = Regex::new(r"\[(?:[^\[\]]|\[[^\]]*\])*\]\(([^)]+)\)").unwrap();
    for cap in inline_re.captures_iter(line) {
        if let Some(m) = cap.get(1) {
            links.push(m.as_str().to_string());
        }
    }

    // Reference-style link definitions: [label]: target
    let ref_re = Regex::new(r"^\s{0,3}\[[^\]]+\]:\s+(\S+)").unwrap();
    if let Some(cap) = ref_re.captures(line) {
        if let Some(m) = cap.get(1) {
            links.push(m.as_str().to_string());
        }
    }

    links
}

/// Check whether a link target is external (http/https).
fn is_external(target: &str) -> bool {
    target.starts_with("http://") || target.starts_with("https://")
}

/// Check whether a link target is a mailto or other non-file scheme.
fn is_non_file_scheme(target: &str) -> bool {
    target.starts_with("mailto:") || target.starts_with("tel:") || target.starts_with("data:")
}

/// Strip the fragment (#anchor) from a link target, returning (path_part, fragment).
fn split_fragment(target: &str) -> (&str, Option<&str>) {
    if let Some(pos) = target.find('#') {
        let path = &target[..pos];
        let frag = &target[pos + 1..];
        if path.is_empty() {
            // Pure anchor link like #heading — skip file check
            ("", Some(frag))
        } else {
            (path, Some(frag))
        }
    } else {
        (target, None)
    }
}

/// Check all links in a single Markdown file.
pub fn check_file(source: &Path) -> Vec<BrokenLink> {
    let content = match std::fs::read_to_string(source) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let source_dir = source.parent().unwrap_or(Path::new("."));
    let mut broken = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        let line_number = line_idx + 1;

        for raw_target in extract_links(line) {
            // Skip external URLs
            if is_external(&raw_target) || is_non_file_scheme(&raw_target) {
                continue;
            }

            let (path_part, _fragment) = split_fragment(&raw_target);

            // Pure anchor links (#something) — skip, they reference the same file
            if path_part.is_empty() {
                continue;
            }

            // Resolve relative to the source file's directory
            let resolved = source_dir.join(path_part);

            if !resolved.exists() {
                broken.push(BrokenLink {
                    source_file: source.to_path_buf(),
                    line_number,
                    target: raw_target,
                    reason: "target file not found".to_string(),
                });
            }
        }
    }

    broken
}

/// Run link checking across all Markdown files in the workspace.
///
/// Returns `Ok(())` if no broken links are found, or an error listing all broken links.
pub fn check_links(root: &Path) -> Result<()> {
    let files = collect_markdown_files(root);
    let mut all_broken: Vec<BrokenLink> = Vec::new();

    for file in &files {
        let broken = check_file(file);
        all_broken.extend(broken);
    }

    if all_broken.is_empty() {
        println!("  checked {} Markdown files — no broken links", files.len());
        Ok(())
    } else {
        eprintln!(
            "found {} broken link(s) across {} file(s):",
            all_broken.len(),
            files.len()
        );
        for bl in &all_broken {
            eprintln!("{bl}");
        }
        bail!(
            "docs-check failed: {} broken link(s) found",
            all_broken.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn extract_inline_links() {
        let links = extract_links("See [foo](bar.md) and [baz](../qux.md#heading).");
        assert_eq!(links, vec!["bar.md", "../qux.md#heading"]);
    }

    #[test]
    fn extract_reference_links() {
        let links = extract_links("[label]: path/to/file.md");
        assert_eq!(links, vec!["path/to/file.md"]);
    }

    #[test]
    fn extract_skips_external() {
        let links = extract_links("[ext](https://example.com) and [local](foo.md)");
        // extract_links returns all; filtering is done in check_file
        assert_eq!(links.len(), 2);
        assert!(is_external("https://example.com"));
        assert!(!is_external("foo.md"));
    }

    #[test]
    fn split_fragment_works() {
        assert_eq!(split_fragment("foo.md#bar"), ("foo.md", Some("bar")));
        assert_eq!(split_fragment("foo.md"), ("foo.md", None));
        assert_eq!(split_fragment("#anchor"), ("", Some("anchor")));
    }

    #[test]
    fn check_file_finds_broken_links() {
        let tmp = TempDir::new().unwrap();
        let md = tmp.path().join("test.md");
        fs::write(&md, "# Test\n\n[good](test.md)\n[bad](missing.md)\n").unwrap();

        let broken = check_file(&md);
        assert_eq!(broken.len(), 1);
        assert_eq!(broken[0].target, "missing.md");
        assert_eq!(broken[0].line_number, 4);
    }

    #[test]
    fn check_file_skips_external_and_anchors() {
        let tmp = TempDir::new().unwrap();
        let md = tmp.path().join("test.md");
        fs::write(
            &md,
            "[ext](https://example.com)\n[anchor](#heading)\n[mailto](mailto:a@b.com)\n",
        )
        .unwrap();

        let broken = check_file(&md);
        assert!(broken.is_empty());
    }

    #[test]
    fn check_links_on_clean_dir() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path().join("docs");
        fs::create_dir_all(&docs).unwrap();
        fs::write(docs.join("readme.md"), "# Docs\n\n[self](readme.md)\n").unwrap();

        // No root Cargo.toml with [workspace], but check_links just needs the root path
        let result = check_links(tmp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn check_links_reports_broken() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path().join("docs");
        fs::create_dir_all(&docs).unwrap();
        fs::write(docs.join("readme.md"), "[broken](nonexistent.md)\n").unwrap();

        let result = check_links(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn collect_markdown_files_finds_root_and_docs() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("README.md"), "# Hi").unwrap();
        let docs = tmp.path().join("docs");
        fs::create_dir_all(docs.join("sub")).unwrap();
        fs::write(docs.join("guide.md"), "# Guide").unwrap();
        fs::write(docs.join("sub").join("deep.md"), "# Deep").unwrap();
        // Non-md file should be ignored
        fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();

        let files = collect_markdown_files(tmp.path());
        assert_eq!(files.len(), 3);
    }
}
