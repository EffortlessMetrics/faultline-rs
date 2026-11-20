//! Real link checking for Markdown documentation.
//!
//! Walks all Markdown files in the workspace (docs/, root *.md) and verifies
//! that local file links resolve to existing targets. External URLs (http/https)
//! are skipped. Reports broken links with file path and line number.

use anyhow::{Context, Result, bail};
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
            if path.is_file()
                && let Some(ext) = path.extension()
                && ext.eq_ignore_ascii_case("md")
            {
                files.push(path);
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
        } else if path.is_file()
            && let Some(ext) = path.extension()
            && ext.eq_ignore_ascii_case("md")
        {
            out.push(path);
        }
    }
}

/// Extract Markdown links from a single line.
///
/// Matches inline links `[text](target)` and returns `(target, fragment)` pairs.
/// Reference-style link definitions `[label]: target` are also matched.
pub fn extract_links(line: &str) -> Result<Vec<String>> {
    let mut links = Vec::new();

    // Inline links: [text](target)
    // Use a regex that handles nested brackets minimally.
    let inline_re = Regex::new(r"\[(?:[^\[\]]|\[[^\]]*\])*\]\(([^)]+)\)")?;
    for cap in inline_re.captures_iter(line) {
        if let Some(m) = cap.get(1) {
            links.push(m.as_str().to_string());
        }
    }

    // Reference-style link definitions: [label]: target
    let ref_re = Regex::new(r"^\s{0,3}\[[^\]]+\]:\s+(\S+)")?;
    if let Some(cap) = ref_re.captures(line)
        && let Some(m) = cap.get(1)
    {
        links.push(m.as_str().to_string());
    }

    Ok(links)
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
    match target.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (target, None),
    }
}

/// Check all links in a single Markdown file.
pub fn check_file(source: &Path) -> Result<Vec<BrokenLink>> {
    let content = std::fs::read_to_string(source)
        .with_context(|| format!("failed to read {}", source.display()))?;

    let source_dir = source.parent().unwrap_or(Path::new("."));
    let mut broken = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        let line_number = line_idx + 1;

        for raw_target in extract_links(line)? {
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

    Ok(broken)
}

/// Run link checking across all Markdown files in the workspace.
///
/// Returns `Ok(())` if no broken links are found, or an error listing all broken links.
pub fn check_links(root: &Path) -> Result<()> {
    let files = collect_markdown_files(root);
    let mut all_broken: Vec<BrokenLink> = Vec::new();

    for file in &files {
        let broken = check_file(file)?;
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
    fn extract_inline_links() -> Result<()> {
        let links = extract_links("See [foo](bar.md) and [baz](../qux.md#heading).")?;
        anyhow::ensure!(
            links == ["bar.md", "../qux.md#heading"],
            "unexpected links: {links:?}"
        );
        Ok(())
    }

    #[test]
    fn extract_reference_links() -> Result<()> {
        let links = extract_links("[label]: path/to/file.md")?;
        anyhow::ensure!(links == ["path/to/file.md"], "unexpected links: {links:?}");
        Ok(())
    }

    #[test]
    fn extract_skips_external() -> Result<()> {
        let links = extract_links("[ext](https://example.com) and [local](foo.md)")?;
        // extract_links returns all; filtering is done in check_file
        anyhow::ensure!(links.len() == 2, "unexpected links: {links:?}");
        anyhow::ensure!(is_external("https://example.com"));
        anyhow::ensure!(!is_external("foo.md"));
        Ok(())
    }

    #[test]
    fn split_fragment_works() {
        assert_eq!(split_fragment("foo.md#bar"), ("foo.md", Some("bar")));
        assert_eq!(split_fragment("foo.md"), ("foo.md", None));
        assert_eq!(split_fragment("#anchor"), ("", Some("anchor")));
    }

    #[test]
    fn check_file_finds_broken_links() -> Result<()> {
        let tmp = TempDir::new()?;
        let md = tmp.path().join("test.md");
        fs::write(&md, "# Test\n\n[good](test.md)\n[bad](missing.md)\n")?;

        let broken = check_file(&md)?;
        anyhow::ensure!(
            matches!(broken.as_slice(), [link] if link.target == "missing.md" && link.line_number == 4),
            "unexpected broken links: {broken:?}"
        );
        Ok(())
    }

    #[test]
    fn check_file_skips_external_and_anchors() -> Result<()> {
        let tmp = TempDir::new()?;
        let md = tmp.path().join("test.md");
        fs::write(
            &md,
            "[ext](https://example.com)\n[anchor](#heading)\n[mailto](mailto:a@b.com)\n",
        )?;

        let broken = check_file(&md)?;
        anyhow::ensure!(broken.is_empty(), "unexpected broken links: {broken:?}");
        Ok(())
    }

    #[test]
    fn check_links_on_clean_dir() -> Result<()> {
        let tmp = TempDir::new()?;
        let docs = tmp.path().join("docs");
        fs::create_dir_all(&docs)?;
        fs::write(docs.join("readme.md"), "# Docs\n\n[self](readme.md)\n")?;

        // No root Cargo.toml with [workspace], but check_links just needs the root path
        check_links(tmp.path())?;
        Ok(())
    }

    #[test]
    fn check_links_reports_broken() -> Result<()> {
        let tmp = TempDir::new()?;
        let docs = tmp.path().join("docs");
        fs::create_dir_all(&docs)?;
        fs::write(docs.join("readme.md"), "[broken](nonexistent.md)\n")?;

        let error = check_links(tmp.path())
            .err()
            .context("expected a broken-link error")?;
        anyhow::ensure!(error.to_string().contains("1 broken link(s)"), "{error}");
        Ok(())
    }

    #[test]
    fn collect_markdown_files_finds_root_and_docs() -> Result<()> {
        let tmp = TempDir::new()?;
        fs::write(tmp.path().join("README.md"), "# Hi")?;
        let docs = tmp.path().join("docs");
        fs::create_dir_all(docs.join("sub"))?;
        fs::write(docs.join("guide.md"), "# Guide")?;
        fs::write(docs.join("sub").join("deep.md"), "# Deep")?;
        // Non-md file should be ignored
        fs::write(tmp.path().join("Cargo.toml"), "[package]")?;

        let files = collect_markdown_files(tmp.path());
        anyhow::ensure!(files.len() == 3, "unexpected files: {files:?}");
        Ok(())
    }
}
