//! `cargo xtask check-lint-policy` — verify the workspace lint posture.
//!
//! Reads `policy/clippy-lints.toml` and verifies:
//!
//! 1. workspace `package.rust-version` matches `policy.msrv`,
//! 2. every active lint is wired into the workspace `[lints]` table,
//! 3. every crate manifest sets `[lints] workspace = true`,
//! 4. no `clippy.toml` test carveouts are present,
//! 5. no bare `#[allow(...)]` attributes exist in tracked Rust sources,
//! 6. planned lints are NOT activated before their `activate_when_msrv`,
//! 7. debt entries (in `policy/clippy-debt.toml`) are not expired.
//!
//! Writes `target/policy/lint-policy.{md,json}`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use super::report_dir;

#[derive(Debug, Deserialize)]
struct PolicyFile {
    schema_version: String,
    msrv: String,
    #[serde(default)]
    policy: PolicyFlags,
    #[serde(default)]
    active: Vec<ActiveLint>,
    #[serde(default)]
    planned: Vec<PlannedLint>,
}

#[derive(Debug, Default, Deserialize)]
#[expect(
    dead_code,
    reason = "Fields validate the policy schema; only `allow_test_carveouts` is consumed today, the rest are reserved for future gates."
)]
struct PolicyFlags {
    #[serde(default)]
    panic_free_tests: bool,
    #[serde(default)]
    allow_test_carveouts: bool,
    #[serde(default)]
    suppression_style: Option<String>,
    #[serde(default)]
    blanket_categories: bool,
}

#[derive(Debug, Deserialize)]
struct ActiveLint {
    name: String,
    level: String,
}

#[derive(Debug, Deserialize)]
struct PlannedLint {
    name: String,
    #[expect(
        dead_code,
        reason = "Captured for the policy report; not consumed by the checker."
    )]
    level: String,
    activate_when_msrv: String,
    #[expect(
        dead_code,
        reason = "Captured for the policy report; not consumed by the checker."
    )]
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DebtFile {
    #[serde(default)]
    debt: Vec<DebtEntry>,
}

#[derive(Debug, Deserialize)]
struct DebtEntry {
    id: String,
    lint: String,
    owner: String,
    reason: String,
    expires: String,
}

#[derive(Debug, Default, Serialize)]
pub struct LintReport {
    pub schema_version: String,
    pub msrv: String,
    pub active_lints: usize,
    pub planned_lints: usize,
    pub crates_checked: usize,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Serialize)]
pub struct Finding {
    pub severity: String,
    pub kind: String,
    pub message: String,
}

impl LintReport {
    fn fail(&mut self, kind: &str, msg: impl Into<String>) {
        self.findings.push(Finding {
            severity: "error".into(),
            kind: kind.into(),
            message: msg.into(),
        });
    }
    fn warn(&mut self, kind: &str, msg: impl Into<String>) {
        self.findings.push(Finding {
            severity: "warn".into(),
            kind: kind.into(),
            message: msg.into(),
        });
    }
    fn errors(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == "error")
            .count()
    }
}

/// Run the lint-policy check. Writes reports under `target/policy/`.
pub fn check(root: &Path) -> Result<LintReport> {
    println!("=== check-lint-policy ===\n");
    let policy_path = root.join("policy").join("clippy-lints.toml");
    let policy_text = std::fs::read_to_string(&policy_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", policy_path.display()))?;
    let policy: PolicyFile = toml::from_str(&policy_text)
        .map_err(|e| anyhow::anyhow!("parse {}: {e}", policy_path.display()))?;

    let mut report = LintReport {
        schema_version: policy.schema_version.clone(),
        msrv: policy.msrv.clone(),
        active_lints: policy.active.len(),
        planned_lints: policy.planned.len(),
        crates_checked: 0,
        findings: Vec::new(),
    };

    let root_manifest_text = std::fs::read_to_string(root.join("Cargo.toml"))?;
    let root_manifest: toml::Value = toml::from_str(&root_manifest_text)?;

    // 1. MSRV alignment.
    let workspace_rust_version = root_manifest
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("rust-version"))
        .and_then(|v| v.as_str());
    match workspace_rust_version {
        Some(v) if v == policy.msrv => {}
        Some(v) => report.fail(
            "msrv_mismatch",
            format!(
                "policy/clippy-lints.toml msrv={} but Cargo.toml workspace.package.rust-version={}",
                policy.msrv, v
            ),
        ),
        None => report.fail(
            "msrv_missing",
            "Cargo.toml [workspace.package] is missing rust-version".to_string(),
        ),
    }

    // 2. Workspace [lints] table coverage of active lints.
    let lints_table = root_manifest.get("workspace").and_then(|w| w.get("lints"));
    let workspace_lints = collect_workspace_lints(lints_table);
    for active in &policy.active {
        let key = active.name.clone();
        match workspace_lints.get(&key) {
            None => report.fail(
                "lint_missing",
                format!("policy active lint not wired into [workspace.lints]: {key}"),
            ),
            Some(level) if level != &active.level => report.warn(
                "lint_level_drift",
                format!(
                    "{key}: policy={} cargo={} (drift; reconcile policy/ vs Cargo.toml)",
                    active.level, level
                ),
            ),
            _ => {}
        }
    }

    // 3. Planned lints must NOT be present in [workspace.lints] when MSRV
    //    is below their activate_when_msrv.
    let current_msrv = parse_msrv(&policy.msrv);
    for planned in &policy.planned {
        let target = parse_msrv(&planned.activate_when_msrv);
        if current_msrv < target && workspace_lints.contains_key(&planned.name) {
            report.fail(
                "planned_lint_active_early",
                format!(
                    "{} is planned for MSRV {} but already wired at MSRV {}",
                    planned.name, planned.activate_when_msrv, policy.msrv
                ),
            );
        }
    }

    // 4. Per-crate lint inheritance.
    let members = root_manifest
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    for member in &members {
        let manifest = root.join(member).join("Cargo.toml");
        if !manifest.exists() {
            report.fail(
                "missing_manifest",
                format!("workspace member missing Cargo.toml: {member}"),
            );
            continue;
        }
        report.crates_checked += 1;
        let text = std::fs::read_to_string(&manifest)?;
        let parsed: toml::Value = toml::from_str(&text)?;
        let lints = parsed.get("lints");
        let inherits = lints
            .and_then(|l| l.get("workspace"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !inherits {
            report.fail(
                "lints_not_inherited",
                format!("{member}/Cargo.toml is missing `[lints] workspace = true`"),
            );
        }
    }

    // 5. clippy.toml test carveout knobs.
    let carveout_keys = [
        "allow-unwrap-in-tests",
        "allow-expect-in-tests",
        "allow-panic-in-tests",
        "allow-indexing-slicing-in-tests",
        "allow-dbg-in-tests",
    ];
    let clippy_toml = root.join("clippy.toml");
    if clippy_toml.exists() && !policy.policy.allow_test_carveouts {
        let text = std::fs::read_to_string(&clippy_toml)?;
        for k in &carveout_keys {
            if text.contains(k) {
                report.fail(
                    "test_carveout_present",
                    format!("clippy.toml contains forbidden carveout: {k}"),
                );
            }
        }
    }

    // 6. Bare #[allow(...)] in source files. Anchor to line-leading
    // whitespace so we only flag actual attributes, not string literals
    // that happen to contain the substring.
    let allow_re = regex::Regex::new(r"^\s*#!?\[allow\(").expect("static regex compiles");
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !is_ignored_dir(e.path(), root))
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
        for (i, line) in text.lines().enumerate() {
            if allow_re.is_match(line) {
                let rel = path.strip_prefix(root).unwrap_or(path);
                report.fail(
                    "bare_allow_attribute",
                    format!(
                        "{}:{}: bare allow attribute (use the receipted form instead)",
                        rel.display(),
                        i + 1
                    ),
                );
            }
        }
    }

    // 7. Debt entries: parse and check expiries.
    let debt_path = root.join("policy").join("clippy-debt.toml");
    if debt_path.exists() {
        let debt_text = std::fs::read_to_string(&debt_path)?;
        let debt: DebtFile = toml::from_str(&debt_text).unwrap_or(DebtFile { debt: Vec::new() });
        let today = today_string();
        for entry in &debt.debt {
            if entry.expires.as_str() < today.as_str() {
                report.fail(
                    "debt_expired",
                    format!(
                        "{}: debt entry {} expired {} (lint={}, owner={}, reason={})",
                        debt_path.display(),
                        entry.id,
                        entry.expires,
                        entry.lint,
                        entry.owner,
                        entry.reason,
                    ),
                );
            }
        }
    }

    // Write reports.
    let dir = report_dir(root)?;
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(dir.join("lint-policy.json"), json)?;
    std::fs::write(dir.join("lint-policy.md"), render_markdown(&report))?;

    println!(
        "  msrv={} active={} planned={} crates={} findings={}",
        report.msrv,
        report.active_lints,
        report.planned_lints,
        report.crates_checked,
        report.findings.len()
    );
    for f in &report.findings {
        println!("  [{}] {}: {}", f.severity, f.kind, f.message);
    }

    if report.errors() > 0 {
        anyhow::bail!(
            "check-lint-policy: {} error(s); see target/policy/lint-policy.md",
            report.errors()
        );
    }
    println!("\n=== check-lint-policy passed ===");
    Ok(report)
}

fn collect_workspace_lints(lints: Option<&toml::Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(table) = lints.and_then(|v| v.as_table()) else {
        return out;
    };
    for (group, value) in table {
        let Some(inner) = value.as_table() else {
            continue;
        };
        for (name, lvl) in inner {
            // Skip the `workspace = true` boolean in member crates; here we
            // are reading the workspace definition.
            let level = match lvl {
                toml::Value::String(s) => s.clone(),
                toml::Value::Table(t) => t
                    .get("level")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_default(),
                _ => continue,
            };
            let key = if group == "rust" || group == "rustdoc" {
                name.clone()
            } else {
                format!("{group}::{name}")
            };
            out.insert(key, level);
        }
    }
    out
}

fn is_ignored_dir(path: &Path, root: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return false;
    };
    let s = rel.to_string_lossy();
    s.starts_with("target")
        || s.starts_with(".git")
        || s.contains("/target/")
        || s == "docs/book/book"
        || s.starts_with("docs/book/book")
}

fn parse_msrv(s: &str) -> (u32, u32, u32) {
    let mut parts = s.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

fn today_string() -> String {
    // SystemTime-based YYYY-MM-DD without pulling in chrono.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let (y, m, d) = days_to_ymd(days as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

// Convert days since 1970-01-01 to (year, month, day).
fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    // Algorithm: Howard Hinnant's date algorithms (civil_from_days).
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

fn render_markdown(r: &LintReport) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(s, "# Lint policy report");
    let _ = writeln!(s);
    let _ = writeln!(s, "- schema_version: `{}`", r.schema_version);
    let _ = writeln!(s, "- msrv: `{}`", r.msrv);
    let _ = writeln!(s, "- active lints: {}", r.active_lints);
    let _ = writeln!(s, "- planned lints: {}", r.planned_lints);
    let _ = writeln!(s, "- crates checked: {}", r.crates_checked);
    let _ = writeln!(s, "- findings: {}", r.findings.len());
    let _ = writeln!(s);
    if r.findings.is_empty() {
        let _ = writeln!(s, "No findings. Policy is clean.");
        return s;
    }
    let _ = writeln!(s, "## Findings");
    let _ = writeln!(s);
    let mut grouped: BTreeMap<String, Vec<&Finding>> = BTreeMap::new();
    for f in &r.findings {
        grouped.entry(f.kind.clone()).or_default().push(f);
    }
    for (kind, fs) in grouped {
        let _ = writeln!(s, "### `{kind}` ({})", fs.len());
        let _ = writeln!(s);
        for f in fs {
            let _ = writeln!(s, "- **{}**: {}", f.severity, f.message);
        }
        let _ = writeln!(s);
    }
    s
}
