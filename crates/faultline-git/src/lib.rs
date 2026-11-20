use std::collections::HashMap;

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
        Self::verify_git_available()?;
        let repo_root = Self::repository_root(&repo_root)?;
        let scratch_root = repo_root.join(".faultline").join("scratch");
        fs::create_dir_all(&scratch_root)?;
        Self::cleanup_stale_worktrees(&repo_root, &scratch_root);
        Ok(Self {
            repo_root,
            scratch_root,
        })
    }

    /// Scan `.faultline/scratch/` for leftover directories from previous runs
    /// and remove them. Attempts `git worktree remove --force` first, falling
    /// back to `fs::remove_dir_all`. Warnings are logged on failure but errors
    /// are never propagated so that construction always succeeds.
    fn cleanup_stale_worktrees(repo_root: &PathBuf, scratch_root: &PathBuf) {
        let entries = match fs::read_dir(scratch_root) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!(
                    "warning: could not scan scratch directory {}: {}",
                    scratch_root.display(),
                    e
                );
                return;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("warning: could not read scratch directory entry: {}", e);
                    continue;
                }
            };

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Try git worktree remove --force first
            let git_result = Command::new("git")
                .args(["worktree", "remove", "--force"])
                .arg(&path)
                .current_dir(repo_root)
                .output();

            let removed_by_git = match git_result {
                Ok(output) => output.status.success(),
                Err(_) => false,
            };

            if removed_by_git {
                continue;
            }

            // Fallback: direct directory removal
            if let Err(e) = fs::remove_dir_all(&path) {
                eprintln!(
                    "warning: failed to remove stale worktree {}: {}",
                    path.display(),
                    e
                );
            }
        }
    }

    fn verify_git_available() -> Result<()> {
        Command::new("git")
            .arg("--version")
            .output()
            .map_err(|_| FaultlineError::Git("git binary not found on PATH".to_string()))?;
        Ok(())
    }

    fn repository_root(path: &std::path::Path) -> Result<PathBuf> {
        let output = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree", "--show-cdup"])
            .current_dir(path)
            .output()
            .map_err(|_| {
                FaultlineError::Git(format!("not a git repository: {}", path.display()))
            })?;
        // Git prints only "../" components for --show-cdup, so even non-UTF-8
        // repository names stay in the original OS path.
        let stdout = std::str::from_utf8(&output.stdout)
            .map_err(|err| FaultlineError::Git(err.to_string()))?;
        let mut lines = stdout.lines();
        if !output.status.success() || lines.next() != Some("true") {
            return Err(FaultlineError::Git(format!(
                "not a git repository: {}",
                path.display()
            )));
        }
        let ascent = lines.next().ok_or_else(|| {
            FaultlineError::Git("git did not return the path to the worktree root".into())
        })?;
        Ok(path.join(ascent).canonicalize()?)
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
            OsString::from("--end-of-options"),
            OsString::from(format!("{}^{{commit}}", spec.0)),
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

    /// Run `git log --format='%aN' --since='90 days ago' -- <path>` and return
    /// the most-frequent author name. Returns `None` if no commits touch the path
    /// or if the git command fails.
    fn most_frequent_author(&self, path: &str) -> Option<String> {
        let output = self.git_output(vec![
            OsString::from("log"),
            OsString::from("--format=%aN"),
            OsString::from("--since=90 days ago"),
            OsString::from("--"),
            OsString::from(path),
        ]);

        let text = match output {
            Ok(t) => t,
            Err(e) => {
                eprintln!(
                    "warning: git log for blame frequency on '{}' failed: {}",
                    path, e
                );
                return None;
            }
        };

        if text.trim().is_empty() {
            return None;
        }

        let mut counts: HashMap<&str, usize> = HashMap::new();
        for line in text.lines() {
            let name = line.trim();
            if !name.is_empty() {
                *counts.entry(name).or_insert(0) += 1;
            }
        }

        counts
            .into_iter()
            .max_by_key(|&(_, count)| count)
            .map(|(name, _)| name.to_string())
    }
}

// ---------------------------------------------------------------------------
// CODEOWNERS parsing (public for testability)
// ---------------------------------------------------------------------------

/// A single parsed CODEOWNERS rule: a gitignore-style pattern and its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeownersRule {
    pub pattern: String,
    pub owner: String,
}

/// Parse CODEOWNERS file content into a list of rules.
/// Lines starting with `#` are comments. Empty lines are ignored.
/// Each non-comment line is `<pattern> <owner> [<owner>...]`.
/// Malformed lines (no owner) are skipped with a warning.
pub fn parse_codeowners(content: &str) -> Vec<CodeownersRule> {
    let mut rules = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 2 {
            eprintln!("warning: malformed CODEOWNERS line (no owner): {}", trimmed);
            continue;
        }
        let pattern = parts[0].to_string();
        // Take the first owner listed
        let owner = parts[1].to_string();
        rules.push(CodeownersRule { pattern, owner });
    }
    rules
}

/// Match a file path against a CODEOWNERS pattern.
/// Supports:
/// - `*` matches anything except `/`
/// - `**` matches everything including `/`
/// - Leading `/` anchors to repo root; without leading `/`, matches anywhere
/// - Trailing `/` matches directories (we treat as prefix match)
fn codeowners_pattern_matches(pattern: &str, path: &str) -> bool {
    let pat = pattern.trim();
    let p = path.trim();

    if pat.is_empty() || p.is_empty() {
        return false;
    }

    // Handle trailing `/` — matches as a directory prefix
    let (pat_str, is_dir_pattern) = if let Some(stripped) = pat.strip_suffix('/') {
        (stripped, true)
    } else {
        (pat, false)
    };

    // Handle leading `/` — anchored to root
    let (pat_str, anchored) = if let Some(stripped) = pat_str.strip_prefix('/') {
        (stripped, true)
    } else {
        (pat_str, false)
    };

    if is_dir_pattern {
        // Directory pattern: path must start with the pattern as a prefix
        if anchored {
            return p == pat_str || p.starts_with(&format!("{}/", pat_str));
        } else {
            // Match anywhere in the path
            return p == pat_str
                || p.starts_with(&format!("{}/", pat_str))
                || p.contains(&format!("/{}/", pat_str))
                || p.ends_with(&format!("/{}", pat_str));
        }
    }

    if anchored {
        glob_match(pat_str, p)
    } else {
        // Unanchored: if pattern contains `/`, match from root; otherwise match basename
        if pat_str.contains('/') {
            glob_match(pat_str, p)
        } else {
            // Match against the basename of the path
            let basename = p.rsplit('/').next().unwrap_or(p);
            glob_match(pat_str, basename) || glob_match(pat_str, p)
        }
    }
}

/// Simple glob matcher supporting `*` (any non-`/` chars) and `**` (any chars including `/`).
fn glob_match(pattern: &str, text: &str) -> bool {
    glob_match_recursive(pattern.as_bytes(), text.as_bytes())
}

fn glob_match_recursive(pat: &[u8], txt: &[u8]) -> bool {
    if pat.is_empty() {
        return txt.is_empty();
    }

    // Handle `**`
    if pat.len() >= 2 && pat[0] == b'*' && pat[1] == b'*' {
        // `**/` or `**` at end
        let rest = if pat.len() >= 3 && pat[2] == b'/' {
            &pat[3..]
        } else {
            &pat[2..]
        };
        // Try matching rest against every suffix of txt
        for i in 0..=txt.len() {
            if glob_match_recursive(rest, &txt[i..]) {
                return true;
            }
        }
        return false;
    }

    // Handle single `*`
    if pat[0] == b'*' {
        // Match any sequence of non-`/` characters
        for i in 0..=txt.len() {
            if i > 0 && txt[i - 1] == b'/' {
                break;
            }
            if glob_match_recursive(&pat[1..], &txt[i..]) {
                return true;
            }
        }
        return false;
    }

    // Handle `?`
    if pat[0] == b'?' {
        if txt.is_empty() || txt[0] == b'/' {
            return false;
        }
        return glob_match_recursive(&pat[1..], &txt[1..]);
    }

    // Literal character match
    if txt.is_empty() || pat[0] != txt[0] {
        return false;
    }
    glob_match_recursive(&pat[1..], &txt[1..])
}

/// Given parsed CODEOWNERS rules and a file path, return the matching owner.
/// CODEOWNERS uses LAST-match-wins semantics.
pub fn match_codeowners(rules: &[CodeownersRule], path: &str) -> Option<String> {
    let mut matched_owner: Option<String> = None;
    for rule in rules {
        if codeowners_pattern_matches(&rule.pattern, path) {
            matched_owner = Some(rule.owner.clone());
        }
    }
    matched_owner
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

        let traversal = match mode {
            HistoryMode::AncestryPath => "--ancestry-path",
            HistoryMode::FirstParent => "--first-parent",
        };
        let range = format!("{}..{}", good_resolved.0, bad_resolved.0);
        // Read each commit's own parent IDs. Asking rev-list for --parents
        // enables parent rewriting, which can hide gaps in a filtered walk.
        let args = [
            "rev-list",
            "--reverse",
            "--topo-order",
            "--no-commit-header",
            "--format=%H %P",
            traversal,
            &range,
            "--",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();

        let output = self.git_output(args)?;
        let mut revisions = vec![good_resolved.clone()];
        let mut previous = good_resolved.0.as_str();
        for line in output.lines() {
            let mut fields = line.split_whitespace();
            let Some(commit) = fields.next() else {
                continue;
            };
            match mode {
                HistoryMode::AncestryPath if !fields.any(|parent| parent == previous) => {
                    return Err(FaultlineError::InvalidInput(format!(
                        "ambiguous ancestry path: {previous} is not a parent of {commit}. \
                         Use --first-parent with a good revision on that chain."
                    )));
                }
                HistoryMode::FirstParent if fields.next() != Some(previous) => {
                    return Err(FaultlineError::InvalidInput(format!(
                        "good revision {} must be on the first-parent chain of bad revision {}",
                        good_resolved.0, bad_resolved.0
                    )));
                }
                _ => {}
            }
            revisions.push(CommitId(commit.to_string()));
            previous = commit;
        }

        if revisions.len() < 2 || previous != bad_resolved.0 {
            return Err(FaultlineError::InvalidInput(
                "history range must contain distinct good and bad endpoints".to_string(),
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
            } else if status_code.starts_with('C') {
                ChangeStatus::Unknown
            } else if status_code.starts_with('T') {
                ChangeStatus::TypeChanged
            } else {
                ChangeStatus::Unknown
            };
            // Rename (R###) and copy (C###) entries have two paths: source\tdest.
            // Use the destination path for both.
            let has_two_paths = status_code.starts_with('R') || status_code.starts_with('C');
            let path = if has_two_paths {
                parts.get(2).or_else(|| parts.get(1)).copied().unwrap_or("")
            } else {
                parts.get(1).copied().unwrap_or("")
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

    fn codeowners_for_paths(&self, paths: &[String]) -> Result<HashMap<String, Option<String>>> {
        // Try .github/CODEOWNERS first, then CODEOWNERS at repo root
        let candidates = [
            self.repo_root.join(".github").join("CODEOWNERS"),
            self.repo_root.join("CODEOWNERS"),
        ];

        let content = candidates.iter().find_map(|p| fs::read_to_string(p).ok());

        let rules = match content {
            Some(text) => parse_codeowners(&text),
            None => {
                // No CODEOWNERS file — return empty map
                return Ok(paths.iter().map(|p| (p.clone(), None)).collect());
            }
        };

        let mut result = HashMap::new();
        for path in paths {
            let owner = match_codeowners(&rules, path);
            result.insert(path.clone(), owner);
        }
        Ok(result)
    }

    fn blame_frequency(&self, paths: &[String]) -> Result<HashMap<String, Option<String>>> {
        let mut result = HashMap::new();
        for path in paths {
            let author = self.most_frequent_author(path);
            result.insert(path.clone(), author);
        }
        Ok(result)
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
            if checkout.path.exists()
                && let Err(e) = fs::remove_dir_all(&checkout.path)
            {
                eprintln!(
                    "warning: failed to clean up checkout {}: {}",
                    checkout.path.display(),
                    e
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test_support {
    use std::path::Path;
    use std::process::Command;

    pub(super) type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    pub(super) fn git_cmd(dir: &Path, args: &[&str]) -> TestResult<String> {
        let out = Command::new("git").args(args).current_dir(dir).output()?;
        if !out.status.success() {
            return Err(format!(
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )
            .into());
        }
        Ok(String::from_utf8(out.stdout)?.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::TestResult;
    use super::test_support::git_cmd;
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
            let tmp = tempfile::tempdir()?;
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

    // Feature: v01-product-sharpening, Property 55: CODEOWNERS parser determinism
    // **Validates: Requirements 1.3**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_codeowners_parser_determinism(
            lines in prop::collection::vec(
                (
                    prop_oneof![
                        Just("*".to_string()),
                        Just("*.rs".to_string()),
                        Just("*.js".to_string()),
                        Just("src/".to_string()),
                        Just("/docs/".to_string()),
                        Just("**/*.py".to_string()),
                        "[a-z/.*]{1,20}".prop_map(|s| s),
                    ],
                    prop_oneof![
                        Just("@team-alpha".to_string()),
                        Just("@team-beta".to_string()),
                        Just("user@example.com".to_string()),
                        "@[a-z]{1,8}".prop_map(|s| s),
                    ],
                ),
                0..15,
            ),
            paths in prop::collection::vec("[a-z][a-z0-9/._]{0,25}", 1..10),
        ) {
            // Build CODEOWNERS content from generated lines
            let content: String = lines
                .iter()
                .map(|(pattern, owner)| format!("{} {}", pattern, owner))
                .collect::<Vec<_>>()
                .join("\n");

            // Parse twice and match each path twice — results must be identical
            let rules_a = parse_codeowners(&content);
            let rules_b = parse_codeowners(&content);
            prop_assert_eq!(&rules_a, &rules_b, "parsing the same content must produce identical rules");

            for path in &paths {
                let owner_a = match_codeowners(&rules_a, path);
                let owner_b = match_codeowners(&rules_b, path);
                prop_assert_eq!(
                    &owner_a, &owner_b,
                    "matching path '{}' must produce deterministic results", path
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // CODEOWNERS unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn codeowners_parse_valid_file() {
        let content =
            "# Comment line\n\n*.rs @rust-team\n/docs/ @docs-team\nsrc/lib.rs user@example.com\n";
        let rules = parse_codeowners(content);
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].pattern, "*.rs");
        assert_eq!(rules[0].owner, "@rust-team");
        assert_eq!(rules[1].pattern, "/docs/");
        assert_eq!(rules[1].owner, "@docs-team");
        assert_eq!(rules[2].pattern, "src/lib.rs");
        assert_eq!(rules[2].owner, "user@example.com");
    }

    #[test]
    fn codeowners_parse_empty_file() {
        let rules = parse_codeowners("");
        assert!(rules.is_empty());
    }

    #[test]
    fn codeowners_parse_comments_only() {
        let rules = parse_codeowners("# just a comment\n# another\n");
        assert!(rules.is_empty());
    }

    #[test]
    fn codeowners_parse_malformed_line_skipped() {
        let content = "*.rs @team\nmalformed-no-owner\n*.js @js-team\n";
        let rules = parse_codeowners(content);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].pattern, "*.rs");
        assert_eq!(rules[1].pattern, "*.js");
    }

    #[test]
    fn codeowners_last_match_wins() {
        let content = "* @default\n*.rs @rust-team\n";
        let rules = parse_codeowners(content);
        // For a .rs file, the last matching rule should win
        let owner = match_codeowners(&rules, "src/main.rs");
        assert_eq!(owner, Some("@rust-team".to_string()));
    }

    #[test]
    fn codeowners_wildcard_matches_all() {
        let content = "* @default-owner\n";
        let rules = parse_codeowners(content);
        assert_eq!(
            match_codeowners(&rules, "anything.txt"),
            Some("@default-owner".to_string())
        );
        assert_eq!(
            match_codeowners(&rules, "src/deep/file.rs"),
            Some("@default-owner".to_string())
        );
    }

    #[test]
    fn codeowners_directory_pattern() {
        let content = "/docs/ @docs-team\n";
        let rules = parse_codeowners(content);
        assert_eq!(
            match_codeowners(&rules, "docs/readme.md"),
            Some("@docs-team".to_string())
        );
        assert_eq!(match_codeowners(&rules, "src/main.rs"), None);
    }

    #[test]
    fn codeowners_doublestar_pattern() {
        let content = "**/*.py @python-team\n";
        let rules = parse_codeowners(content);
        assert_eq!(
            match_codeowners(&rules, "scripts/test.py"),
            Some("@python-team".to_string())
        );
        assert_eq!(
            match_codeowners(&rules, "deep/nested/dir/file.py"),
            Some("@python-team".to_string())
        );
        assert_eq!(
            match_codeowners(&rules, "test.py"),
            Some("@python-team".to_string())
        );
    }

    #[test]
    fn codeowners_no_match_returns_none() {
        let content = "*.rs @rust-team\n";
        let rules = parse_codeowners(content);
        assert_eq!(match_codeowners(&rules, "readme.md"), None);
    }

    #[test]
    fn canonical_root_and_checkout_from_nested_paths()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let cwd = std::env::current_dir()?;
        let tmp = tempfile::Builder::new()
            .prefix("git root ")
            .tempdir_in(&cwd)?;
        let root = tmp.path().canonicalize()?;
        let nested = tmp.path().join("one").join("two");
        fs::create_dir_all(&nested)?;
        git_cmd(tmp.path(), &["init"])?;
        fs::write(tmp.path().join("marker"), "tracked")?;
        git_cmd(tmp.path(), &["add", "marker"])?;
        git_cmd(
            tmp.path(),
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-m",
                "initial",
            ],
        )?;
        let head = CommitId(git_cmd(tmp.path(), &["rev-parse", "HEAD"])?);
        for input in [
            tmp.path().strip_prefix(&cwd)?,
            nested.strip_prefix(&cwd)?,
            tmp.path(),
            nested.as_path(),
        ] {
            let adapter = GitAdapter::new(input)?;
            assert_eq!(adapter.repo_root, root);
            let checkout = adapter.checkout_revision(&head)?;
            assert!(checkout.path.is_absolute());
            assert!(checkout.path.starts_with(root.join(".faultline/scratch")));
            assert_eq!(fs::read_to_string(checkout.path.join("marker"))?, "tracked");
            adapter.cleanup_checkout(&checkout)?;
            assert!(!checkout.path.exists());
        }
        Ok(())
    }

    #[test]
    fn changed_paths_detects_add_modify_delete_rename() -> TestResult {
        let tmp = tempfile::tempdir()?;
        let repo = tmp.path();

        // Init repo with an initial commit containing two files.
        git_cmd(repo, &["init"])?;
        git_cmd(repo, &["config", "user.email", "test@test.com"])?;
        git_cmd(repo, &["config", "user.name", "Test"])?;

        std::fs::write(repo.join("keep.txt"), "keep")?;
        std::fs::write(repo.join("to_modify.txt"), "original")?;
        std::fs::write(repo.join("to_delete.txt"), "delete me")?;
        std::fs::write(repo.join("to_rename.txt"), "rename me")?;
        git_cmd(repo, &["add", "."])?;
        git_cmd(repo, &["commit", "-m", "initial"])?;
        let from_sha = git_cmd(repo, &["rev-parse", "HEAD"])?;

        // Second commit: add, modify, delete, rename.
        std::fs::write(repo.join("added.txt"), "new file")?;
        std::fs::write(repo.join("to_modify.txt"), "changed")?;
        std::fs::remove_file(repo.join("to_delete.txt"))?;
        std::fs::rename(repo.join("to_rename.txt"), repo.join("renamed.txt"))?;
        git_cmd(repo, &["add", "."])?;
        git_cmd(repo, &["commit", "-m", "changes"])?;
        let to_sha = git_cmd(repo, &["rev-parse", "HEAD"])?;

        let adapter = GitAdapter::new(repo)?;
        let changes = adapter.changed_paths(&CommitId(from_sha), &CommitId(to_sha))?;

        // Verify we got the expected changes.
        let added: Vec<_> = changes
            .iter()
            .filter(|c| c.status == ChangeStatus::Added)
            .collect();
        assert!(
            added.iter().any(|c| c.path == "added.txt"),
            "should detect added.txt, got: {:?}",
            added
        );

        let modified: Vec<_> = changes
            .iter()
            .filter(|c| c.status == ChangeStatus::Modified)
            .collect();
        assert!(
            modified.iter().any(|c| c.path == "to_modify.txt"),
            "should detect to_modify.txt as modified, got: {:?}",
            modified
        );

        let deleted: Vec<_> = changes
            .iter()
            .filter(|c| c.status == ChangeStatus::Deleted)
            .collect();
        assert!(
            deleted.iter().any(|c| c.path == "to_delete.txt"),
            "should detect to_delete.txt as deleted, got: {:?}",
            deleted
        );

        // Rename detection: git may detect as rename (R) or as delete+add.
        // If detected as rename, the path should be the destination.
        let renamed: Vec<_> = changes
            .iter()
            .filter(|c| c.status == ChangeStatus::Renamed)
            .collect();
        if !renamed.is_empty() {
            assert!(
                renamed.iter().any(|c| c.path == "renamed.txt"),
                "rename entry should use destination path, got: {:?}",
                renamed
            );
        }
        Ok(())
    }

    #[test]
    fn changed_paths_empty_diff_returns_empty_vec() -> TestResult {
        let tmp = tempfile::tempdir()?;
        let repo = tmp.path();

        git_cmd(repo, &["init"])?;
        git_cmd(repo, &["config", "user.email", "test@test.com"])?;
        git_cmd(repo, &["config", "user.name", "Test"])?;
        std::fs::write(repo.join("file.txt"), "content")?;
        git_cmd(repo, &["add", "."])?;
        git_cmd(repo, &["commit", "-m", "initial"])?;
        let sha = git_cmd(repo, &["rev-parse", "HEAD"])?;

        let adapter = GitAdapter::new(repo)?;
        let changes = adapter.changed_paths(&CommitId(sha.clone()), &CommitId(sha))?;
        assert!(changes.is_empty(), "same commit should yield no changes");
        Ok(())
    }
}

#[cfg(test)]
mod env_validation_tests {
    use super::test_support::TestResult;
    use super::*;
    use faultline_fixtures::{FileOp, GitRepoBuilder};
    use faultline_ports::CheckoutPort;
    use faultline_types::CheckedOutRevision;

    #[test]
    fn rejects_non_repo_path() -> TestResult {
        let tmp = tempfile::tempdir()?;
        let result = GitAdapter::new(tmp.path());
        let err = result
            .err()
            .ok_or("expected rejection of a non-git directory")?;
        let msg = format!("{}", err);
        assert!(
            msg.contains("not a git repository"),
            "error should mention 'not a git repository', got: {}",
            msg
        );
        Ok(())
    }

    /// Validates: Requirements 4.3
    #[test]
    fn cleans_stale_worktrees_on_construction() -> TestResult {
        let repo = GitRepoBuilder::new()?
            .commit(
                "initial",
                vec![FileOp::Write {
                    path: "file.txt".into(),
                    content: "hello".into(),
                }],
            )
            .build()?;

        let repo_path = repo.dir.path();
        let scratch = repo_path.join(".faultline").join("scratch");
        fs::create_dir_all(&scratch)?;

        // Manually create a stale directory that looks like a leftover worktree.
        let stale = scratch.join("stale-worktree-12345");
        fs::create_dir_all(&stale)?;
        fs::write(stale.join("marker.txt"), "stale")?;
        assert!(
            stale.exists(),
            "stale directory should exist before construction"
        );

        // Constructing a new GitAdapter should clean up the stale directory.
        let _adapter = GitAdapter::new(repo_path)?;
        assert!(
            !stale.exists(),
            "stale worktree directory should be removed after GitAdapter construction"
        );
        Ok(())
    }

    /// Validates: Requirements 4.6
    #[test]
    fn cleanup_checkout_returns_ok_on_missing_directory() -> TestResult {
        let repo = GitRepoBuilder::new()?
            .commit(
                "initial",
                vec![FileOp::Write {
                    path: "file.txt".into(),
                    content: "hello".into(),
                }],
            )
            .build()?;

        let adapter = GitAdapter::new(repo.dir.path())?;

        // Create a CheckedOutRevision pointing to a non-existent path.
        let fake_checkout = CheckedOutRevision {
            commit: CommitId("deadbeef".to_string()),
            path: repo.dir.path().join("nonexistent-worktree"),
        };

        let result = adapter.cleanup_checkout(&fake_checkout);
        assert!(
            result.is_ok(),
            "cleanup_checkout should return Ok(()) for a missing directory, got: {:?}",
            result
        );
        Ok(())
    }
}

#[cfg(test)]
mod fixture_scenario_tests {
    use super::test_support::TestResult;
    use super::test_support::git_cmd;
    use super::*;
    use faultline_codes::{ObservationClass, ProbeKind};
    use faultline_fixtures::{FileOp, GitRepoBuilder};
    use faultline_localization::LocalizationSession;
    use faultline_ports::HistoryPort;
    use faultline_types::{
        Confidence, LocalizationOutcome, ProbeObservation, RevisionSpec, SearchPolicy,
    };

    fn make_obs(commit: &CommitId, class: ObservationClass, seq_idx: u64) -> ProbeObservation {
        ProbeObservation {
            commit: commit.clone(),
            class,
            kind: ProbeKind::Test,
            exit_code: Some(match class {
                ObservationClass::Pass => 0,
                ObservationClass::Skip => 125,
                _ => 1,
            }),
            timed_out: false,
            duration_ms: 1,
            stdout: String::new(),
            stderr: String::new(),
            sequence_index: seq_idx,
            signal_number: None,
            probe_command: String::new(),
            working_dir: String::new(),
            flake_signal: None,
        }
    }

    /// Fixture scenario: exact-first-bad-commit (real Git)
    /// A linear 5-commit repo where commit 3 (index 3) introduces a failing change.
    /// Commits 0–2 pass, commits 3–4 fail. End-to-end with GitAdapter::linearize
    /// + LocalizationSession verifies FirstBad outcome with correct boundary pair.
    /// **Validates: Requirements 7.4**
    #[test]
    fn exact_first_bad_commit_real_git() -> TestResult {
        // Build a 5-commit linear repo; commit 3 introduces the "bug".
        let repo = GitRepoBuilder::new()?
            .commit(
                "commit-0: initial good",
                vec![FileOp::Write {
                    path: "src/main.rs".into(),
                    content: "fn main() { println!(\"v0\"); }".into(),
                }],
            )
            .commit(
                "commit-1: still good",
                vec![FileOp::Write {
                    path: "src/main.rs".into(),
                    content: "fn main() { println!(\"v1\"); }".into(),
                }],
            )
            .commit(
                "commit-2: last good",
                vec![FileOp::Write {
                    path: "src/main.rs".into(),
                    content: "fn main() { println!(\"v2\"); }".into(),
                }],
            )
            .commit(
                "commit-3: introduces bug",
                vec![FileOp::Write {
                    path: "src/main.rs".into(),
                    content: concat!(
                        "// The regression starts in this revision.\n",
                        "fn main() -> std::process::ExitCode {\n",
                        "    std::process::ExitCode::from(1)\n",
                        "}\n",
                    )
                    .into(),
                }],
            )
            .commit(
                "commit-4: still bad",
                vec![FileOp::Write {
                    path: "src/main.rs".into(),
                    content: concat!(
                        "// The regression persists in this revision.\n",
                        "fn main() -> std::process::ExitCode {\n",
                        "    std::process::ExitCode::from(1)\n",
                        "}\n",
                    )
                    .into(),
                }],
            )
            .build()?;

        assert_eq!(repo.commits.len(), 5);

        // Use both tag kinds as endpoints and expect the underlying commits.
        let repo_path = repo.dir.path();
        git_cmd(repo_path, &["tag", "good", &repo.commits[0].0])?;
        git_cmd(repo_path, &["tag", "-a", "bad", "-m", "known bad"])?;
        let adapter = GitAdapter::new(repo_path)?;
        let good = RevisionSpec("good".to_string());
        let bad = RevisionSpec("bad".to_string());
        let sequence = adapter.linearize(&good, &bad, HistoryMode::AncestryPath)?;

        // The sequence should contain all 5 commits in order.
        assert_eq!(sequence.revisions, repo.commits);

        for spec in ["HEAD^{tree}", "--all"] {
            assert!(
                adapter
                    .linearize(
                        &good,
                        &RevisionSpec(spec.to_string()),
                        HistoryMode::AncestryPath
                    )
                    .is_err(),
                "only a commit revision may be used as an endpoint: {spec}"
            );
        }

        // Create a LocalizationSession and simulate probing.
        let policy = SearchPolicy::default();
        let mut session = LocalizationSession::new(sequence, policy)?;

        // Record observations: commits 0–2 pass, commits 3–4 fail.
        for i in 0..5u64 {
            let class = if i <= 2 {
                ObservationClass::Pass
            } else {
                ObservationClass::Fail
            };
            session.record(make_obs(&repo.commits[i as usize], class, i))?;
        }

        // Verify the outcome is FirstBad with last_good=commit-2, first_bad=commit-3.
        match session.outcome() {
            LocalizationOutcome::FirstBad {
                last_good,
                first_bad,
                confidence,
            } => {
                assert_eq!(last_good, repo.commits[2], "last_good should be commit-2");
                assert_eq!(first_bad, repo.commits[3], "first_bad should be commit-3");
                assert_eq!(
                    confidence,
                    Confidence::high(),
                    "exact boundary should have high confidence"
                );
            }
            other => return Err(format!("expected FirstBad outcome, got: {other:?}").into()),
        }
        Ok(())
    }

    /// Fixture scenario: first-parent-merge-history (real Git)
    /// A merge diamond is ambiguous in ancestry-path mode. First-parent mode
    /// can use the main chain but requires its known-good commit to be on it.
    /// **Validates: Requirements 7.8**
    #[test]
    fn first_parent_merge_history_real_git() -> TestResult {
        // Build a repo with an initial commit on main.
        let repo = GitRepoBuilder::new()?
            .commit(
                "initial on main",
                vec![FileOp::Write {
                    path: "main.txt".into(),
                    content: "v0".into(),
                }],
            )
            .build()?;

        let repo_path = repo.dir.path();
        let initial_sha = repo.commits[0].0.clone();

        // Create a feature branch with two commits.
        git_cmd(repo_path, &["checkout", "-b", "feature"])?;
        std::fs::write(repo_path.join("feature.txt"), "feature-1")?;
        git_cmd(repo_path, &["add", "."])?;
        git_cmd(repo_path, &["commit", "-m", "feature commit 1"])?;

        std::fs::write(repo_path.join("feature.txt"), "feature-2")?;
        git_cmd(repo_path, &["add", "."])?;
        git_cmd(repo_path, &["commit", "-m", "feature commit 2"])?;
        let feature_sha = git_cmd(repo_path, &["rev-parse", "HEAD"])?;

        // Switch back to main and add a commit so the merge is non-trivial.
        git_cmd(repo_path, &["checkout", "main"])?;
        std::fs::write(repo_path.join("main.txt"), "v1")?;
        git_cmd(repo_path, &["add", "."])?;
        git_cmd(repo_path, &["commit", "-m", "main commit after branch"])?;
        let main_sha = git_cmd(repo_path, &["rev-parse", "HEAD"])?;

        // Merge feature into main with --no-ff to force a merge commit.
        git_cmd(
            repo_path,
            &["merge", "--no-ff", "-m", "merge feature", "feature"],
        )?;
        let merge_sha = git_cmd(repo_path, &["rev-parse", "HEAD"])?;

        let adapter = GitAdapter::new(repo_path)?;
        let good = RevisionSpec(initial_sha.clone());
        let bad = RevisionSpec(merge_sha.clone());
        let side_good = RevisionSpec(feature_sha.clone());

        let ancestry = adapter
            .linearize(&good, &bad, HistoryMode::AncestryPath)
            .expect_err("ancestry-path must reject the merge diamond before search");
        assert!(
            matches!(&ancestry, FaultlineError::InvalidInput(message) if message.contains("ambiguous ancestry path")),
            "expected an ambiguity diagnostic, got {ancestry}"
        );

        let first_parent = adapter.linearize(&good, &bad, HistoryMode::FirstParent)?;
        assert_eq!(
            first_parent.revisions,
            [
                CommitId(initial_sha),
                CommitId(main_sha),
                CommitId(merge_sha.clone())
            ]
        );

        let side_first_parent = adapter
            .linearize(&side_good, &bad, HistoryMode::FirstParent)
            .expect_err("a side-branch good commit must not start a first-parent chain");
        assert!(
            matches!(&side_first_parent, FaultlineError::InvalidInput(message) if message.contains("first-parent chain")),
            "expected a first-parent diagnostic, got {side_first_parent}"
        );

        let side_ancestry = adapter.linearize(&side_good, &bad, HistoryMode::AncestryPath)?;
        assert_eq!(
            side_ancestry.revisions,
            [CommitId(feature_sha), CommitId(merge_sha)]
        );
        Ok(())
    }

    /// Fixture scenario: rename-and-delete (real Git)
    /// A repository where files are renamed and deleted between boundary commits.
    /// Verifies `GitAdapter::changed_paths` returns correct `PathChange` entries
    /// with the expected statuses (Renamed, Deleted).
    /// **Validates: Requirements 7.9**
    #[test]
    fn rename_and_delete_real_git() -> TestResult {
        // First commit: add several files.
        let repo = GitRepoBuilder::new()?
            .commit(
                "initial: add files",
                vec![
                    FileOp::Write {
                        path: "keep.txt".into(),
                        content: "stays the same".into(),
                    },
                    FileOp::Write {
                        path: "to_rename.txt".into(),
                        content: "I will be renamed".into(),
                    },
                    FileOp::Write {
                        path: "to_delete.txt".into(),
                        content: "I will be deleted".into(),
                    },
                    FileOp::Write {
                        path: "another.txt".into(),
                        content: "also stays".into(),
                    },
                ],
            )
            .commit(
                "rename one file and delete another",
                vec![
                    FileOp::Rename {
                        from: "to_rename.txt".into(),
                        to: "renamed.txt".into(),
                    },
                    FileOp::Delete {
                        path: "to_delete.txt".into(),
                    },
                ],
            )
            .build()?;

        assert_eq!(repo.commits.len(), 2);

        let adapter = GitAdapter::new(repo.dir.path())?;
        let changes = adapter.changed_paths(&repo.commits[0], &repo.commits[1])?;

        // Verify deleted file is detected.
        let deleted: Vec<_> = changes
            .iter()
            .filter(|c| c.status == ChangeStatus::Deleted)
            .collect();
        assert!(
            deleted.iter().any(|c| c.path == "to_delete.txt"),
            "should detect to_delete.txt as Deleted, got: {:?}",
            deleted
        );

        // Verify renamed file is detected.
        // Git may detect the rename as Renamed (R) or as Delete+Add depending on
        // similarity detection. Check for Renamed first; if not present, verify
        // the old path is Deleted and the new path is Added.
        let renamed: Vec<_> = changes
            .iter()
            .filter(|c| c.status == ChangeStatus::Renamed)
            .collect();
        if !renamed.is_empty() {
            assert!(
                renamed.iter().any(|c| c.path == "renamed.txt"),
                "rename entry should use destination path 'renamed.txt', got: {:?}",
                renamed
            );
        } else {
            // Fallback: git detected as delete + add instead of rename.
            let has_old_deleted = changes
                .iter()
                .any(|c| c.status == ChangeStatus::Deleted && c.path == "to_rename.txt");
            let has_new_added = changes
                .iter()
                .any(|c| c.status == ChangeStatus::Added && c.path == "renamed.txt");
            assert!(
                has_old_deleted && has_new_added,
                "if not detected as Renamed, should see to_rename.txt Deleted and renamed.txt Added, got: {:?}",
                changes
            );
        }

        // Verify unchanged files are NOT in the diff.
        let unchanged_paths: Vec<_> = changes.iter().map(|c| c.path.as_str()).collect();
        assert!(
            !unchanged_paths.contains(&"keep.txt"),
            "keep.txt should not appear in changed_paths"
        );
        assert!(
            !unchanged_paths.contains(&"another.txt"),
            "another.txt should not appear in changed_paths"
        );
        Ok(())
    }

    /// Fixture scenario: invalid-boundaries (real Git)
    /// A repository where the good commit is not an ancestor of the bad commit.
    /// Two divergent branches with no ancestor relationship between their tips.
    /// Verifies `GitAdapter::linearize` returns an error.
    /// **Validates: Requirements 7.10**
    #[test]
    fn invalid_boundaries_real_git() -> TestResult {
        // Build a repo with an initial commit on main.
        let repo = GitRepoBuilder::new()?
            .commit(
                "initial on main",
                vec![FileOp::Write {
                    path: "main.txt".into(),
                    content: "main content".into(),
                }],
            )
            .build()?;

        let repo_path = repo.dir.path();
        let initial_sha = repo.commits[0].0.clone();

        // Create branch-a from initial and add a commit.
        git_cmd(repo_path, &["checkout", "-b", "branch-a"])?;
        std::fs::write(repo_path.join("a.txt"), "branch-a work")?;
        git_cmd(repo_path, &["add", "."])?;
        git_cmd(repo_path, &["commit", "-m", "commit on branch-a"])?;
        let branch_a_sha = git_cmd(repo_path, &["rev-parse", "HEAD"])?;

        // Go back to initial and create branch-b with a divergent commit.
        git_cmd(repo_path, &["checkout", &initial_sha])?;
        git_cmd(repo_path, &["checkout", "-b", "branch-b"])?;
        std::fs::write(repo_path.join("b.txt"), "branch-b work")?;
        git_cmd(repo_path, &["add", "."])?;
        git_cmd(repo_path, &["commit", "-m", "commit on branch-b"])?;
        let branch_b_sha = git_cmd(repo_path, &["rev-parse", "HEAD"])?;

        // branch-a tip is NOT an ancestor of branch-b tip (and vice versa).
        let adapter = GitAdapter::new(repo_path)?;

        // Try linearize with branch-a as good and branch-b as bad.
        let good = RevisionSpec(branch_a_sha.clone());
        let bad = RevisionSpec(branch_b_sha.clone());
        let result = adapter.linearize(&good, &bad, HistoryMode::AncestryPath);

        let err_msg = result
            .err()
            .ok_or("expected rejection of unrelated commits")?
            .to_string();
        assert!(
            err_msg.contains("must be an ancestor"),
            "error should mention ancestor relationship, got: {}",
            err_msg
        );

        // Also verify the reverse direction fails.
        let good_rev = RevisionSpec(branch_b_sha);
        let bad_rev = RevisionSpec(branch_a_sha);
        let result_rev = adapter.linearize(&good_rev, &bad_rev, HistoryMode::AncestryPath);

        assert!(
            result_rev.is_err(),
            "linearize should also fail in the reverse direction"
        );
        Ok(())
    }
}

#[cfg(test)]
mod codeowners_integration_tests {
    use super::test_support::TestResult;
    use super::*;
    use faultline_fixtures::{FileOp, GitRepoBuilder};
    use faultline_ports::HistoryPort;

    #[test]
    fn codeowners_from_github_dir() -> TestResult {
        let repo = GitRepoBuilder::new()?
            .commit(
                "initial with CODEOWNERS",
                vec![
                    FileOp::Write {
                        path: ".github/CODEOWNERS".into(),
                        content: "*.rs @rust-team\n*.md @docs-team\n".into(),
                    },
                    FileOp::Write {
                        path: "src/main.rs".into(),
                        content: "fn main() {}".into(),
                    },
                    FileOp::Write {
                        path: "README.md".into(),
                        content: "# Hello".into(),
                    },
                ],
            )
            .build()?;

        let adapter = GitAdapter::new(repo.dir.path())?;
        let paths = vec![
            "src/main.rs".to_string(),
            "README.md".to_string(),
            "unknown.txt".to_string(),
        ];
        let owners = adapter.codeowners_for_paths(&paths)?;

        assert_eq!(
            owners
                .get("src/main.rs")
                .ok_or("missing src/main.rs result")?,
            &Some("@rust-team".to_string())
        );
        assert_eq!(
            owners.get("README.md").ok_or("missing README.md result")?,
            &Some("@docs-team".to_string())
        );
        assert_eq!(owners.get("unknown.txt"), Some(&None));
        Ok(())
    }

    #[test]
    fn codeowners_from_repo_root() -> TestResult {
        let repo = GitRepoBuilder::new()?
            .commit(
                "initial with root CODEOWNERS",
                vec![
                    FileOp::Write {
                        path: "CODEOWNERS".into(),
                        content: "* @default-owner\n".into(),
                    },
                    FileOp::Write {
                        path: "file.txt".into(),
                        content: "content".into(),
                    },
                ],
            )
            .build()?;

        let adapter = GitAdapter::new(repo.dir.path())?;
        let paths = vec!["file.txt".to_string()];
        let owners = adapter.codeowners_for_paths(&paths)?;

        assert_eq!(
            owners.get("file.txt").ok_or("missing file.txt result")?,
            &Some("@default-owner".to_string())
        );
        Ok(())
    }

    #[test]
    fn codeowners_missing_returns_none_owners() -> TestResult {
        let repo = GitRepoBuilder::new()?
            .commit(
                "no codeowners",
                vec![FileOp::Write {
                    path: "file.txt".into(),
                    content: "content".into(),
                }],
            )
            .build()?;

        let adapter = GitAdapter::new(repo.dir.path())?;
        let paths = vec!["file.txt".to_string()];
        let owners = adapter.codeowners_for_paths(&paths)?;

        assert_eq!(owners.get("file.txt"), Some(&None));
        Ok(())
    }

    #[test]
    fn blame_frequency_returns_most_frequent_author() -> TestResult {
        let repo = GitRepoBuilder::new()?
            .commit(
                "first",
                vec![FileOp::Write {
                    path: "file.txt".into(),
                    content: "v1".into(),
                }],
            )
            .commit(
                "second",
                vec![FileOp::Write {
                    path: "file.txt".into(),
                    content: "v2".into(),
                }],
            )
            .build()?;

        let adapter = GitAdapter::new(repo.dir.path())?;
        let paths = vec!["file.txt".to_string()];
        let result = adapter.blame_frequency(&paths)?;

        // The GitRepoBuilder uses "Fixture" as the author name
        let owner = result.get("file.txt").ok_or("missing file.txt result")?;
        assert!(owner.is_some(), "should find an author for file.txt");
        assert_eq!(owner.as_deref(), Some("Fixture"));
        Ok(())
    }

    #[test]
    fn blame_frequency_no_commits_returns_none() -> TestResult {
        let repo = GitRepoBuilder::new()?
            .commit(
                "initial",
                vec![FileOp::Write {
                    path: "other.txt".into(),
                    content: "content".into(),
                }],
            )
            .build()?;

        let adapter = GitAdapter::new(repo.dir.path())?;
        let paths = vec!["nonexistent.txt".to_string()];
        let result = adapter.blame_frequency(&paths)?;

        assert_eq!(result.get("nonexistent.txt"), Some(&None));
        Ok(())
    }
}
