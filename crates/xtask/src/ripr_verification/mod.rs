pub(crate) mod annotations;
pub(crate) mod badges;
pub(crate) mod impacted_evidence;
pub(crate) mod pr_evidence;
pub(crate) mod pr_evidence_summary;
pub(crate) mod review_comments;

use std::path::{Path, PathBuf};

pub(crate) fn repo_root() -> Result<PathBuf, String> {
    let mut current = Path::new(env!("CARGO_MANIFEST_DIR"));
    loop {
        let manifest = current.join("Cargo.toml");
        if manifest.exists() {
            let text = std::fs::read_to_string(&manifest)
                .map_err(|err| format!("read {}: {err}", manifest.display()))?;
            if text.contains("[workspace]") {
                return Ok(current.to_path_buf());
            }
        }
        current = current.parent().ok_or_else(|| {
            format!(
                "failed to find workspace root from {}",
                env!("CARGO_MANIFEST_DIR")
            )
        })?;
    }
}

pub fn badges(check: bool) -> Result<(), String> {
    badges::badges(check)
}

pub fn ripr_pr(args: &[String]) -> Result<(), String> {
    pr_evidence::ripr_pr(args)
}

pub fn ripr_review_comments(args: &[String]) -> Result<(), String> {
    review_comments::ripr_review_comments(args)
}

pub fn ripr_pr_summary(args: &[String]) -> Result<(), String> {
    pr_evidence_summary::ripr_pr_summary(args)
}

pub fn ripr_annotations(args: &[String]) -> Result<(), String> {
    annotations::ripr_annotations(args)
}

pub fn impacted_evidence(args: &[String]) -> Result<(), String> {
    impacted_evidence::impacted_evidence(args)
}
