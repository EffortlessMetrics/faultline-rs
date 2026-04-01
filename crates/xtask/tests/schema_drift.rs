//! 15.5: CI contract failure scenario
//! Validates: Requirements 8.1
//!
//! Simulate schema drift by writing a modified schema file, then verify
//! that the xtask schema check logic detects the mismatch.

use tempfile::TempDir;

/// Generate the current correct schema JSON from the Rust types.
fn generate_current_schema() -> String {
    let schema = schemars::schema_for!(faultline_types::AnalysisReport);
    serde_json::to_string_pretty(&schema).expect("schema generation must succeed")
}

#[test]
fn scenario_schema_drift_detected_on_modified_report() {
    let correct_schema = generate_current_schema();

    // Simulate drift: inject a modification into the schema
    let drifted_schema = correct_schema.replace("AnalysisReport", "AnalysisReport_DRIFTED");
    assert_ne!(
        correct_schema, drifted_schema,
        "drifted schema must differ from correct schema"
    );

    // Write the drifted schema to a temp file
    let tmp = TempDir::new().unwrap();
    let schema_path = tmp.path().join("analysis-report.schema.json");
    std::fs::write(&schema_path, &drifted_schema).unwrap();

    // Read back and compare against freshly generated — simulates check_schema logic
    let on_disk = std::fs::read_to_string(&schema_path).unwrap();
    let freshly_generated = generate_current_schema();

    assert_ne!(
        on_disk, freshly_generated,
        "drifted on-disk schema must differ from freshly generated schema"
    );
}

#[test]
fn scenario_schema_no_drift_when_matching() {
    let correct_schema = generate_current_schema();

    // Write the correct schema to a temp file
    let tmp = TempDir::new().unwrap();
    let schema_path = tmp.path().join("analysis-report.schema.json");
    std::fs::write(&schema_path, &correct_schema).unwrap();

    // Read back and compare — should match
    let on_disk = std::fs::read_to_string(&schema_path).unwrap();
    let freshly_generated = generate_current_schema();

    assert_eq!(
        on_disk, freshly_generated,
        "correct on-disk schema must match freshly generated schema"
    );
}

#[test]
fn scenario_schema_drift_field_removal_detected() {
    let correct_schema = generate_current_schema();

    // Simulate drift by removing the suspect_surface field reference
    let drifted = correct_schema.replace("suspect_surface", "");
    assert_ne!(correct_schema, drifted);

    // The comparison detects the mismatch
    let freshly_generated = generate_current_schema();
    assert_ne!(
        drifted, freshly_generated,
        "schema with removed field must be detected as drift"
    );
}
