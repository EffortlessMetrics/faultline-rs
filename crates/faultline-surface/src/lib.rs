use faultline_types::{ChangeStatus, PathChange, SubsystemBucket, SurfaceSummary, SuspectEntry};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug, Default, Clone)]
pub struct SurfaceAnalyzer;

impl SurfaceAnalyzer {
    pub fn summarize(&self, changes: &[PathChange]) -> SurfaceSummary {
        let mut buckets: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut bucket_kinds: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut execution_surfaces = BTreeSet::new();

        for change in changes {
            let bucket = bucket_name(&change.path);
            buckets
                .entry(bucket.clone())
                .or_default()
                .push(change.path.clone());
            let kind = surface_kind(&change.path);
            bucket_kinds
                .entry(bucket.clone())
                .or_default()
                .insert(kind.clone());
            if is_execution_surface(&change.path) {
                execution_surfaces.insert(change.path.clone());
            }
        }

        let bucket_items = buckets
            .into_iter()
            .map(|(name, mut paths)| {
                paths.sort();
                let kinds = bucket_kinds
                    .remove(&name)
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                SubsystemBucket {
                    change_count: paths.len(),
                    name,
                    paths,
                    surface_kinds: kinds,
                }
            })
            .collect();

        SurfaceSummary {
            total_changes: changes.len(),
            buckets: bucket_items,
            execution_surfaces: execution_surfaces.into_iter().collect(),
        }
    }

    /// Rank changed paths by investigation priority.
    /// `owners` maps path → optional owner string (from CODEOWNERS or blame).
    pub fn rank_suspect_surface(
        &self,
        changes: &[PathChange],
        owners: &HashMap<String, Option<String>>,
    ) -> Vec<SuspectEntry> {
        if changes.is_empty() {
            return Vec::new();
        }

        let mut entries: Vec<SuspectEntry> = changes
            .iter()
            .map(|change| {
                let mut score: u32 = 100;

                if is_execution_surface(&change.path) {
                    score += 200;
                }

                if change.status == ChangeStatus::Deleted {
                    score += 150;
                }

                if change.status == ChangeStatus::Renamed {
                    score += 100;
                }

                let kind = surface_kind(&change.path);

                if kind == "source" {
                    score += 50;
                }

                if kind == "tests" {
                    score += 25;
                }

                let owner_hint = owners.get(&change.path).cloned().flatten();

                SuspectEntry {
                    path: change.path.clone(),
                    priority_score: score,
                    surface_kind: kind,
                    change_status: change.status.clone(),
                    is_execution_surface: is_execution_surface(&change.path),
                    owner_hint,
                }
            })
            .collect();

        entries.sort_by(|a, b| {
            b.priority_score
                .cmp(&a.priority_score)
                .then_with(|| a.path.cmp(&b.path))
        });

        entries
    }
}

fn bucket_name(path: &str) -> String {
    let normalized = path.trim_matches('/');
    if normalized.is_empty() {
        return "repo-root".to_string();
    }
    normalized
        .split('/')
        .next()
        .unwrap_or("repo-root")
        .to_string()
}

fn surface_kind(path: &str) -> String {
    let normalized = path.trim_matches('/');
    let filename = normalized.rsplit('/').next().unwrap_or(normalized);
    if matches!(
        filename,
        "Cargo.lock"
            | "package-lock.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "Gemfile.lock"
            | "poetry.lock"
            | "composer.lock"
    ) {
        return "lockfile".to_string();
    }
    if normalized.starts_with(".github/workflows/") {
        return "workflows".to_string();
    }
    if normalized.ends_with("build.rs") {
        return "build-script".to_string();
    }
    if normalized.starts_with("tests/") || normalized.contains("/tests/") {
        return "tests".to_string();
    }
    if normalized.starts_with("benches/") || normalized.contains("/benches/") {
        return "benchmarks".to_string();
    }
    if normalized.starts_with("scripts/") || normalized.ends_with(".sh") {
        return "scripts".to_string();
    }
    if normalized.starts_with("src/") || normalized.contains("/src/") {
        return "source".to_string();
    }
    if normalized.starts_with("docs/") || normalized.ends_with(".md") {
        return "docs".to_string();
    }
    if normalized.contains("migration") || normalized.starts_with("migrations/") {
        return "migrations".to_string();
    }
    "other".to_string()
}

fn is_execution_surface(path: &str) -> bool {
    let normalized = path.trim_matches('/');
    normalized.starts_with(".github/workflows/")
        || normalized.ends_with("build.rs")
        || normalized.starts_with("scripts/")
        || normalized.ends_with(".sh")
}

#[cfg(test)]
mod tests {
    use super::*;
    use faultline_types::{ChangeStatus, PathChange};
    use proptest::prelude::*;
    use std::collections::HashMap;

    fn pc(path: &str) -> PathChange {
        PathChange {
            status: ChangeStatus::Modified,
            path: path.into(),
        }
    }

    // --- bucket_name tests ---

    #[test]
    fn bucket_name_extracts_top_level_directory() {
        assert_eq!(bucket_name("src/lib.rs"), "src");
        assert_eq!(bucket_name("crates/foo/bar.rs"), "crates");
        assert_eq!(bucket_name(".github/workflows/ci.yml"), ".github");
    }

    #[test]
    fn bucket_name_root_level_file_uses_filename() {
        assert_eq!(bucket_name("Cargo.lock"), "Cargo.lock");
        assert_eq!(bucket_name("README.md"), "README.md");
    }

    #[test]
    fn bucket_name_empty_path_returns_repo_root() {
        assert_eq!(bucket_name(""), "repo-root");
        assert_eq!(bucket_name("/"), "repo-root");
    }

    // --- surface_kind tests ---

    #[test]
    fn surface_kind_source() {
        assert_eq!(surface_kind("src/main.rs"), "source");
        assert_eq!(surface_kind("crates/foo/src/lib.rs"), "source");
    }

    #[test]
    fn surface_kind_tests() {
        assert_eq!(surface_kind("tests/integration.rs"), "tests");
        assert_eq!(surface_kind("crates/foo/tests/bar.rs"), "tests");
    }

    #[test]
    fn surface_kind_benchmarks() {
        assert_eq!(surface_kind("benches/perf.rs"), "benchmarks");
        assert_eq!(surface_kind("crates/foo/benches/bench.rs"), "benchmarks");
    }

    #[test]
    fn surface_kind_scripts() {
        assert_eq!(surface_kind("scripts/deploy.sh"), "scripts");
        assert_eq!(surface_kind("tools/run.sh"), "scripts");
    }

    #[test]
    fn surface_kind_workflows() {
        assert_eq!(surface_kind(".github/workflows/ci.yml"), "workflows");
        assert_eq!(surface_kind(".github/workflows/release.yml"), "workflows");
    }

    #[test]
    fn surface_kind_docs() {
        assert_eq!(surface_kind("docs/guide.md"), "docs");
        assert_eq!(surface_kind("README.md"), "docs");
    }

    #[test]
    fn surface_kind_build_script() {
        assert_eq!(surface_kind("build.rs"), "build-script");
        assert_eq!(surface_kind("crates/foo/build.rs"), "build-script");
    }

    #[test]
    fn surface_kind_lockfile() {
        assert_eq!(surface_kind("Cargo.lock"), "lockfile");
        assert_eq!(surface_kind("package-lock.json"), "lockfile");
        assert_eq!(surface_kind("yarn.lock"), "lockfile");
        assert_eq!(surface_kind("sub/Cargo.lock"), "lockfile");
    }

    #[test]
    fn surface_kind_migrations() {
        assert_eq!(surface_kind("migrations/001_init.sql"), "migrations");
        assert_eq!(surface_kind("db/migration_v2.sql"), "migrations");
    }

    #[test]
    fn surface_kind_other() {
        assert_eq!(surface_kind("Cargo.toml"), "other");
        assert_eq!(surface_kind(".gitignore"), "other");
    }

    // --- is_execution_surface tests ---

    #[test]
    fn execution_surface_workflows() {
        assert!(is_execution_surface(".github/workflows/ci.yml"));
    }

    #[test]
    fn execution_surface_build_scripts() {
        assert!(is_execution_surface("build.rs"));
        assert!(is_execution_surface("crates/foo/build.rs"));
    }

    #[test]
    fn execution_surface_shell_scripts() {
        assert!(is_execution_surface("scripts/deploy.sh"));
        assert!(is_execution_surface("tools/run.sh"));
    }

    #[test]
    fn non_execution_surface() {
        assert!(!is_execution_surface("src/main.rs"));
        assert!(!is_execution_surface("Cargo.toml"));
        assert!(!is_execution_surface("docs/guide.md"));
    }

    // --- summarize integration tests ---

    #[test]
    fn summarize_empty_input() {
        let analyzer = SurfaceAnalyzer;
        let summary = analyzer.summarize(&[]);
        assert_eq!(summary.total_changes, 0);
        assert!(summary.buckets.is_empty());
        assert!(summary.execution_surfaces.is_empty());
    }

    #[test]
    fn summarize_groups_by_top_level_directory() {
        let analyzer = SurfaceAnalyzer;
        let summary =
            analyzer.summarize(&[pc("src/lib.rs"), pc("src/main.rs"), pc("tests/basic.rs")]);
        assert_eq!(summary.total_changes, 3);
        assert_eq!(summary.buckets.len(), 2);

        let src_bucket = summary.buckets.iter().find(|b| b.name == "src").unwrap();
        assert_eq!(src_bucket.change_count, 2);
        assert_eq!(src_bucket.paths, vec!["src/lib.rs", "src/main.rs"]);
        assert!(src_bucket.surface_kinds.contains(&"source".to_string()));

        let test_bucket = summary.buckets.iter().find(|b| b.name == "tests").unwrap();
        assert_eq!(test_bucket.change_count, 1);
    }

    #[test]
    fn summarize_collects_execution_surfaces() {
        let analyzer = SurfaceAnalyzer;
        let summary = analyzer.summarize(&[
            pc("src/lib.rs"),
            pc(".github/workflows/ci.yml"),
            pc("build.rs"),
            pc("scripts/deploy.sh"),
        ]);
        assert_eq!(summary.execution_surfaces.len(), 3);
        assert!(
            summary
                .execution_surfaces
                .contains(&".github/workflows/ci.yml".to_string())
        );
        assert!(summary.execution_surfaces.contains(&"build.rs".to_string()));
        assert!(
            summary
                .execution_surfaces
                .contains(&"scripts/deploy.sh".to_string())
        );
    }

    #[test]
    fn summarize_assigns_multiple_surface_kinds_per_bucket() {
        let analyzer = SurfaceAnalyzer;
        let summary = analyzer.summarize(&[
            pc("crates/foo/src/lib.rs"),
            pc("crates/foo/tests/bar.rs"),
            pc("crates/foo/build.rs"),
        ]);
        assert_eq!(summary.total_changes, 3);
        let bucket = summary.buckets.iter().find(|b| b.name == "crates").unwrap();
        assert_eq!(bucket.change_count, 3);
        assert!(bucket.surface_kinds.contains(&"source".to_string()));
        assert!(bucket.surface_kinds.contains(&"tests".to_string()));
        assert!(bucket.surface_kinds.contains(&"build-script".to_string()));
    }

    #[test]
    fn summarize_total_changes_equals_input_length() {
        let analyzer = SurfaceAnalyzer;
        let changes = vec![
            pc("src/a.rs"),
            pc("src/b.rs"),
            pc("tests/c.rs"),
            pc("Cargo.lock"),
            pc(".github/workflows/ci.yml"),
        ];
        let summary = analyzer.summarize(&changes);
        assert_eq!(summary.total_changes, changes.len());
    }

    #[test]
    fn summarize_every_path_in_exactly_one_bucket() {
        let analyzer = SurfaceAnalyzer;
        let changes = vec![
            pc("src/a.rs"),
            pc("tests/b.rs"),
            pc("docs/c.md"),
            pc("Cargo.lock"),
        ];
        let summary = analyzer.summarize(&changes);
        let all_paths: Vec<&String> = summary.buckets.iter().flat_map(|b| &b.paths).collect();
        assert_eq!(all_paths.len(), changes.len());
        for change in &changes {
            assert!(
                all_paths.contains(&&change.path),
                "path {} not found in any bucket",
                change.path
            );
        }
    }

    // --- rank_suspect_surface tests ---

    fn pc_with_status(path: &str, status: ChangeStatus) -> PathChange {
        PathChange {
            status,
            path: path.into(),
        }
    }

    #[test]
    fn rank_suspect_surface_empty_input() {
        let analyzer = SurfaceAnalyzer;
        let owners = HashMap::new();
        let result = analyzer.rank_suspect_surface(&[], &owners);
        assert!(result.is_empty());
    }

    #[test]
    fn rank_suspect_surface_single_modified_source() {
        let analyzer = SurfaceAnalyzer;
        let owners = HashMap::new();
        let changes = vec![pc_with_status("src/main.rs", ChangeStatus::Modified)];
        let result = analyzer.rank_suspect_surface(&changes, &owners);
        assert_eq!(result.len(), 1);
        // base 100 + source 50 = 150
        assert_eq!(result[0].priority_score, 150);
        assert_eq!(result[0].surface_kind, "source");
        assert!(!result[0].is_execution_surface);
        assert_eq!(result[0].change_status, ChangeStatus::Modified);
        assert_eq!(result[0].owner_hint, None);
    }

    #[test]
    fn rank_suspect_surface_scoring_rules() {
        let analyzer = SurfaceAnalyzer;
        let owners = HashMap::new();
        let changes = vec![
            pc_with_status(".github/workflows/ci.yml", ChangeStatus::Modified), // exec: 100+200 = 300
            pc_with_status("src/lib.rs", ChangeStatus::Deleted), // source+deleted: 100+150+50 = 300
            pc_with_status("src/old.rs", ChangeStatus::Renamed), // source+renamed: 100+100+50 = 250
            pc_with_status("src/main.rs", ChangeStatus::Modified), // source: 100+50 = 150
            pc_with_status("tests/basic.rs", ChangeStatus::Modified), // test: 100+25 = 125
            pc_with_status("Cargo.toml", ChangeStatus::Modified), // other: 100
        ];
        let result = analyzer.rank_suspect_surface(&changes, &owners);
        assert_eq!(result.len(), 6);
        // Descending score, ascending path for ties
        assert_eq!(result[0].priority_score, 300);
        assert_eq!(result[1].priority_score, 300);
        // Tie-break: ascending path
        assert_eq!(result[0].path, ".github/workflows/ci.yml");
        assert_eq!(result[1].path, "src/lib.rs");
        assert_eq!(result[2].priority_score, 250);
        assert_eq!(result[3].priority_score, 150);
        assert_eq!(result[4].priority_score, 125);
        assert_eq!(result[5].priority_score, 100);
    }

    #[test]
    fn rank_suspect_surface_owner_hints() {
        let analyzer = SurfaceAnalyzer;
        let mut owners = HashMap::new();
        owners.insert("src/main.rs".to_string(), Some("alice".to_string()));
        owners.insert("src/lib.rs".to_string(), None);
        let changes = vec![
            pc_with_status("src/main.rs", ChangeStatus::Modified),
            pc_with_status("src/lib.rs", ChangeStatus::Modified),
            pc_with_status("src/other.rs", ChangeStatus::Modified),
        ];
        let result = analyzer.rank_suspect_surface(&changes, &owners);
        assert_eq!(result.len(), 3);
        let main_entry = result.iter().find(|e| e.path == "src/main.rs").unwrap();
        assert_eq!(main_entry.owner_hint, Some("alice".to_string()));
        let lib_entry = result.iter().find(|e| e.path == "src/lib.rs").unwrap();
        assert_eq!(lib_entry.owner_hint, None);
        let other_entry = result.iter().find(|e| e.path == "src/other.rs").unwrap();
        assert_eq!(other_entry.owner_hint, None);
    }

    #[test]
    fn rank_suspect_surface_execution_surface_deleted() {
        let analyzer = SurfaceAnalyzer;
        let owners = HashMap::new();
        // Deleted execution surface: 100 + 200 + 150 = 450
        let changes = vec![pc_with_status("scripts/deploy.sh", ChangeStatus::Deleted)];
        let result = analyzer.rank_suspect_surface(&changes, &owners);
        assert_eq!(result[0].priority_score, 450);
        assert!(result[0].is_execution_surface);
        assert_eq!(result[0].change_status, ChangeStatus::Deleted);
    }

    // --- Proptest strategies for Property 13: Surface Analysis Invariants ---

    const VALID_SURFACE_KINDS: &[&str] = &[
        "source",
        "tests",
        "benchmarks",
        "scripts",
        "workflows",
        "docs",
        "build-script",
        "lockfile",
        "migrations",
        "other",
    ];

    fn arb_change_status() -> impl Strategy<Value = ChangeStatus> {
        prop_oneof![
            Just(ChangeStatus::Added),
            Just(ChangeStatus::Modified),
            Just(ChangeStatus::Deleted),
            Just(ChangeStatus::Renamed),
            Just(ChangeStatus::TypeChanged),
            Just(ChangeStatus::Unknown),
        ]
    }

    fn arb_path_change() -> impl Strategy<Value = PathChange> {
        (
            arb_change_status(),
            "[a-z][a-z0-9_]{0,9}(/[a-z][a-z0-9_.]{0,9}){0,3}",
        )
            .prop_map(|(status, path)| PathChange { status, path })
    }

    // Feature: v01-release-train, Property 13: Surface Analysis Invariants
    // **Validates: Requirements 5.2, 5.3, 5.4**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_surface_analysis_invariants(changes in prop::collection::vec(arb_path_change(), 1..30)) {
            let analyzer = SurfaceAnalyzer;
            let summary = analyzer.summarize(&changes);

            // (a) total_changes == input length
            prop_assert_eq!(
                summary.total_changes, changes.len(),
                "total_changes must equal input length"
            );

            // (b) every input path appears in exactly one bucket
            let all_bucket_paths: Vec<&String> = summary.buckets.iter().flat_map(|b| &b.paths).collect();
            prop_assert_eq!(
                all_bucket_paths.len(), changes.len(),
                "sum of bucket path counts must equal input length (no duplicates across buckets)"
            );
            for change in &changes {
                let count = all_bucket_paths.iter().filter(|p| ***p == change.path).count();
                prop_assert!(
                    count >= 1,
                    "path '{}' must appear in at least one bucket", change.path
                );
            }

            // (c) bucket names match top-level dirs of their paths
            for bucket in &summary.buckets {
                for path in &bucket.paths {
                    let expected_bucket = bucket_name(path);
                    prop_assert_eq!(
                        &bucket.name, &expected_bucket,
                        "path '{}' should be in bucket '{}' but found in '{}'",
                        path, expected_bucket, bucket.name
                    );
                }
            }

            // (d) valid surface kinds
            for bucket in &summary.buckets {
                for kind in &bucket.surface_kinds {
                    prop_assert!(
                        VALID_SURFACE_KINDS.contains(&kind.as_str()),
                        "surface kind '{}' is not in the valid set", kind
                    );
                }
            }

            // (e) execution_surfaces is a subset of input paths
            let input_paths: std::collections::BTreeSet<&str> = changes.iter().map(|c| c.path.as_str()).collect();
            for exec_path in &summary.execution_surfaces {
                prop_assert!(
                    input_paths.contains(exec_path.as_str()),
                    "execution surface '{}' must be a path from the input", exec_path
                );
                // Also verify it actually qualifies as an execution surface
                prop_assert!(
                    is_execution_surface(exec_path),
                    "execution surface '{}' must satisfy is_execution_surface()", exec_path
                );
            }
        }
    }

    // --- Proptest strategies for Properties 43–46: Suspect Surface Ranking ---

    /// Strategy that generates a mixed set of PathChange values guaranteed to contain
    /// at least one execution surface, one rename, one delete, and one ordinary modified source.
    fn arb_mixed_path_changes() -> impl Strategy<Value = Vec<PathChange>> {
        // Fixed entries that guarantee the required mix
        let exec_surface = Just(PathChange {
            status: ChangeStatus::Modified,
            path: ".github/workflows/ci.yml".into(),
        });
        let renamed = Just(PathChange {
            status: ChangeStatus::Renamed,
            path: "src/renamed.rs".into(),
        });
        let deleted = Just(PathChange {
            status: ChangeStatus::Deleted,
            path: "src/deleted.rs".into(),
        });
        let ordinary_source = Just(PathChange {
            status: ChangeStatus::Modified,
            path: "src/ordinary.rs".into(),
        });
        // Additional random changes
        let extras = prop::collection::vec(arb_path_change(), 0..10);

        (exec_surface, renamed, deleted, ordinary_source, extras).prop_map(
            |(exec, ren, del, ord, mut extra)| {
                let mut changes = vec![exec, ren, del, ord];
                changes.append(&mut extra);
                changes
            },
        )
    }

    // Feature: v01-product-sharpening, Property 43: Suspect surface ranking is sorted and deterministic
    // **Validates: Requirements 1.1, 1.10**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_suspect_ranking_sorted_and_deterministic(
            changes in prop::collection::vec(arb_path_change(), 0..30)
        ) {
            let analyzer = SurfaceAnalyzer;
            // Build a deterministic owners map from the generated changes
            let owners: HashMap<String, Option<String>> = HashMap::new();

            let result1 = analyzer.rank_suspect_surface(&changes, &owners);
            let result2 = analyzer.rank_suspect_surface(&changes, &owners);

            // Deterministic: identical calls produce identical output
            prop_assert_eq!(&result1, &result2, "rank_suspect_surface must be deterministic");

            // Sorted: descending score, ascending path for ties
            for window in result1.windows(2) {
                let a = &window[0];
                let b = &window[1];
                prop_assert!(
                    a.priority_score > b.priority_score
                        || (a.priority_score == b.priority_score && a.path <= b.path),
                    "entries must be sorted by descending score then ascending path: \
                     ({}, {}) vs ({}, {})",
                    a.priority_score, a.path, b.priority_score, b.path
                );
            }
        }
    }

    // Feature: v01-product-sharpening, Property 44: Execution surfaces, renames, and deletes score higher than ordinary modifications
    // **Validates: Requirements 1.2**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_exec_rename_delete_score_higher_than_ordinary(
            changes in arb_mixed_path_changes()
        ) {
            let analyzer = SurfaceAnalyzer;
            let owners = HashMap::new();
            let result = analyzer.rank_suspect_surface(&changes, &owners);

            // Find the ordinary modified source entry
            let ordinary_score = result
                .iter()
                .find(|e| e.path == "src/ordinary.rs")
                .expect("ordinary source must be present")
                .priority_score;

            // Execution surface must score higher
            let exec_score = result
                .iter()
                .find(|e| e.path == ".github/workflows/ci.yml")
                .expect("execution surface must be present")
                .priority_score;
            prop_assert!(
                exec_score > ordinary_score,
                "execution surface score {} must be > ordinary source score {}",
                exec_score, ordinary_score
            );

            // Renamed file must score higher
            let renamed_score = result
                .iter()
                .find(|e| e.path == "src/renamed.rs")
                .expect("renamed file must be present")
                .priority_score;
            prop_assert!(
                renamed_score > ordinary_score,
                "renamed file score {} must be > ordinary source score {}",
                renamed_score, ordinary_score
            );

            // Deleted file must score higher
            let deleted_score = result
                .iter()
                .find(|e| e.path == "src/deleted.rs")
                .expect("deleted file must be present")
                .priority_score;
            prop_assert!(
                deleted_score > ordinary_score,
                "deleted file score {} must be > ordinary source score {}",
                deleted_score, ordinary_score
            );
        }
    }

    // Feature: v01-product-sharpening, Property 45: SuspectEntry preserves change_status and has consistent surface_kind
    // **Validates: Requirements 1.6, 1.7**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_suspect_entry_preserves_status_and_surface_kind(
            change in arb_path_change()
        ) {
            let analyzer = SurfaceAnalyzer;
            let owners = HashMap::new();
            let result = analyzer.rank_suspect_surface(&[change.clone()], &owners);

            prop_assert_eq!(result.len(), 1, "single input must produce single entry");
            let entry = &result[0];

            // change_status must match input
            prop_assert_eq!(
                &entry.change_status, &change.status,
                "change_status must match input PathChange.status"
            );

            // surface_kind must match the classification function
            let expected_kind = surface_kind(&change.path);
            prop_assert_eq!(
                &entry.surface_kind, &expected_kind,
                "surface_kind must match surface_kind() for path '{}'", change.path
            );

            // is_execution_surface must match the classification function
            let expected_exec = is_execution_surface(&change.path);
            prop_assert_eq!(
                entry.is_execution_surface, expected_exec,
                "is_execution_surface must match is_execution_surface() for path '{}'", change.path
            );
        }
    }

    // Feature: v01-product-sharpening, Property 46: SuspectEntry owner_hint matches the provided owners map
    // **Validates: Requirements 1.3, 1.4, 1.5**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_suspect_entry_owner_hint_matches_map(
            changes in prop::collection::vec(arb_path_change(), 1..20)
        ) {
            let analyzer = SurfaceAnalyzer;

            // Build a random owners map: for each path, randomly decide whether to include it
            // and if so, randomly assign Some(owner) or None
            let mut owners: HashMap<String, Option<String>> = HashMap::new();
            // Use a simple deterministic pattern based on path length for variety
            for (i, change) in changes.iter().enumerate() {
                match i % 3 {
                    0 => { owners.insert(change.path.clone(), Some(format!("owner{}", i))); }
                    1 => { owners.insert(change.path.clone(), None); }
                    _ => { /* absent from map */ }
                }
            }

            let result = analyzer.rank_suspect_surface(&changes, &owners);

            for entry in &result {
                let expected = owners.get(&entry.path).cloned().flatten();
                prop_assert_eq!(
                    &entry.owner_hint, &expected,
                    "owner_hint for path '{}' must match owners map (expected {:?}, got {:?})",
                    entry.path, expected, entry.owner_hint
                );
            }
        }
    }
}
