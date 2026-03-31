use faultline_types::{CommitId, RevisionSequence};
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[derive(Debug, Default, Clone)]
pub struct RevisionSequenceBuilder {
    revisions: Vec<CommitId>,
}

impl RevisionSequenceBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(mut self, revision: impl Into<String>) -> Self {
        self.revisions.push(CommitId(revision.into()));
        self
    }

    pub fn build(self) -> RevisionSequence {
        RevisionSequence {
            revisions: self.revisions,
        }
    }

    /// Creates a sequence of `n` commits labeled "commit-0" through "commit-{n-1}".
    pub fn exact_boundary(n: usize) -> RevisionSequence {
        let revisions = (0..n).map(|i| CommitId(format!("commit-{}", i))).collect();
        RevisionSequence { revisions }
    }

    /// Creates a sequence from a list of labels.
    pub fn with_labels(labels: &[&str]) -> RevisionSequence {
        let revisions = labels.iter().map(|l| CommitId(l.to_string())).collect();
        RevisionSequence { revisions }
    }
}

// ---------------------------------------------------------------------------
// GitRepoBuilder — real temporary Git repositories for adapter-level tests
// ---------------------------------------------------------------------------

/// A file operation to apply within a single commit.
#[derive(Debug, Clone)]
pub enum FileOp {
    Write { path: String, content: String },
    Delete { path: String },
    Rename { from: String, to: String },
}

/// A single commit to be created by the builder.
#[derive(Debug, Clone)]
pub struct FixtureCommit {
    pub message: String,
    pub operations: Vec<FileOp>,
}

/// The result of building a fixture repository.
pub struct FixtureRepo {
    pub dir: TempDir,
    pub commits: Vec<CommitId>,
}

/// Describes a deferred action in the builder (either a normal commit or a merge).
enum BuilderAction {
    Commit(FixtureCommit),
    Merge { message: String, branch: String },
}

/// Builder that creates a real temporary Git repository with real commits.
pub struct GitRepoBuilder {
    dir: TempDir,
    actions: Vec<BuilderAction>,
}

impl GitRepoBuilder {
    /// Create a new builder with an initialised Git repository.
    pub fn new() -> Result<Self, String> {
        let dir = TempDir::new().map_err(|e| format!("failed to create temp dir: {e}"))?;
        let repo = dir.path();

        run_git(repo, &["init", "--initial-branch", "main"])?;
        run_git(repo, &["config", "user.email", "fixture@test.local"])?;
        run_git(repo, &["config", "user.name", "Fixture"])?;

        Ok(Self {
            dir,
            actions: Vec::new(),
        })
    }

    /// Queue a commit with the given message and file operations.
    pub fn commit(mut self, message: &str, ops: Vec<FileOp>) -> Self {
        self.actions.push(BuilderAction::Commit(FixtureCommit {
            message: message.to_string(),
            operations: ops,
        }));
        self
    }

    /// Queue a merge commit that merges `branch` into the current branch.
    pub fn merge(mut self, message: &str, branch: &str) -> Self {
        self.actions.push(BuilderAction::Merge {
            message: message.to_string(),
            branch: branch.to_string(),
        });
        self
    }

    /// Execute all queued actions and return the finished fixture repository.
    pub fn build(self) -> Result<FixtureRepo, String> {
        let repo = self.dir.path();
        let mut commits: Vec<CommitId> = Vec::new();

        for action in &self.actions {
            match action {
                BuilderAction::Commit(fc) => {
                    apply_file_ops(repo, &fc.operations)?;
                    run_git(repo, &["add", "."])?;
                    // Use --allow-empty so commits with no net change still work.
                    run_git(repo, &["commit", "--allow-empty", "-m", &fc.message])?;
                    let sha = rev_parse_head(repo)?;
                    commits.push(CommitId(sha));
                }
                BuilderAction::Merge { message, branch } => {
                    run_git(repo, &["merge", "--no-ff", "-m", message, branch])?;
                    let sha = rev_parse_head(repo)?;
                    commits.push(CommitId(sha));
                }
            }
        }

        Ok(FixtureRepo {
            dir: self.dir,
            commits,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run_git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| format!("failed to run git {}: {e}", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "git {} failed (exit {:?}): {}",
            args.join(" "),
            output.status.code(),
            stderr.trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn rev_parse_head(repo: &Path) -> Result<String, String> {
    run_git(repo, &["rev-parse", "HEAD"])
}

fn apply_file_ops(repo: &Path, ops: &[FileOp]) -> Result<(), String> {
    for op in ops {
        match op {
            FileOp::Write { path, content } => {
                let full = repo.join(path);
                if let Some(parent) = full.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
                }
                std::fs::write(&full, content)
                    .map_err(|e| format!("write {}: {e}", full.display()))?;
            }
            FileOp::Delete { path } => {
                let full = repo.join(path);
                if full.exists() {
                    std::fs::remove_file(&full)
                        .map_err(|e| format!("delete {}: {e}", full.display()))?;
                }
            }
            FileOp::Rename { from, to } => {
                let src = repo.join(from);
                let dst = repo.join(to);
                if let Some(parent) = dst.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
                }
                std::fs::rename(&src, &dst)
                    .map_err(|e| format!("rename {} → {}: {e}", src.display(), dst.display()))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn push_and_build_produces_correct_sequence() {
        let seq = RevisionSequenceBuilder::new()
            .push("aaa")
            .push("bbb")
            .push("ccc")
            .build();

        assert_eq!(seq.revisions.len(), 3);
        assert_eq!(seq.revisions[0], CommitId("aaa".into()));
        assert_eq!(seq.revisions[1], CommitId("bbb".into()));
        assert_eq!(seq.revisions[2], CommitId("ccc".into()));
    }

    #[test]
    fn exact_boundary_produces_n_commits() {
        let seq = RevisionSequenceBuilder::exact_boundary(5);
        assert_eq!(seq.revisions.len(), 5);
        for i in 0..5 {
            assert_eq!(seq.revisions[i], CommitId(format!("commit-{}", i)));
        }
    }

    #[test]
    fn exact_boundary_zero_produces_empty_sequence() {
        let seq = RevisionSequenceBuilder::exact_boundary(0);
        assert!(seq.revisions.is_empty());
    }

    #[test]
    fn with_labels_produces_correct_sequence() {
        let seq = RevisionSequenceBuilder::with_labels(&["good", "mid", "bad"]);
        assert_eq!(seq.revisions.len(), 3);
        assert_eq!(seq.revisions[0], CommitId("good".into()));
        assert_eq!(seq.revisions[1], CommitId("mid".into()));
        assert_eq!(seq.revisions[2], CommitId("bad".into()));
    }

    #[test]
    fn with_labels_empty_produces_empty_sequence() {
        let seq = RevisionSequenceBuilder::with_labels(&[]);
        assert!(seq.revisions.is_empty());
    }

    #[test]
    fn build_with_fewer_than_two_commits_still_works() {
        let empty = RevisionSequenceBuilder::new().build();
        assert!(empty.revisions.is_empty());

        let single = RevisionSequenceBuilder::new().push("only-one").build();
        assert_eq!(single.revisions.len(), 1);
        assert_eq!(single.revisions[0], CommitId("only-one".into()));
    }

    // Feature: v01-release-train, Property 3: Revision Sequence Boundary Invariant
    // **Validates: Requirements 1.4, 1.5**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_revision_sequence_boundary_invariant(
            good in "[a-f0-9]{8,40}",
            bad in "[a-f0-9]{8,40}",
            intermediates in prop::collection::vec("[a-f0-9]{8,40}", 0..20),
        ) {
            let mut builder = RevisionSequenceBuilder::new().push(good.clone());
            for mid in &intermediates {
                builder = builder.push(mid.clone());
            }
            builder = builder.push(bad.clone());
            let seq = builder.build();

            // First element is the good commit
            prop_assert_eq!(&seq.revisions.first().unwrap().0, &good,
                "first element must be the good commit");
            // Last element is the bad commit
            prop_assert_eq!(&seq.revisions.last().unwrap().0, &bad,
                "last element must be the bad commit");
            // Length >= 2 (at minimum good + bad)
            prop_assert!(seq.revisions.len() >= 2,
                "sequence must contain at least 2 elements, got {}", seq.revisions.len());
        }
    }

    // -----------------------------------------------------------------------
    // GitRepoBuilder tests
    // -----------------------------------------------------------------------

    #[test]
    fn git_repo_builder_creates_repo_with_commits() {
        let repo = GitRepoBuilder::new()
            .unwrap()
            .commit(
                "first",
                vec![FileOp::Write {
                    path: "hello.txt".into(),
                    content: "hello".into(),
                }],
            )
            .commit(
                "second",
                vec![FileOp::Write {
                    path: "hello.txt".into(),
                    content: "world".into(),
                }],
            )
            .build()
            .unwrap();

        assert_eq!(repo.commits.len(), 2);
        // Each commit SHA should be a 40-char hex string
        for c in &repo.commits {
            assert_eq!(c.0.len(), 40, "expected 40-char SHA, got: {}", c.0);
        }
        // Commits should be distinct
        assert_ne!(repo.commits[0], repo.commits[1]);
    }

    #[test]
    fn git_repo_builder_supports_delete() {
        let repo = GitRepoBuilder::new()
            .unwrap()
            .commit(
                "add file",
                vec![FileOp::Write {
                    path: "remove_me.txt".into(),
                    content: "bye".into(),
                }],
            )
            .commit(
                "delete file",
                vec![FileOp::Delete {
                    path: "remove_me.txt".into(),
                }],
            )
            .build()
            .unwrap();

        assert_eq!(repo.commits.len(), 2);
        // The file should no longer exist in the working tree
        assert!(!repo.dir.path().join("remove_me.txt").exists());
    }

    #[test]
    fn git_repo_builder_supports_rename() {
        let repo = GitRepoBuilder::new()
            .unwrap()
            .commit(
                "add file",
                vec![FileOp::Write {
                    path: "old_name.txt".into(),
                    content: "data".into(),
                }],
            )
            .commit(
                "rename file",
                vec![FileOp::Rename {
                    from: "old_name.txt".into(),
                    to: "new_name.txt".into(),
                }],
            )
            .build()
            .unwrap();

        assert_eq!(repo.commits.len(), 2);
        assert!(!repo.dir.path().join("old_name.txt").exists());
        assert!(repo.dir.path().join("new_name.txt").exists());
    }

    #[test]
    fn git_repo_builder_merge() {
        // Create a repo, branch off, then merge back
        let builder = GitRepoBuilder::new().unwrap();
        let dir = builder.dir.path().to_path_buf();

        // We need to do the branch creation manually since the builder
        // queues actions. Build a base commit first, then create a branch,
        // add a commit on it, switch back, and merge.
        let repo = GitRepoBuilder::new()
            .unwrap()
            .commit(
                "initial",
                vec![FileOp::Write {
                    path: "main.txt".into(),
                    content: "main".into(),
                }],
            )
            .build()
            .unwrap();

        let repo_path = repo.dir.path();

        // Create and populate a feature branch
        run_git(repo_path, &["checkout", "-b", "feature"]).unwrap();
        std::fs::write(repo_path.join("feature.txt"), "feature work").unwrap();
        run_git(repo_path, &["add", "."]).unwrap();
        run_git(repo_path, &["commit", "-m", "feature commit"]).unwrap();
        run_git(repo_path, &["checkout", "main"]).unwrap();

        // Now merge
        run_git(
            repo_path,
            &["merge", "--no-ff", "-m", "merge feature", "feature"],
        )
        .unwrap();

        let merge_sha = rev_parse_head(repo_path).unwrap();
        assert_eq!(merge_sha.len(), 40);

        // Verify it's actually a merge commit (two parents)
        let parents = run_git(repo_path, &["rev-list", "--parents", "-1", "HEAD"]).unwrap();
        let parent_count = parents.split_whitespace().count() - 1; // first token is the commit itself
        assert_eq!(parent_count, 2, "merge commit should have 2 parents");

        drop(dir); // keep the builder's dir alive until here
    }

    #[test]
    fn git_repo_builder_subdirectories() {
        let repo = GitRepoBuilder::new()
            .unwrap()
            .commit(
                "nested",
                vec![FileOp::Write {
                    path: "src/lib/mod.rs".into(),
                    content: "// module".into(),
                }],
            )
            .build()
            .unwrap();

        assert_eq!(repo.commits.len(), 1);
        assert!(repo.dir.path().join("src/lib/mod.rs").exists());
    }
}
