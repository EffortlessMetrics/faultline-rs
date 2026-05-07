//! Roll-up of all policy gates into a single Markdown report.

use anyhow::Result;
use std::path::Path;

use super::report_dir;
use super::{file_policy::FilePolicyReport, lint_policy::LintReport, no_panic::NoPanicReport};

/// Write `target/policy/policy-report.md` summarising every gate.
pub fn write_roll_up(
    root: &Path,
    lint: &LintReport,
    panic: &NoPanicReport,
    files: &FilePolicyReport,
) -> Result<()> {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(s, "# Repository policy report\n");

    let _ = writeln!(s, "## Lint policy");
    let _ = writeln!(s, "- msrv: `{}`", lint.msrv);
    let _ = writeln!(s, "- active lints: {}", lint.active_lints);
    let _ = writeln!(s, "- planned lints: {}", lint.planned_lints);
    let _ = writeln!(s, "- crates checked: {}", lint.crates_checked);
    let _ = writeln!(s, "- findings: {}\n", lint.findings.len());

    let _ = writeln!(s, "## No-panic");
    let _ = writeln!(s, "- total findings: {}", panic.findings_total);
    let _ = writeln!(s, "- allowlist entries: {}", panic.allowlist_entries);
    let _ = writeln!(s, "- unallowlisted: {}", panic.findings_unallowlisted.len());
    let _ = writeln!(s, "- expired: {}", panic.allowlist_expired.len());
    let _ = writeln!(s, "- unused: {}\n", panic.allowlist_unused.len());

    let _ = writeln!(s, "## File policy");
    let _ = writeln!(s, "- tracked files: {}", files.tracked_files);
    let _ = writeln!(s, "- non-Rust files: {}", files.non_rust_files);
    let _ = writeln!(s, "- allowlist entries: {}", files.allowlist_entries);
    let _ = writeln!(s, "- unallowlisted: {}", files.unallowlisted.len());
    let _ = writeln!(s, "- expired: {}", files.expired.len());
    let _ = writeln!(s, "- unused: {}\n", files.unused.len());

    let dir = report_dir(root)?;
    std::fs::write(dir.join("policy-report.md"), s)?;
    println!(
        "policy-report -> {}",
        dir.join("policy-report.md").display()
    );
    Ok(())
}
