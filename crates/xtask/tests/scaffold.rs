// Feature: repo-operating-system, Property 47: Scaffold Crate Generation
// Feature: repo-operating-system, Property 48: Scaffold ADR Sequential Numbering
// Feature: repo-operating-system, Property 49: Scaffold File Generation
// Feature: repo-operating-system, Property 50: Scaffold Input Validation

use proptest::prelude::*;
use tempfile::TempDir;

/// Create a minimal workspace root in a temp directory with the required structure.
fn setup_workspace(tmp: &TempDir) -> std::path::PathBuf {
    let root = tmp.path().to_path_buf();

    // Minimal workspace Cargo.toml
    std::fs::write(
        root.join("Cargo.toml"),
        r#"[workspace]
members = [
  "crates/xtask",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "Apache-2.0"
authors = ["Test"]
"#,
    )
    .unwrap();

    // docs/adr/ with TEMPLATE.md
    let adr_dir = root.join("docs/adr");
    std::fs::create_dir_all(&adr_dir).unwrap();
    std::fs::write(
        adr_dir.join("TEMPLATE.md"),
        r#"# ADR-NNNN: <title>

## Status

Proposed

## Context

## Decision

## Consequences

## Related Patterns
"#,
    )
    .unwrap();

    // docs/scenarios/scenario_index.md
    let scenarios_dir = root.join("docs/scenarios");
    std::fs::create_dir_all(&scenarios_dir).unwrap();
    std::fs::write(
        scenarios_dir.join("scenario_index.md"),
        "# Scenario Index\n\n| Scenario | Problem | Fixture/Generator | Crate(s) | Artifact | Invariant | Refs |\n|----------|---------|-------------------|----------|----------|-----------|------|\n",
    )
    .unwrap();

    // docs/book/src/SUMMARY.md with section headings
    let book_src = root.join("docs/book/src");
    std::fs::create_dir_all(book_src.join("tutorials")).unwrap();
    std::fs::create_dir_all(book_src.join("howto")).unwrap();
    std::fs::create_dir_all(book_src.join("explanations")).unwrap();
    std::fs::create_dir_all(book_src.join("reference")).unwrap();
    std::fs::write(
        book_src.join("SUMMARY.md"),
        r#"# Summary

## Tutorials

- [First Run](tutorials/first-run.md)

## How-To Guides

- [Add Test](howto/add-test.md)

## Explanations

- [Localization](explanations/localization.md)

## Reference

- [CLI Flags](reference/cli-flags.md)
"#,
    )
    .unwrap();

    // crates/ directory
    std::fs::create_dir_all(root.join("crates")).unwrap();

    root
}

/// Strategy for generating valid crate name suffixes.
fn arb_crate_suffix() -> impl Strategy<Value = String> {
    // Start with a lowercase letter, then lowercase alphanumeric or hyphens
    // Ensure no trailing hyphen and length 1..20
    "[a-z][a-z0-9]{0,8}(-[a-z][a-z0-9]{0,4}){0,2}"
}

/// Strategy for generating valid tier names.
fn arb_tier() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("domain".into()),
        Just("adapter".into()),
        Just("app".into()),
    ]
}

// =========================================================================
// Property 47: Scaffold Crate Generation
// =========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

    // Feature: repo-operating-system, Property 47: Scaffold Crate Generation
    // **Validates: Requirements 10.2**
    #[test]
    fn prop_scaffold_crate_generation(
        suffix in arb_crate_suffix(),
        tier in arb_tier(),
    ) {
        let tmp = TempDir::new().unwrap();
        let root = setup_workspace(&tmp);
        let name = format!("faultline-{suffix}");

        let result = xtask::scaffold::scaffold_crate(&root, &name, &tier);
        prop_assert!(result.is_ok(), "scaffold_crate failed: {:?}", result.err());

        // Verify Cargo.toml exists and inherits workspace metadata
        let cargo_toml_path = root.join("crates").join(&name).join("Cargo.toml");
        prop_assert!(cargo_toml_path.exists(), "Cargo.toml not created");
        let cargo_content = std::fs::read_to_string(&cargo_toml_path).unwrap();
        prop_assert!(cargo_content.contains("version.workspace = true"),
            "Cargo.toml missing workspace version inheritance");
        prop_assert!(cargo_content.contains("edition.workspace = true"),
            "Cargo.toml missing workspace edition inheritance");
        prop_assert!(cargo_content.contains(&format!("name = \"{name}\"")),
            "Cargo.toml missing crate name");

        // Verify src/lib.rs exists with doc comment
        let lib_rs_path = root.join("crates").join(&name).join("src/lib.rs");
        prop_assert!(lib_rs_path.exists(), "src/lib.rs not created");
        let lib_content = std::fs::read_to_string(&lib_rs_path).unwrap();
        prop_assert!(lib_content.starts_with("//!"), "src/lib.rs missing doc comment");

        // Verify crate name appears in workspace Cargo.toml members
        let ws_cargo = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
        prop_assert!(ws_cargo.contains(&format!("\"crates/{name}\"")),
            "crate not added to workspace members");
    }
}

// =========================================================================
// Property 48: Scaffold ADR Sequential Numbering
// =========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

    // Feature: repo-operating-system, Property 48: Scaffold ADR Sequential Numbering
    // **Validates: Requirements 10.3**
    #[test]
    fn prop_scaffold_adr_sequential_numbering(
        existing_count in 0u32..20,
    ) {
        let tmp = TempDir::new().unwrap();
        let root = setup_workspace(&tmp);
        let adr_dir = root.join("docs/adr");

        // Create `existing_count` ADR files with sequential numbering
        for i in 1..=existing_count {
            let filename = format!("{:04}-existing-adr-{}.md", i, i);
            std::fs::write(adr_dir.join(&filename), "# placeholder").unwrap();
        }

        // Scaffold a new ADR
        let result = xtask::scaffold::scaffold_adr(&root, "Test Decision");
        prop_assert!(result.is_ok(), "scaffold_adr failed: {:?}", result.err());

        // The next number should be existing_count + 1
        let expected_num = existing_count + 1;
        let expected_prefix = format!("{:04}", expected_num);

        // Find the new ADR file
        let entries: Vec<_> = std::fs::read_dir(&adr_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with(&expected_prefix) && name.ends_with(".md") && name != "TEMPLATE.md"
            })
            .collect();

        prop_assert!(
            entries.len() == 1,
            "expected exactly one ADR with prefix {}, found {}",
            expected_prefix,
            entries.len()
        );

        // Verify the file uses the template (has "## Status" section)
        let content = std::fs::read_to_string(entries[0].path()).unwrap();
        prop_assert!(content.contains("## Status"), "ADR missing Status section from template");
        prop_assert!(content.contains("Test Decision"), "ADR missing title");
    }
}

// =========================================================================
// Property 49: Scaffold File Generation for Scenarios and Docs
// =========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

    // Feature: repo-operating-system, Property 49: Scaffold File Generation
    // **Validates: Requirements 10.4, 10.5**
    #[test]
    fn prop_scaffold_scenario_creates_stub_and_index(
        name in "[a-z][a-z0-9_]{1,10}",
    ) {
        let tmp = TempDir::new().unwrap();
        let root = setup_workspace(&tmp);

        // Create the target crate directory
        let crate_name = "faultline-test";
        std::fs::create_dir_all(root.join("crates").join(crate_name)).unwrap();

        let result = xtask::scaffold::scaffold_scenario(&root, &name, crate_name);
        prop_assert!(result.is_ok(), "scaffold_scenario failed: {:?}", result.err());

        // Verify test file stub exists
        let test_file = root
            .join("crates")
            .join(crate_name)
            .join("tests")
            .join(format!("{}.rs", name.replace('-', "_")));
        prop_assert!(test_file.exists(), "test stub not created at {:?}", test_file);

        let test_content = std::fs::read_to_string(&test_file).unwrap();
        prop_assert!(test_content.contains("#[test]"), "test stub missing #[test] attribute");

        // Verify scenario index entry
        let index = std::fs::read_to_string(root.join("docs/scenarios/scenario_index.md")).unwrap();
        prop_assert!(index.contains(&name), "scenario index missing entry for {}", name);
    }

    // Feature: repo-operating-system, Property 49: Scaffold File Generation
    // **Validates: Requirements 10.4, 10.5**
    #[test]
    fn prop_scaffold_doc_creates_file_and_summary_entry(
        section in prop_oneof![
            Just("tutorial"),
            Just("howto"),
            Just("explanation"),
            Just("reference"),
        ],
    ) {
        let tmp = TempDir::new().unwrap();
        let root = setup_workspace(&tmp);

        let title = "My Test Page";
        let result = xtask::scaffold::scaffold_doc(&root, title, &section);
        prop_assert!(result.is_ok(), "scaffold_doc failed: {:?}", result.err());

        // Determine expected directory
        let dir_name = match section.as_ref() {
            "tutorial" => "tutorials",
            "howto" => "howto",
            "explanation" => "explanations",
            "reference" => "reference",
            _ => unreachable!(),
        };

        // Verify Markdown file exists
        let doc_file = root
            .join("docs/book/src")
            .join(dir_name)
            .join("my-test-page.md");
        prop_assert!(doc_file.exists(), "doc file not created at {:?}", doc_file);

        let doc_content = std::fs::read_to_string(&doc_file).unwrap();
        prop_assert!(doc_content.contains(title), "doc file missing title");

        // Verify SUMMARY.md entry
        let summary = std::fs::read_to_string(root.join("docs/book/src/SUMMARY.md")).unwrap();
        prop_assert!(
            summary.contains("My Test Page"),
            "SUMMARY.md missing entry for doc"
        );
    }
}

// =========================================================================
// Property 50: Scaffold Input Validation
// =========================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

    // Feature: repo-operating-system, Property 50: Scaffold Input Validation
    // **Validates: Requirements 10.6**
    #[test]
    fn prop_scaffold_rejects_invalid_crate_names(
        name in prop_oneof![
            Just("foo".to_string()),
            Just("faultline-".to_string()),
            Just("faultline-1bad".to_string()),
            Just("faultline-Foo".to_string()),
            Just("".to_string()),
            Just("FAULTLINE-foo".to_string()),
            Just("faultline_foo".to_string()),
            "[A-Z][a-z]{0,5}".prop_map(|s| s),
            "[0-9]{1,5}".prop_map(|s| format!("faultline-{s}")),
        ],
    ) {
        let result = xtask::scaffold::validate_crate_name(&name);
        prop_assert!(result.is_err(), "expected rejection for crate name: {}", name);
        let err_msg = format!("{}", result.unwrap_err());
        prop_assert!(
            err_msg.contains("crate name must match"),
            "error message missing expected text, got: {}",
            err_msg
        );
    }

    // Feature: repo-operating-system, Property 50: Scaffold Input Validation
    // **Validates: Requirements 10.6**
    #[test]
    fn prop_scaffold_rejects_empty_adr_titles(
        title in prop_oneof![
            Just("".to_string()),
            Just("   ".to_string()),
            Just("\t".to_string()),
        ],
    ) {
        let result = xtask::scaffold::validate_non_empty(&title, "ADR title");
        prop_assert!(result.is_err(), "expected rejection for empty ADR title");
    }

    // Feature: repo-operating-system, Property 50: Scaffold Input Validation
    // **Validates: Requirements 10.6**
    #[test]
    fn prop_scaffold_rejects_empty_scenario_names(
        name in prop_oneof![
            Just("".to_string()),
            Just("   ".to_string()),
        ],
    ) {
        let result = xtask::scaffold::validate_non_empty(&name, "scenario name");
        prop_assert!(result.is_err(), "expected rejection for empty scenario name");
    }

    // Feature: repo-operating-system, Property 50: Scaffold Input Validation
    // **Validates: Requirements 10.6**
    #[test]
    fn prop_scaffold_rejects_invalid_doc_sections(
        section in prop_oneof![
            Just("tutorials".to_string()),
            Just("blog".to_string()),
            Just("".to_string()),
            Just("TUTORIAL".to_string()),
            Just("how-to".to_string()),
            Just("ref".to_string()),
            "[a-z]{1,8}".prop_filter("must not be valid section", |s| {
                !["tutorial", "howto", "explanation", "reference"].contains(&s.as_str())
            }),
        ],
    ) {
        let result = xtask::scaffold::validate_section(&section);
        prop_assert!(result.is_err(), "expected rejection for section: {}", section);
        let err_msg = format!("{}", result.unwrap_err());
        prop_assert!(
            err_msg.contains("section must be one of"),
            "error message missing expected text, got: {}",
            err_msg
        );
    }
}
