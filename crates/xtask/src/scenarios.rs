//! Scenario atlas verification logic.
//!
//! Compares test function names found in workspace source files against
//! entries listed in `docs/scenarios/scenario_index.md`. Reports the
//! symmetric difference: tests missing from the index and index entries
//! referencing tests that no longer exist.

use std::collections::BTreeSet;
use std::path::Path;

use regex::Regex;

/// Result of comparing workspace tests against the scenario index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioCheckResult {
    /// Test functions found in source but not listed in the scenario index.
    pub missing_from_index: BTreeSet<String>,
    /// Entries in the scenario index that reference tests not found in source.
    pub stale_in_index: BTreeSet<String>,
}

impl ScenarioCheckResult {
    pub fn is_ok(&self) -> bool {
        self.missing_from_index.is_empty() && self.stale_in_index.is_empty()
    }
}

/// Extract `#[test]` function names from Rust source content.
///
/// Looks for patterns like:
///   `fn some_test_name(`  preceded by `#[test]` (possibly with other attributes in between)
///
/// Also extracts test names from `proptest! { ... #[test] fn name(` blocks.
pub fn extract_test_names(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    // Match #[test] fn <name>(
    let test_fn_re = Regex::new(r"#\[test\]\s*\n\s*fn\s+(\w+)\s*\(").unwrap();
    for cap in test_fn_re.captures_iter(source) {
        if let Some(m) = cap.get(1) {
            names.insert(m.as_str().to_string());
        }
    }

    // Match fn <name> inside proptest! blocks — these look like:
    //   fn name(  (inside a proptest! { ... } macro)
    // We detect proptest blocks and extract fn names within them.
    let proptest_fn_re = Regex::new(r"(?s)proptest!\s*\{(.*?)\}").unwrap();
    let inner_fn_re = Regex::new(r"fn\s+(\w+)\s*\(").unwrap();
    for block in proptest_fn_re.captures_iter(source) {
        if let Some(body) = block.get(1) {
            for cap in inner_fn_re.captures_iter(body.as_str()) {
                if let Some(m) = cap.get(1) {
                    names.insert(m.as_str().to_string());
                }
            }
        }
    }

    names
}

/// Extract scenario names from the scenario index markdown content.
///
/// Looks for backtick-quoted names in the first column of markdown tables:
///   `| \`some_test_name\` | ... |`
pub fn extract_index_entries(index_content: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    for line in index_content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }

        // Split by pipe, get first data cell
        let cells: Vec<&str> = trimmed.split('|').collect();
        if cells.len() < 3 {
            continue;
        }

        let first_cell = cells[1].trim();

        // Skip header rows and separator rows
        if first_cell == "Scenario" || first_cell.chars().all(|c| c == '-' || c == ':' || c == ' ')
        {
            continue;
        }

        // Extract name from backticks: `name`
        if first_cell.starts_with('`') && first_cell.ends_with('`') && first_cell.len() > 2 {
            let name = &first_cell[1..first_cell.len() - 1];
            if !name.is_empty() {
                names.insert(name.to_string());
            }
        }
    }

    names
}

/// Compare two sets of test names and return the symmetric difference.
///
/// This is the core verification logic, independent of file I/O.
pub fn check_consistency(
    workspace_tests: &BTreeSet<String>,
    index_entries: &BTreeSet<String>,
) -> ScenarioCheckResult {
    let missing_from_index: BTreeSet<String> =
        workspace_tests.difference(index_entries).cloned().collect();

    let stale_in_index: BTreeSet<String> =
        index_entries.difference(workspace_tests).cloned().collect();

    ScenarioCheckResult {
        missing_from_index,
        stale_in_index,
    }
}

/// Scan all Rust source files under a directory for test function names.
pub fn scan_workspace_tests(root: &Path) -> BTreeSet<String> {
    let mut all_tests = BTreeSet::new();
    let crates_dir = root.join("crates");

    if !crates_dir.is_dir() {
        return all_tests;
    }

    scan_dir_recursive(&crates_dir, &mut all_tests);
    all_tests
}

fn scan_dir_recursive(dir: &Path, tests: &mut BTreeSet<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip target directories and hidden directories
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "target" || name.starts_with('.') || name == "snapshots" {
                continue;
            }
            scan_dir_recursive(&path, tests);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
            && let Ok(content) = std::fs::read_to_string(&path)
        {
            let names = extract_test_names(&content);
            tests.extend(names);
        }
    }
}

/// Read the scenario index file and extract entries.
pub fn read_scenario_index(root: &Path) -> BTreeSet<String> {
    let index_path = root.join("docs/scenarios/scenario_index.md");
    match std::fs::read_to_string(&index_path) {
        Ok(content) => extract_index_entries(&content),
        Err(_) => BTreeSet::new(),
    }
}
