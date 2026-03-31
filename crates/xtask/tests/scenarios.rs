// Feature: repo-operating-system, Property 39: Scenario Atlas Consistency
// **Validates: Requirements 2.5, 8.4**
//
// Generate random sets of test names and index entries, verify the
// verification logic reports exactly the symmetric difference.

use proptest::prelude::*;
use std::collections::BTreeSet;

use xtask::scenarios::{check_consistency, extract_index_entries, extract_test_names};

// ---------------------------------------------------------------------------
// Unit tests for extraction helpers
// ---------------------------------------------------------------------------

#[test]
fn extract_test_names_finds_standard_tests() {
    let source = r#"
#[test]
fn my_test_one() {
    assert!(true);
}

#[test]
fn my_test_two() {
    assert!(true);
}
"#;
    let names = extract_test_names(source);
    assert!(names.contains("my_test_one"));
    assert!(names.contains("my_test_two"));
    assert_eq!(names.len(), 2);
}

#[test]
fn extract_test_names_finds_proptest_fns() {
    let source = r#"
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_something(x in 0..100u32) {
        prop_assert!(x < 100);
    }
}
"#;
    let names = extract_test_names(source);
    assert!(names.contains("prop_something"));
}

#[test]
fn extract_index_entries_parses_table() {
    let index = r#"
# Scenario Index

| Scenario | Problem | Fixture | Crate | Artifact | Invariant | Refs |
|----------|---------|---------|-------|----------|-----------|------|
| `test_alpha` | Does alpha | Hand-built | crate-a | — | Correctness | — |
| `test_beta` | Does beta | Generator | crate-b | — | Validity | — |
"#;
    let entries = extract_index_entries(index);
    assert!(entries.contains("test_alpha"));
    assert!(entries.contains("test_beta"));
    assert_eq!(entries.len(), 2);
}

#[test]
fn check_consistency_reports_symmetric_difference() {
    let mut workspace: BTreeSet<String> = BTreeSet::new();
    workspace.insert("test_a".into());
    workspace.insert("test_b".into());
    workspace.insert("test_c".into());

    let mut index: BTreeSet<String> = BTreeSet::new();
    index.insert("test_b".into());
    index.insert("test_c".into());
    index.insert("test_d".into());

    let result = check_consistency(&workspace, &index);

    // test_a is in workspace but not index → missing
    assert!(result.missing_from_index.contains("test_a"));
    assert_eq!(result.missing_from_index.len(), 1);

    // test_d is in index but not workspace → stale
    assert!(result.stale_in_index.contains("test_d"));
    assert_eq!(result.stale_in_index.len(), 1);

    assert!(!result.is_ok());
}

#[test]
fn check_consistency_perfect_match_is_ok() {
    let mut workspace: BTreeSet<String> = BTreeSet::new();
    workspace.insert("test_x".into());
    workspace.insert("test_y".into());

    let index = workspace.clone();
    let result = check_consistency(&workspace, &index);

    assert!(result.is_ok());
    assert!(result.missing_from_index.is_empty());
    assert!(result.stale_in_index.is_empty());
}

// ---------------------------------------------------------------------------
// Property 39 — Scenario Atlas Consistency
// **Validates: Requirements 2.5, 8.4**
// ---------------------------------------------------------------------------

/// Generate a set of plausible test names.
fn arb_test_name_set() -> impl Strategy<Value = BTreeSet<String>> {
    prop::collection::btree_set("[a-z][a-z0-9_]{2,30}", 0..20)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // Feature: repo-operating-system, Property 39: Scenario Atlas Consistency
    // **Validates: Requirements 2.5, 8.4**
    #[test]
    fn prop_symmetric_difference_is_exact(
        workspace_tests in arb_test_name_set(),
        index_entries in arb_test_name_set(),
    ) {
        let result = check_consistency(&workspace_tests, &index_entries);

        // missing_from_index = workspace_tests - index_entries
        let expected_missing: BTreeSet<String> = workspace_tests
            .difference(&index_entries)
            .cloned()
            .collect();
        prop_assert_eq!(
            &result.missing_from_index,
            &expected_missing,
            "missing_from_index should be workspace \\ index"
        );

        // stale_in_index = index_entries - workspace_tests
        let expected_stale: BTreeSet<String> = index_entries
            .difference(&workspace_tests)
            .cloned()
            .collect();
        prop_assert_eq!(
            &result.stale_in_index,
            &expected_stale,
            "stale_in_index should be index \\ workspace"
        );

        // is_ok iff both sets are equal
        prop_assert_eq!(
            result.is_ok(),
            workspace_tests == index_entries,
            "is_ok should be true iff sets are equal"
        );
    }

    // Feature: repo-operating-system, Property 39: Scenario Atlas Consistency
    // **Validates: Requirements 2.5, 8.4**
    #[test]
    fn prop_identical_sets_yield_ok(
        tests in arb_test_name_set(),
    ) {
        let result = check_consistency(&tests, &tests);
        prop_assert!(result.is_ok(), "identical sets should yield ok");
        prop_assert!(result.missing_from_index.is_empty());
        prop_assert!(result.stale_in_index.is_empty());
    }

    // Feature: repo-operating-system, Property 39: Scenario Atlas Consistency
    // **Validates: Requirements 2.5, 8.4**
    #[test]
    fn prop_extract_and_check_round_trip(
        names in prop::collection::btree_set("[a-z][a-z_]{3,20}", 1..10),
    ) {
        // Build a fake source file with #[test] fns
        let mut source = String::new();
        for name in &names {
            source.push_str(&format!("#[test]\nfn {name}() {{}}\n\n"));
        }

        // Build a fake index with the same names
        let mut index = String::from("| Scenario | Problem | Fix | Crate | Art | Inv | Refs |\n");
        index.push_str("|----------|---------|-----|-------|-----|-----|------|\n");
        for name in &names {
            index.push_str(&format!("| `{name}` | desc | fix | c | — | inv | — |\n"));
        }

        let extracted_tests = extract_test_names(&source);
        let extracted_index = extract_index_entries(&index);
        let result = check_consistency(&extracted_tests, &extracted_index);

        prop_assert!(
            result.is_ok(),
            "round-trip with same names should be ok, but got: missing={:?}, stale={:?}",
            result.missing_from_index,
            result.stale_in_index
        );
    }
}
