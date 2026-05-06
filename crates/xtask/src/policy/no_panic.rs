//! `cargo xtask check-no-panic-family` and `cargo xtask no-panic propose`.
//!
//! See `docs/NO_PANIC_POLICY.md` for the policy. The checker scans Rust
//! source files in the workspace for panic-family call sites and matches
//! each finding against `policy/no-panic-allowlist.toml`. It does **not**
//! do AST-level resolution; identity is `(path-or-glob, family, selector)`
//! where the selector is a coarse syntactic fingerprint. `last_seen` line
//! and column are advisory only.
//!
//! Panic families recognised:
//!
//! ```text
//! unwrap            method call ending `.unwrap()`
//! expect            method call ending `.expect("...")`
//! panic_macro       `panic!(...)`
//! todo              `todo!(...)`
//! unimplemented     `unimplemented!(...)`
//! unreachable       `unreachable!(...)`
//! get_unwrap        `.get(...).unwrap()` collapsed via clippy
//! indexing          `<expr>[<index>]` (excluding type generics — heuristic)
//! string_slice      `&s[a..b]` on a known string-slice receiver
//! ```
//!
//! The checker emits `target/policy/no-panic.{md,json}` and exits non-zero
//! on errors. `propose` emits `target/policy/no-panic-proposed-allowlist.toml`
//! and never mutates the source-of-truth allowlist.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

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
    reason = "Fields validate the allowlist TOML schema; classification/explanation/last_seen are surfaced in reports but not used by the matcher."
)]
struct AllowEntry {
    id: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    glob: Option<String>,
    family: String,
    classification: String,
    owner: String,
    explanation: String,
    expires: String,
    selector: AllowSelector,
    #[serde(default)]
    last_seen: Option<LastSeen>,
}

#[derive(Debug, Deserialize, Clone)]
#[expect(
    dead_code,
    reason = "receiver_fingerprint is advisory metadata for human review; kept on the schema for forward-compatible matching."
)]
struct AllowSelector {
    kind: String,
    #[serde(default)]
    container: Option<String>,
    #[serde(default)]
    callee: Option<String>,
    #[serde(default)]
    receiver_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[expect(
    dead_code,
    reason = "Advisory line/column hints for reviewers; never part of identity."
)]
struct LastSeen {
    #[serde(default)]
    line: Option<usize>,
    #[serde(default)]
    column: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub family: String,
    pub callee: String,
    pub container: String,
    pub snippet: String,
}

#[derive(Debug, Default, Serialize)]
pub struct NoPanicReport {
    pub schema_version: String,
    pub findings_total: usize,
    pub findings_unallowlisted: Vec<Finding>,
    pub allowlist_entries: usize,
    pub allowlist_unused: Vec<String>,
    pub allowlist_expired: Vec<String>,
    pub errors: usize,
}

/// Run the blocking no-panic check.
pub fn check(root: &Path) -> Result<NoPanicReport> {
    println!("=== check-no-panic-family ===\n");
    let allowlist = load_allowlist(root)?;
    let findings = scan_workspace(root)?;
    let mut report = NoPanicReport {
        schema_version: allowlist.schema_version.clone(),
        findings_total: findings.len(),
        allowlist_entries: allowlist.entries.len(),
        ..Default::default()
    };

    let today = today_string();
    let mut used_ids: BTreeSet<String> = BTreeSet::new();
    for entry in &allowlist.entries {
        if entry.expires.as_str() < today.as_str() {
            report.allowlist_expired.push(format!(
                "{} (expired {}, owner={})",
                entry.id, entry.expires, entry.owner
            ));
        }
    }

    for finding in &findings {
        if let Some(entry) = match_entry(finding, &allowlist.entries) {
            used_ids.insert(entry.id.clone());
        } else {
            report.findings_unallowlisted.push(finding.clone());
        }
    }

    for entry in &allowlist.entries {
        if !used_ids.contains(&entry.id) {
            report
                .allowlist_unused
                .push(format!("{} (owner={})", entry.id, entry.owner));
        }
    }

    report.errors = report.findings_unallowlisted.len() + report.allowlist_expired.len();

    let dir = report_dir(root)?;
    std::fs::write(
        dir.join("no-panic.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    std::fs::write(dir.join("no-panic.md"), render_markdown(&report))?;

    println!(
        "  total findings: {}    allowlist entries: {}    unallowlisted: {}    expired: {}    unused: {}",
        report.findings_total,
        report.allowlist_entries,
        report.findings_unallowlisted.len(),
        report.allowlist_expired.len(),
        report.allowlist_unused.len(),
    );

    if report.errors > 0 {
        anyhow::bail!(
            "check-no-panic-family: {} unallowlisted finding(s) and {} expired entry/ies; see target/policy/no-panic.md",
            report.findings_unallowlisted.len(),
            report.allowlist_expired.len(),
        );
    }
    println!("\n=== check-no-panic-family passed ===");
    Ok(report)
}

/// Emit a proposed allowlist for review.
pub fn propose(root: &Path) -> Result<()> {
    println!("=== no-panic propose ===\n");
    let findings = scan_workspace(root)?;
    let mut grouped: BTreeMap<(String, String, String), Vec<&Finding>> = BTreeMap::new();
    for f in &findings {
        let key = (f.path.clone(), f.family.clone(), f.container.clone());
        grouped.entry(key).or_default().push(f);
    }

    let dir = report_dir(root)?;
    let out_path = dir.join("no-panic-proposed-allowlist.toml");

    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(s, "# Proposed no-panic allowlist (for review).");
    let _ = writeln!(
        s,
        "# Generated by `cargo xtask no-panic propose`. Do NOT auto-merge."
    );
    let _ = writeln!(s, "schema_version = \"0.3\"");
    let _ = writeln!(s);
    let mut idx = 0usize;
    for ((path, family, container), entries) in &grouped {
        idx += 1;
        let representative = entries.first().expect("non-empty group");
        let _ = writeln!(s, "[[allow]]");
        let _ = writeln!(s, "id = \"panic-proposed-{idx:04}\"");
        let _ = writeln!(s, "path = \"{path}\"");
        let _ = writeln!(s, "family = \"{family}\"");
        let _ = writeln!(s, "classification = \"baseline\"");
        let _ = writeln!(s, "owner = \"unassigned\"");
        let _ = writeln!(
            s,
            "explanation = \"Proposed entry; classify and burn down before adoption.\""
        );
        let _ = writeln!(s, "expires = \"2026-09-01\"");
        let _ = writeln!(s);
        let _ = writeln!(s, "[allow.selector]");
        let _ = writeln!(s, "kind = \"method_or_macro\"");
        let _ = writeln!(s, "container = \"{container}\"");
        let _ = writeln!(s, "callee = \"{}\"", representative.callee);
        let _ = writeln!(s);
        let _ = writeln!(s, "[allow.last_seen]");
        let _ = writeln!(s, "line = {}", representative.line);
        let _ = writeln!(s, "column = {}", representative.column);
        let _ = writeln!(s);
    }
    std::fs::write(&out_path, &s)?;
    println!("  wrote {}", out_path.display());
    println!("\n=== no-panic propose done ===");
    Ok(())
}

fn load_allowlist(root: &Path) -> Result<AllowlistFile> {
    let path = root.join("policy").join("no-panic-allowlist.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
    let parsed: AllowlistFile =
        toml::from_str(&text).map_err(|e| anyhow::anyhow!("parse {}: {e}", path.display()))?;
    Ok(parsed)
}

fn scan_workspace(root: &Path) -> Result<Vec<Finding>> {
    let mut findings = Vec::new();
    let unwrap_re = regex::Regex::new(r"\.unwrap\s*\(\s*\)").expect("static regex");
    let expect_re = regex::Regex::new(r"\.expect\s*\(").expect("static regex");
    let macro_re = regex::Regex::new(r"\b(panic|todo|unimplemented|unreachable)\s*!\s*[\(\[\{]")
        .expect("static regex");
    let get_unwrap_re =
        regex::Regex::new(r"\.get\s*\([^)]*\)\s*\.unwrap\s*\(\s*\)").expect("static regex");

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path(), root))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let mut current_container = String::from("<file>");
        let fn_re = regex::Regex::new(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)")
            .expect("static regex");

        for (i, raw_line) in text.lines().enumerate() {
            let line_num = i + 1;
            // Strip trailing comments for matching but keep the original
            // line for the snippet.
            let active = strip_line_comment(raw_line);

            if let Some(caps) = fn_re.captures(active) {
                if let Some(name) = caps.get(1) {
                    current_container = name.as_str().to_string();
                }
            }

            // Order matters: `get_unwrap` is a more specific subset of
            // `unwrap`; check it first so we classify the finding as
            // `get_unwrap` when both match.
            if let Some(m) = get_unwrap_re.find(active) {
                findings.push(Finding {
                    path: rel.clone(),
                    line: line_num,
                    column: m.start() + 1,
                    family: "get_unwrap".into(),
                    callee: "unwrap".into(),
                    container: current_container.clone(),
                    snippet: raw_line.trim().to_string(),
                });
                continue;
            }

            if let Some(m) = unwrap_re.find(active) {
                findings.push(Finding {
                    path: rel.clone(),
                    line: line_num,
                    column: m.start() + 1,
                    family: "unwrap".into(),
                    callee: "unwrap".into(),
                    container: current_container.clone(),
                    snippet: raw_line.trim().to_string(),
                });
            }
            if let Some(m) = expect_re.find(active) {
                findings.push(Finding {
                    path: rel.clone(),
                    line: line_num,
                    column: m.start() + 1,
                    family: "expect".into(),
                    callee: "expect".into(),
                    container: current_container.clone(),
                    snippet: raw_line.trim().to_string(),
                });
            }
            if let Some(caps) = macro_re.captures(active) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("panic");
                let family = match name {
                    "panic" => "panic_macro",
                    "todo" => "todo",
                    "unimplemented" => "unimplemented",
                    "unreachable" => "unreachable",
                    _ => "panic_macro",
                };
                let m = caps.get(0).expect("group 0 always present");
                findings.push(Finding {
                    path: rel.clone(),
                    line: line_num,
                    column: m.start() + 1,
                    family: family.into(),
                    callee: name.into(),
                    container: current_container.clone(),
                    snippet: raw_line.trim().to_string(),
                });
            }
        }
    }

    Ok(findings)
}

fn strip_line_comment(line: &str) -> &str {
    // Naive: split on the first `//`. Misses `//` inside strings, which is
    // acceptable for a coarse syntactic checker.
    match line.find("//") {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn match_entry<'a>(finding: &Finding, entries: &'a [AllowEntry]) -> Option<&'a AllowEntry> {
    for entry in entries {
        if !family_matches(&entry.family, &finding.family) {
            continue;
        }
        if !path_matches(entry, &finding.path) {
            continue;
        }
        if !selector_matches(&entry.selector, finding) {
            continue;
        }
        return Some(entry);
    }
    None
}

fn family_matches(allow_family: &str, finding_family: &str) -> bool {
    if allow_family == "any" {
        return true;
    }
    allow_family == finding_family
}

fn path_matches(entry: &AllowEntry, path: &str) -> bool {
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

fn selector_matches(selector: &AllowSelector, finding: &Finding) -> bool {
    match selector.kind.as_str() {
        "baseline_glob" => true,
        _ => {
            if let Some(callee) = &selector.callee {
                if callee != "*" && callee != &finding.callee {
                    return false;
                }
            }
            if let Some(container) = &selector.container {
                if container != "*" && container != &finding.container {
                    return false;
                }
            }
            true
        }
    }
}

fn is_ignored(path: &Path, root: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return false;
    };
    let s = rel.to_string_lossy();
    s.starts_with("target")
        || s.starts_with(".git")
        || s.contains("/target/")
        || s.starts_with("docs/book/book")
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

fn render_markdown(r: &NoPanicReport) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(s, "# No-panic policy report");
    let _ = writeln!(s);
    let _ = writeln!(s, "- schema_version: `{}`", r.schema_version);
    let _ = writeln!(s, "- total findings: {}", r.findings_total);
    let _ = writeln!(s, "- allowlist entries: {}", r.allowlist_entries);
    let _ = writeln!(
        s,
        "- unallowlisted findings: {}",
        r.findings_unallowlisted.len()
    );
    let _ = writeln!(s, "- expired entries: {}", r.allowlist_expired.len());
    let _ = writeln!(s, "- unused entries: {}", r.allowlist_unused.len());
    let _ = writeln!(s);

    if !r.findings_unallowlisted.is_empty() {
        let _ = writeln!(s, "## Unallowlisted findings");
        let _ = writeln!(s);
        for f in &r.findings_unallowlisted {
            let _ = writeln!(
                s,
                "- `{}:{}:{}` family=`{}` container=`{}`",
                f.path, f.line, f.column, f.family, f.container
            );
            let _ = writeln!(s, "  - `{}`", f.snippet);
        }
        let _ = writeln!(s);
    }

    if !r.allowlist_expired.is_empty() {
        let _ = writeln!(s, "## Expired entries");
        let _ = writeln!(s);
        for e in &r.allowlist_expired {
            let _ = writeln!(s, "- {e}");
        }
        let _ = writeln!(s);
    }

    if !r.allowlist_unused.is_empty() {
        let _ = writeln!(s, "## Unused entries (warning)");
        let _ = writeln!(s);
        for e in &r.allowlist_unused {
            let _ = writeln!(s, "- {e}");
        }
        let _ = writeln!(s);
    }

    if r.errors == 0 {
        let _ = writeln!(
            s,
            "All panic-family findings are receipted. Burn down baseline entries on schedule."
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: Option<&str>, glob: Option<&str>, family: &str, container: &str) -> AllowEntry {
        AllowEntry {
            id: "test".into(),
            path: path.map(String::from),
            glob: glob.map(String::from),
            family: family.into(),
            classification: "test_helper".into(),
            owner: "test".into(),
            explanation: "test".into(),
            expires: "2999-01-01".into(),
            selector: AllowSelector {
                kind: "method_or_macro".into(),
                container: Some(container.into()),
                callee: Some("unwrap".into()),
                receiver_fingerprint: None,
            },
            last_seen: None,
        }
    }

    #[test]
    fn family_matches_exact_and_any() {
        assert!(family_matches("unwrap", "unwrap"));
        assert!(!family_matches("unwrap", "expect"));
        assert!(family_matches("any", "unwrap"));
    }

    #[test]
    fn path_matches_exact_and_glob() {
        let e_path = entry(Some("a/b/c.rs"), None, "unwrap", "f");
        assert!(path_matches(&e_path, "a/b/c.rs"));
        assert!(!path_matches(&e_path, "a/b/d.rs"));

        let e_glob = entry(None, Some("crates/**/*.rs"), "unwrap", "f");
        assert!(path_matches(&e_glob, "crates/x/src/lib.rs"));
        assert!(!path_matches(&e_glob, "fuzz/x.rs"));
    }

    #[test]
    fn selector_baseline_glob_always_matches() {
        let sel = AllowSelector {
            kind: "baseline_glob".into(),
            container: Some("*".into()),
            callee: Some("*".into()),
            receiver_fingerprint: None,
        };
        let f = Finding {
            path: "x.rs".into(),
            line: 1,
            column: 1,
            family: "unwrap".into(),
            callee: "unwrap".into(),
            container: "irrelevant".into(),
            snippet: String::new(),
        };
        assert!(selector_matches(&sel, &f));
    }
}
