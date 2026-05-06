//! Effortless Metrics Rust policy stack — checkers and report writers.
//!
//! This module owns four policy gates:
//!
//! - [`check_lint_policy`]: cross-checks `policy/clippy-lints.toml` against
//!   the workspace `[lints]` table and verifies lint inheritance on every
//!   crate.
//! - [`check_no_panic_family`]: scans Rust sources for panic-family call
//!   sites and matches them against `policy/no-panic-allowlist.toml`.
//! - [`no_panic_propose`]: emits a proposed allowlist TOML for review,
//!   never mutating the source-of-truth file.
//! - [`check_file_policy`]: enforces the non-Rust file allowlist.
//!
//! [`policy_report`] rolls up the per-gate reports into a single document.
//!
//! Design principle: the semantic checker (no-panic-family) is the
//! authoritative exception mechanism. Clippy is the source-shape detector.
//! See `docs/CLIPPY_POLICY.md` and `docs/NO_PANIC_POLICY.md`.

pub mod file_policy;
pub mod lint_policy;
pub mod no_panic;
pub mod report;

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Locate the workspace root by walking up from `CARGO_MANIFEST_DIR` until a
/// `Cargo.toml` containing `[workspace]` is found.
pub fn workspace_root() -> Result<PathBuf> {
    let start = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut cur = Some(start);
    while let Some(dir) = cur {
        let manifest = dir.join("Cargo.toml");
        if manifest.exists() {
            let text = std::fs::read_to_string(&manifest)
                .map_err(|e| anyhow::anyhow!("read {}: {e}", manifest.display()))?;
            if text.contains("[workspace]") {
                return Ok(dir.to_path_buf());
            }
        }
        cur = dir.parent();
    }
    anyhow::bail!("workspace root not found above {}", start.display())
}

/// Output directory under `target/policy/` for generated reports.
pub fn report_dir(root: &Path) -> Result<PathBuf> {
    let dir = root.join("target").join("policy");
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .map_err(|e| anyhow::anyhow!("create {}: {e}", dir.display()))?;
    }
    Ok(dir)
}

/// Run every policy gate. Each individual check writes its own report under
/// `target/policy/`. The roll-up report is written last.
pub fn check_all(root: &Path) -> Result<()> {
    let lint = lint_policy::check(root)?;
    let panic = no_panic::check(root)?;
    let files = file_policy::check(root)?;
    report::write_roll_up(root, &lint, &panic, &files)?;
    Ok(())
}

pub use file_policy::check as check_file_policy;
pub use lint_policy::check as check_lint_policy;
pub use no_panic::{check as check_no_panic_family, propose as no_panic_propose};
pub use report::write_roll_up as policy_report;
