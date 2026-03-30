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
    if normalized == "Cargo.lock" {
        return "lockfile".to_string();
    }
    if normalized.starts_with(".github/workflows/") {
        return "workflow".to_string();
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

    #[test]
    fn buckets_changes_by_top_level_path() {
        let analyzer = SurfaceAnalyzer;
        let summary = analyzer.summarize(&[
            PathChange {
                status: ChangeStatus::Modified,
                path: "src/lib.rs".into(),
            },
            PathChange {
                status: ChangeStatus::Modified,
                path: "tests/basic.rs".into(),
            },
            PathChange {
                status: ChangeStatus::Modified,
                path: ".github/workflows/ci.yml".into(),
            },
        ]);
        assert_eq!(summary.total_changes, 3);
        assert_eq!(summary.buckets.len(), 3);
        assert_eq!(summary.execution_surfaces.len(), 1);
    }
}
