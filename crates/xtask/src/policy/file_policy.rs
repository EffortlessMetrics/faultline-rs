//! `cargo xtask check-file-policy` — non-Rust file allowlist gate.
//!
//! See `docs/FILE_POLICY.md`. Enumerates git-tracked files, ignores `*.rs`
//! and `*.md`, and matches everything else against
//! `policy/non-rust-allowlist.toml`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use super::report_dir;

#[derive(Debug, Deserialize)]
struct AllowlistFile {
    schema_version: String,
    #[serde(default)]
    #[serde(rename = "allow")]
    entries: Vec<AllowEntry>,
}

#[derive(Debug, Deserialize, Clone)]
#[expect(
    dead_code,
    reason = "Fields validate the allowlist TOML schema and surface in reports; not all are consumed by the matcher today."
)]
struct AllowEntry {
    #[serde(default)]
    glob: Option<String>,
    #[serde(default)]
    path: Option<String>,
    kind: String,
    owner: String,
    surface: String,
    classification: String,
    reason: String,
    #[serde(default)]
    covered_by: Vec<String>,
    #[serde(default)]
    expires: Option<String>,
    #[serde(default)]
    generated_by: Option<String>,
    #[serde(default)]
    retired: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct FilePolicyReport {
    pub schema_version: String,
    pub tracked_files: usize,
    pub non_rust_files: usize,
    pub allowlist_entries: usize,
    pub unallowlisted: Vec<String>,
    pub expired: Vec<String>,
    pub unused: Vec<String>,
    pub errors: usize,
}

pub fn check(root: &Path) -> Result<FilePolicyReport> {
    println!("=== check-file-policy ===\n");
    let allowlist = load(root)?;
    let tracked = git_ls_files(root)?;
    let mut report = FilePolicyReport {
        schema_version: allowlist.schema_version.clone(),
        tracked_files: tracked.len(),
        allowlist_entries: allowlist.entries.len(),
        ..Default::default()
    };

    let today = today_string();
    for entry in &allowlist.entries {
        if let Some(exp) = &entry.expires {
            if exp.as_str() < today.as_str() {
                let label = entry
                    .glob
                    .clone()
                    .or_else(|| entry.path.clone())
                    .unwrap_or_else(|| "<unknown>".into());
                report.expired.push(format!(
                    "{label} (expired {exp}, owner={}, surface={})",
                    entry.owner, entry.surface
                ));
            }
        }
    }

    let mut used: BTreeSet<usize> = BTreeSet::new();
    for file in &tracked {
        if is_rust_or_markdown(file) {
            continue;
        }
        report.non_rust_files += 1;
        let mut matched = false;
        for (i, entry) in allowlist.entries.iter().enumerate() {
            if entry_matches(entry, file) {
                used.insert(i);
                matched = true;
            }
        }
        if !matched {
            report.unallowlisted.push(file.clone());
        }
    }

    for (i, entry) in allowlist.entries.iter().enumerate() {
        if used.contains(&i) {
            continue;
        }
        if entry.retired {
            continue;
        }
        let label = entry
            .glob
            .clone()
            .or_else(|| entry.path.clone())
            .unwrap_or_else(|| "<unknown>".into());
        report.unused.push(format!(
            "{label} (owner={}, surface={}, classification={}, reason={}, generated_by={})",
            entry.owner,
            entry.surface,
            entry.classification,
            entry.reason,
            entry.generated_by.as_deref().unwrap_or("-"),
        ));
    }

    report.errors = report.unallowlisted.len() + report.expired.len() + report.unused.len();

    let dir = report_dir(root)?;
    std::fs::write(
        dir.join("file-policy.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    std::fs::write(dir.join("file-policy.md"), render_markdown(&report))?;

    println!(
        "  tracked={}  non-rust={}  allow-entries={}  unallowlisted={}  expired={}  unused={}",
        report.tracked_files,
        report.non_rust_files,
        report.allowlist_entries,
        report.unallowlisted.len(),
        report.expired.len(),
        report.unused.len(),
    );

    if report.errors > 0 {
        anyhow::bail!(
            "check-file-policy: {} unallowlisted, {} expired, {} unused; see target/policy/file-policy.md",
            report.unallowlisted.len(),
            report.expired.len(),
            report.unused.len(),
        );
    }
    println!("\n=== check-file-policy passed ===");
    Ok(report)
}

fn load(root: &Path) -> Result<AllowlistFile> {
    let path = root.join("policy").join("non-rust-allowlist.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
    let parsed: AllowlistFile =
        toml::from_str(&text).map_err(|e| anyhow::anyhow!("parse {}: {e}", path.display()))?;
    Ok(parsed)
}

fn git_ls_files(root: &Path) -> Result<Vec<String>> {
    let out = Command::new("git")
        .arg("ls-files")
        .current_dir(root)
        .output()
        .map_err(|e| anyhow::anyhow!("git ls-files: {e}"))?;
    if !out.status.success() {
        anyhow::bail!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let s = String::from_utf8_lossy(&out.stdout);
    Ok(s.lines().map(|l| l.to_string()).collect())
}

fn is_rust_or_markdown(path: &str) -> bool {
    path.ends_with(".rs") || path.ends_with(".md")
}

fn entry_matches(entry: &AllowEntry, path: &str) -> bool {
    if let Some(p) = &entry.path {
        return p == path;
    }
    if let Some(g) = &entry.glob {
        if let Ok(pattern) = glob::Pattern::new(g) {
            return pattern.matches(path);
        }
    }
    false
}

fn today_string() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let (y, m, d) = days_to_ymd(days as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

fn render_markdown(r: &FilePolicyReport) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(s, "# File policy report");
    let _ = writeln!(s);
    let _ = writeln!(s, "- schema_version: `{}`", r.schema_version);
    let _ = writeln!(s, "- tracked files: {}", r.tracked_files);
    let _ = writeln!(s, "- non-Rust/Markdown files: {}", r.non_rust_files);
    let _ = writeln!(s, "- allowlist entries: {}", r.allowlist_entries);
    let _ = writeln!(s, "- unallowlisted: {}", r.unallowlisted.len());
    let _ = writeln!(s, "- expired entries: {}", r.expired.len());
    let _ = writeln!(s, "- unused entries: {}", r.unused.len());
    let _ = writeln!(s);

    if !r.unallowlisted.is_empty() {
        let _ = writeln!(s, "## Unallowlisted files");
        let _ = writeln!(s);
        for p in &r.unallowlisted {
            let _ = writeln!(s, "- `{p}`");
        }
        let _ = writeln!(s);
    }

    if !r.expired.is_empty() {
        let _ = writeln!(s, "## Expired entries");
        let _ = writeln!(s);
        for e in &r.expired {
            let _ = writeln!(s, "- {e}");
        }
        let _ = writeln!(s);
    }

    if !r.unused.is_empty() {
        let _ = writeln!(s, "## Unused entries");
        let _ = writeln!(s);
        for e in &r.unused {
            let _ = writeln!(s, "- {e}");
        }
        let _ = writeln!(s);
    }

    if r.errors == 0 {
        let _ = writeln!(s, "All non-Rust files are receipted.");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(glob: Option<&str>, path: Option<&str>) -> AllowEntry {
        AllowEntry {
            glob: glob.map(String::from),
            path: path.map(String::from),
            kind: "k".into(),
            owner: "o".into(),
            surface: "s".into(),
            classification: "c".into(),
            reason: "r".into(),
            covered_by: vec![],
            expires: None,
            generated_by: None,
            retired: false,
        }
    }

    #[test]
    fn rust_and_markdown_excluded() {
        assert!(is_rust_or_markdown("src/lib.rs"));
        assert!(is_rust_or_markdown("README.md"));
        assert!(!is_rust_or_markdown("Cargo.toml"));
    }

    #[test]
    fn entry_path_and_glob_matching() {
        let e_p = entry(None, Some("Cargo.toml"));
        assert!(entry_matches(&e_p, "Cargo.toml"));
        assert!(!entry_matches(&e_p, "Cargo.lock"));

        let e_g = entry(Some("crates/*/Cargo.toml"), None);
        assert!(entry_matches(&e_g, "crates/foo/Cargo.toml"));
        assert!(!entry_matches(&e_g, "Cargo.toml"));
    }
}
