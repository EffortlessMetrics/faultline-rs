use faultline_ports::{CheckoutPort, HistoryPort};
use faultline_types::{
    ChangeStatus, CheckedOutRevision, CommitId, FaultlineError, HistoryMode, PathChange, Result,
    RevisionSequence, RevisionSpec,
};
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static WORKTREE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct GitAdapter {
    repo_root: PathBuf,
    scratch_root: PathBuf,
}

impl GitAdapter {
    pub fn new(repo_root: impl Into<PathBuf>) -> Result<Self> {
        let repo_root = repo_root.into();
        let scratch_root = repo_root.join(".faultline").join("scratch");
        fs::create_dir_all(&scratch_root)?;
        Ok(Self {
            repo_root,
            scratch_root,
        })
    }

    fn git_output(&self, args: Vec<OsString>) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_root)
            .output()
            .map_err(|err| FaultlineError::Git(err.to_string()))?;

        if !output.status.success() {
            return Err(FaultlineError::Git(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn git_status(&self, args: Vec<OsString>) -> Result<()> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_root)
            .output()
            .map_err(|err| FaultlineError::Git(err.to_string()))?;
        if !output.status.success() {
            return Err(FaultlineError::Git(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        Ok(())
    }

    fn resolve_revision(&self, spec: &RevisionSpec) -> Result<CommitId> {
        let resolved = self.git_output(vec![
            OsString::from("rev-parse"),
            OsString::from("--verify"),
            OsString::from(spec.0.clone()),
        ])?;
        Ok(CommitId(resolved))
    }

    fn ensure_ancestor(&self, good: &CommitId, bad: &CommitId) -> Result<()> {
        self.git_status(vec![
            OsString::from("merge-base"),
            OsString::from("--is-ancestor"),
            OsString::from(good.0.clone()),
            OsString::from(bad.0.clone()),
        ])
        .map_err(|_| {
            FaultlineError::InvalidInput(format!(
                "good revision {} must be an ancestor of bad revision {}",
                good.0, bad.0
            ))
        })
    }

    fn unique_worktree_path(&self, commit: &CommitId) -> PathBuf {
        let counter = WORKTREE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let short = commit.0.chars().take(12).collect::<String>();
        self.scratch_root
            .join(format!("{}-{}-{}", short, stamp, counter))
    }
}

impl HistoryPort for GitAdapter {
    fn linearize(
        &self,
        good: &RevisionSpec,
        bad: &RevisionSpec,
        mode: HistoryMode,
    ) -> Result<RevisionSequence> {
        let good_resolved = self.resolve_revision(good)?;
        let bad_resolved = self.resolve_revision(bad)?;
        self.ensure_ancestor(&good_resolved, &bad_resolved)?;

        let range = format!("{}..{}", good_resolved.0, bad_resolved.0);
        let mut args = vec![OsString::from("rev-list"), OsString::from("--reverse")];
        match mode {
            HistoryMode::AncestryPath => {
                args.push(OsString::from("--ancestry-path"));
            }
            HistoryMode::FirstParent => {
                args.push(OsString::from("--ancestry-path"));
                args.push(OsString::from("--first-parent"));
            }
        }
        args.push(OsString::from(range));

        let output = self.git_output(args)?;
        let mut revisions = vec![good_resolved.clone()];
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            revisions.push(CommitId(trimmed.to_string()));
        }
        if revisions.last() != Some(&bad_resolved) {
            revisions.push(bad_resolved);
        }

        if revisions.len() < 2 {
            return Err(FaultlineError::InvalidInput(
                "history range must contain at least a good and bad revision".to_string(),
            ));
        }

        Ok(RevisionSequence { revisions })
    }

    fn changed_paths(&self, from: &CommitId, to: &CommitId) -> Result<Vec<PathChange>> {
        let output = self.git_output(vec![
            OsString::from("diff"),
            OsString::from("--name-status"),
            OsString::from(from.0.clone()),
            OsString::from(to.0.clone()),
        ])?;
        let mut changes = Vec::new();
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parts: Vec<&str> = trimmed.split('\t').collect();
            let status_code = parts.first().copied().unwrap_or("");
            let status = if status_code.starts_with('A') {
                ChangeStatus::Added
            } else if status_code.starts_with('M') {
                ChangeStatus::Modified
            } else if status_code.starts_with('D') {
                ChangeStatus::Deleted
            } else if status_code.starts_with('R') {
                ChangeStatus::Renamed
            } else if status_code.starts_with('T') {
                ChangeStatus::TypeChanged
            } else {
                ChangeStatus::Unknown
            };
            let path = match status {
                ChangeStatus::Renamed => {
                    parts.get(2).or_else(|| parts.get(1)).copied().unwrap_or("")
                }
                _ => parts.get(1).copied().unwrap_or(""),
            };
            if !path.is_empty() {
                changes.push(PathChange {
                    status,
                    path: path.to_string(),
                });
            }
        }
        Ok(changes)
    }
}

impl CheckoutPort for GitAdapter {
    fn checkout_revision(&self, commit: &CommitId) -> Result<CheckedOutRevision> {
        let worktree_path = self.unique_worktree_path(commit);
        fs::create_dir_all(&self.scratch_root)?;
        self.git_status(vec![
            OsString::from("worktree"),
            OsString::from("add"),
            OsString::from("--detach"),
            OsString::from("--force"),
            worktree_path.as_os_str().to_os_string(),
            OsString::from(commit.0.clone()),
        ])?;
        Ok(CheckedOutRevision {
            commit: commit.clone(),
            path: worktree_path,
        })
    }

    fn cleanup_checkout(&self, checkout: &CheckedOutRevision) -> Result<()> {
        if checkout.path.exists() {
            let _ = self.git_status(vec![
                OsString::from("worktree"),
                OsString::from("remove"),
                OsString::from("--force"),
                checkout.path.as_os_str().to_os_string(),
            ]);
            if checkout.path.exists() {
                let _ = fs::remove_dir_all(&checkout.path);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: v01-release-train, Property 19: Worktree Path Uniqueness
    // **Validates: Requirements 9.4**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_worktree_path_uniqueness(
            sha_a in "[a-f0-9]{8,40}",
            sha_b in "[a-f0-9]{8,40}",
        ) {
            let tmp = tempfile::tempdir().expect("create temp dir");
            let adapter = GitAdapter {
                repo_root: tmp.path().to_path_buf(),
                scratch_root: tmp.path().join("scratch"),
            };

            let commit_a = CommitId(sha_a);
            let commit_b = CommitId(sha_b);

            let path_a = adapter.unique_worktree_path(&commit_a);
            let path_b = adapter.unique_worktree_path(&commit_b);

            prop_assert_ne!(path_a, path_b, "two calls to unique_worktree_path must return distinct paths, even for the same commit");
        }
    }
}
