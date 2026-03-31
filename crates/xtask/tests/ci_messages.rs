// Feature: repo-operating-system, Property 44: Tool Detection Error Messages
// Feature: repo-operating-system, Property 46: CI Failure Messages Identify Broken Contract

use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Property 44 — Tool Detection Error Messages
// **Validates: Requirements 5.7**
//
// For a set of known tool names, verify the error message contains both the
// tool name and an install command.
// ---------------------------------------------------------------------------

/// Known tools and their install commands used by xtask.
fn known_tools() -> Vec<(&'static str, &'static str)> {
    vec![
        ("cargo-insta", "cargo install cargo-insta"),
        ("cargo-mutants", "cargo install cargo-mutants"),
        ("cargo-fuzz", "cargo install cargo-fuzz"),
        ("mdbook", "cargo install mdbook"),
        ("cargo-deny", "cargo install cargo-deny"),
        ("cargo-audit", "cargo install cargo-audit"),
        ("cargo-semver-checks", "cargo install cargo-semver-checks"),
    ]
}

#[test]
fn tool_detection_error_messages_known_tools() {
    // Feature: repo-operating-system, Property 44: Tool Detection Error Messages
    // **Validates: Requirements 5.7**

    for (name, install_cmd) in known_tools() {
        let msg = xtask::tools::missing_tool_message(name, install_cmd);

        assert!(
            msg.contains(name),
            "Error message for tool '{name}' does not contain the tool name.\nMessage: {msg}"
        );
        assert!(
            msg.contains(install_cmd),
            "Error message for tool '{name}' does not contain the install command.\nMessage: {msg}"
        );
        assert!(
            msg.contains("error:"),
            "Error message for tool '{name}' does not contain 'error:' prefix.\nMessage: {msg}"
        );
        assert!(
            msg.contains("install:"),
            "Error message for tool '{name}' does not contain 'install:' directive.\nMessage: {msg}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // Feature: repo-operating-system, Property 44: Tool Detection Error Messages
    // **Validates: Requirements 5.7**
    #[test]
    fn tool_detection_error_message_format(
        name in "[a-z][a-z0-9-]{1,30}",
        install_cmd in "cargo install [a-z][a-z0-9-]{1,30}",
    ) {
        let msg = xtask::tools::missing_tool_message(&name, &install_cmd);

        // Message must contain the tool name
        prop_assert!(
            msg.contains(&name),
            "Error message does not contain tool name '{}'.\nMessage: {}",
            name,
            msg
        );

        // Message must contain the install command
        prop_assert!(
            msg.contains(&install_cmd),
            "Error message does not contain install command '{}'.\nMessage: {}",
            install_cmd,
            msg
        );

        // Message must have the expected structure
        prop_assert!(
            msg.starts_with("error:"),
            "Error message does not start with 'error:'.\nMessage: {}",
            msg
        );
        prop_assert!(
            msg.contains("install:"),
            "Error message does not contain 'install:' directive.\nMessage: {}",
            msg
        );
    }
}

// ---------------------------------------------------------------------------
// Property 46 — CI Failure Messages Identify Broken Contract
// **Validates: Requirements 8.7**
//
// For each contract check failure type (schema drift, stale golden, missing
// scenario), verify the error message contains the contract name and a
// documentation reference.
// ---------------------------------------------------------------------------

#[test]
fn ci_failure_messages_identify_broken_contract() {
    // Feature: repo-operating-system, Property 46: CI Failure Messages Identify Broken Contract
    // **Validates: Requirements 8.7**

    // 1. Contract-broken messages contain the contract name
    let contracts = [
        "code formatting",
        "lint warnings",
        "test suite",
        "supply-chain policy",
        "security audit",
        "semver compatibility",
    ];

    for contract in &contracts {
        let msg = xtask::ci::contract_broken_message(contract);
        assert!(
            msg.contains(contract),
            "Contract broken message does not contain contract name '{contract}'.\nMessage: {msg}"
        );
        assert!(
            msg.contains("contract broken:"),
            "Contract broken message missing 'contract broken:' prefix.\nMessage: {msg}"
        );
    }

    // 2. Golden failure message contains contract name and doc reference
    let golden_msg = xtask::ci::golden_failure_message("analysis.json");
    assert!(
        golden_msg.contains("golden artifact"),
        "Golden failure message missing 'golden artifact'.\nMessage: {golden_msg}"
    );
    assert!(
        golden_msg.contains("analysis.json"),
        "Golden failure message missing artifact name.\nMessage: {golden_msg}"
    );
    assert!(
        golden_msg.contains("cargo insta review"),
        "Golden failure message missing remediation command.\nMessage: {golden_msg}"
    );
    assert!(
        golden_msg.contains("TESTING.md"),
        "Golden failure message missing documentation reference.\nMessage: {golden_msg}"
    );

    // 3. Schema drift message contains contract name and doc reference
    let schema_msg = xtask::ci::schema_drift_message();
    assert!(
        schema_msg.contains("schema drift"),
        "Schema drift message missing 'schema drift'.\nMessage: {schema_msg}"
    );
    assert!(
        schema_msg.contains("cargo xtask generate-schema"),
        "Schema drift message missing remediation command.\nMessage: {schema_msg}"
    );
    assert!(
        schema_msg.contains("TESTING.md"),
        "Schema drift message missing documentation reference.\nMessage: {schema_msg}"
    );

    // 4. Missing scenario message contains contract name and doc reference
    let scenario_msg = xtask::ci::missing_scenario_message("test_foo.rs, test_bar.rs");
    assert!(
        scenario_msg.contains("scenario atlas"),
        "Missing scenario message missing 'scenario atlas'.\nMessage: {scenario_msg}"
    );
    assert!(
        scenario_msg.contains("test_foo.rs"),
        "Missing scenario message missing file names.\nMessage: {scenario_msg}"
    );
    assert!(
        scenario_msg.contains("TESTING.md"),
        "Missing scenario message missing documentation reference.\nMessage: {scenario_msg}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // Feature: repo-operating-system, Property 46: CI Failure Messages Identify Broken Contract
    // **Validates: Requirements 8.7**
    #[test]
    fn ci_contract_broken_message_contains_contract_name(
        contract in "[a-z][a-z ]{2,40}",
    ) {
        let msg = xtask::ci::contract_broken_message(&contract);

        prop_assert!(
            msg.contains(&contract),
            "Contract broken message does not contain contract name '{}'.\nMessage: {}",
            contract,
            msg
        );
        prop_assert!(
            msg.contains("contract broken:"),
            "Contract broken message missing 'contract broken:' prefix.\nMessage: {}",
            msg
        );
    }

    #[test]
    fn ci_golden_failure_message_contains_artifact_and_docs(
        artifact in "[a-z][a-z0-9_.]{1,30}",
    ) {
        let msg = xtask::ci::golden_failure_message(&artifact);

        prop_assert!(
            msg.contains(&artifact),
            "Golden failure message does not contain artifact name '{}'.\nMessage: {}",
            artifact,
            msg
        );
        prop_assert!(
            msg.contains("TESTING.md"),
            "Golden failure message missing documentation reference.\nMessage: {}",
            msg
        );
        prop_assert!(
            msg.contains("cargo insta review"),
            "Golden failure message missing remediation command.\nMessage: {}",
            msg
        );
    }

    #[test]
    fn ci_missing_scenario_message_contains_files_and_docs(
        files in "[a-z][a-z0-9_.]{1,30}(, [a-z][a-z0-9_.]{1,30}){0,3}",
    ) {
        let msg = xtask::ci::missing_scenario_message(&files);

        prop_assert!(
            msg.contains(&files),
            "Missing scenario message does not contain file names '{}'.\nMessage: {}",
            files,
            msg
        );
        prop_assert!(
            msg.contains("scenario atlas"),
            "Missing scenario message missing 'scenario atlas'.\nMessage: {}",
            msg
        );
        prop_assert!(
            msg.contains("TESTING.md"),
            "Missing scenario message missing documentation reference.\nMessage: {}",
            msg
        );
    }
}
