use faultline_types::{PathChange, SubsystemBucket, SurfaceSummary};
use std::collections::{BTreeMap, BTreeSet};

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
}
