//! End-to-end tests for `inspect-run` and `bundle` CLI subcommands.
//!
//! Validates Requirements: 4.1, 4.2, 4.5, 4.6, 5.2, 5.3, 5.4, 5.5, 5.7
//!
//! These tests invoke the actual CLI binary via `std::process::Command`.

use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Locate the faultline-cli binary built by cargo.
fn cli_binary() -> PathBuf {
    let mut path = std::env::current_exe()
        .expect("current_exe")
        .parent()
        .expect("parent of test binary")
        .parent()
        .expect("parent of deps dir")
        .to_path_buf();
    if cfg!(windows) {
        path.push("faultline-cli.exe");
    } else {
        path.push("faultline-cli");
    }
    path
}

/// Create a minimal valid AnalysisReport JSON string.
fn minimal_report_json() -> String {
    serde_json::json!({
        "schema_version": "0.3.0",
        "run_id": "test-run-inspect-001",
        "created_at_epoch_seconds": 1700000000_u64,
        "request": {
            "repo_root": "/tmp/repo",
            "good": "aaaa1111",
            "bad": "bbbb2222",
            "history_mode": "AncestryPath",
            "probe": {
                "Shell": {
                    "kind": "Test",
                    "shell": "Default",
                    "script": "echo hello",
                    "env": [["MY_SECRET", "s3cr3t"]],
                    "timeout_seconds": 30
                }
            },
            "policy": {
                "max_probes": 64,
                "flake_policy": {
                    "retries": 0,
                    "stability_threshold": 1.0
                }
            }
        },
        "sequence": {
            "revisions": ["aaaa1111", "cccc3333", "bbbb2222"]
        },
        "observations": [
            {
                "commit": "aaaa1111",
                "class": "Pass",
                "kind": "Test",
                "exit_code": 0,
                "timed_out": false,
                "duration_ms": 100,
                "stdout": "ok",
                "stderr": "",
                "sequence_index": 0,
                "signal_number": null,
                "probe_command": "echo hello",
                "working_dir": "/tmp/repo"
            },
            {
                "commit": "bbbb2222",
                "class": "Fail",
                "kind": "Test",
                "exit_code": 1,
                "timed_out": false,
                "duration_ms": 100,
                "stdout": "",
                "stderr": "fail",
                "sequence_index": 2,
                "signal_number": null,
                "probe_command": "echo hello",
                "working_dir": "/tmp/repo"
            }
        ],
        "outcome": {
            "FirstBad": {
                "last_good": "aaaa1111",
                "first_bad": "bbbb2222",
                "confidence": {
                    "score": 100,
                    "label": "high"
                }
            }
        },
        "changed_paths": [],
        "surface": {
            "total_changes": 0,
            "buckets": [],
            "execution_surfaces": []
        },
        "suspect_surface": [],
        "reproduction_capsules": [
            {
                "commit": "aaaa1111",
                "predicate": {
                    "Shell": {
                        "kind": "Test",
                        "shell": "Default",
                        "script": "echo hello",
                        "env": [],
                        "timeout_seconds": 30
                    }
                },
                "env": [],
                "working_dir": "/tmp/repo",
                "timeout_seconds": 30
            }
        ]
    })
    .to_string()
}

/// Set up a run directory with report.json and some ancillary files.
fn setup_run_dir() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    std::fs::write(dir.path().join("report.json"), minimal_report_json())
        .expect("write report.json");
    std::fs::write(
        dir.path().join("observations.json"),
        r#"[{"commit":"aaaa1111"},{"commit":"bbbb2222"}]"#,
    )
    .expect("write observations.json");
    std::fs::write(
        dir.path().join("metadata.json"),
        r#"{"schema_version":"0.3.0","tool_version":"0.1.0"}"#,
    )
    .expect("write metadata.json");
    // Create logs directory with a couple of files
    let logs_dir = dir.path().join("logs");
    std::fs::create_dir(&logs_dir).expect("create logs dir");
    std::fs::write(logs_dir.join("probe-0.log"), "log content 0").expect("write log 0");
    std::fs::write(logs_dir.join("probe-1.log"), "log content 1").expect("write log 1");
    dir
}

// ============================================================================
// inspect-run tests — Requirements 4.1, 4.2, 4.5
// ============================================================================

#[test]
fn inspect_run_lists_files_with_descriptions() {
    let dir = setup_run_dir();
    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "inspect-run",
            "--run-dir",
            &dir.path().display().to_string(),
        ])
        .output()
        .expect("failed to execute CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "inspect-run should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Should list known files with descriptions
    assert!(
        stdout.contains("report.json"),
        "should list report.json.\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("observations.json"),
        "should list observations.json.\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("metadata.json"),
        "should list metadata.json.\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("logs/"),
        "should list logs/ directory.\nstdout:\n{stdout}"
    );
    // Descriptions should be present (at least one known description)
    assert!(
        stdout.contains("Full unredacted AnalysisReport")
            || stdout.contains("Cached probe observations"),
        "should include file descriptions.\nstdout:\n{stdout}"
    );
}

#[test]
fn inspect_run_extracts_report_metadata() {
    let dir = setup_run_dir();
    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "inspect-run",
            "--run-dir",
            &dir.path().display().to_string(),
        ])
        .output()
        .expect("failed to execute CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "inspect-run should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Should extract and display report metadata
    assert!(
        stdout.contains("test-run-inspect-001"),
        "should display run ID.\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("0.3.0"),
        "should display schema version.\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("FirstBad"),
        "should display outcome type.\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("1700000000"),
        "should display created_at timestamp.\nstdout:\n{stdout}"
    );
}

#[test]
fn inspect_run_json_emits_valid_json() {
    let dir = setup_run_dir();
    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "inspect-run",
            "--run-dir",
            &dir.path().display().to_string(),
            "--json",
        ])
        .output()
        .expect("failed to execute CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "inspect-run --json should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Parse the output as JSON
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("output should be valid JSON: {e}\nstdout:\n{stdout}"));

    // Verify expected fields exist
    assert!(
        parsed.get("discovered_files").is_some(),
        "JSON should have discovered_files field.\nparsed:\n{parsed}"
    );
    assert!(
        parsed.get("report_summary").is_some(),
        "JSON should have report_summary field.\nparsed:\n{parsed}"
    );

    // Verify report_summary has expected content
    let summary = parsed.get("report_summary").unwrap();
    assert_eq!(
        summary.get("run_id").and_then(|v| v.as_str()),
        Some("test-run-inspect-001")
    );
    assert_eq!(
        summary.get("schema_version").and_then(|v| v.as_str()),
        Some("0.3.0")
    );
    assert_eq!(
        summary.get("outcome_type").and_then(|v| v.as_str()),
        Some("FirstBad")
    );
    assert_eq!(
        summary.get("observation_count").and_then(|v| v.as_u64()),
        Some(2)
    );

    // Verify observation_count from observations.json
    assert!(
        parsed.get("observation_count").is_some(),
        "JSON should have observation_count field"
    );

    // Verify log_file_count
    assert_eq!(
        parsed.get("log_file_count").and_then(|v| v.as_u64()),
        Some(2),
        "should report 2 log files"
    );

    // report_parse_error should be null (report parsed OK)
    assert!(
        parsed.get("report_parse_error").unwrap().is_null(),
        "report_parse_error should be null when report parses OK"
    );
}

#[test]
fn inspect_run_json_with_unparseable_report_has_parse_error_field() {
    let dir = TempDir::new().expect("create temp dir");
    // Write invalid JSON as report.json
    std::fs::write(dir.path().join("report.json"), "{ not valid json !!!")
        .expect("write invalid report.json");

    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "inspect-run",
            "--run-dir",
            &dir.path().display().to_string(),
            "--json",
        ])
        .output()
        .expect("failed to execute CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should still exit 0 in --json mode even with unparseable report
    assert_eq!(
        output.status.code(),
        Some(0),
        "inspect-run --json should exit 0 even with unparseable report.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Parse the output as JSON (should always be well-formed)
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("output should be valid JSON: {e}\nstdout:\n{stdout}"));

    // report_summary should be null
    assert!(
        parsed.get("report_summary").unwrap().is_null(),
        "report_summary should be null when report is unparseable"
    );

    // report_parse_error should be a non-null string
    let parse_error = parsed.get("report_parse_error").unwrap();
    assert!(
        parse_error.is_string(),
        "report_parse_error should be a string.\nparsed:\n{parsed}"
    );
    assert!(
        parse_error.as_str().unwrap().contains("failed to parse"),
        "report_parse_error should describe the parse failure.\nvalue: {}",
        parse_error
    );
}

#[test]
fn inspect_run_errors_on_missing_directory() {
    let dir = TempDir::new().expect("create temp dir");
    let nonexistent = dir.path().join("does-not-exist");

    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "inspect-run",
            "--run-dir",
            &nonexistent.display().to_string(),
        ])
        .output()
        .expect("failed to execute CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should exit with code 2 (ExecutionError)
    assert_eq!(
        output.status.code(),
        Some(2),
        "inspect-run on missing dir should exit 2.\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("does not exist"),
        "error should mention directory does not exist.\nstderr:\n{stderr}"
    );
}

// ============================================================================
// bundle tests — Requirements 5.2, 5.3, 5.4, 5.5, 5.7
// ============================================================================

#[test]
fn bundle_generates_all_core_artifacts_fresh() {
    let source_dir = setup_run_dir();
    let output_dir = TempDir::new().expect("create output dir");
    let bundle_dest = output_dir.path().join("bundle-out");

    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "bundle",
            "--source",
            &source_dir.path().display().to_string(),
            "--output",
            &bundle_dest.display().to_string(),
        ])
        .output()
        .expect("failed to execute CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "bundle should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Core artifacts should exist in the bundle output
    assert!(
        bundle_dest.join("analysis.json").exists(),
        "bundle should contain analysis.json"
    );
    assert!(
        bundle_dest.join("index.html").exists(),
        "bundle should contain index.html"
    );
    assert!(
        bundle_dest.join("dossier.md").exists(),
        "bundle should contain dossier.md"
    );

    // SARIF should NOT be present (not requested)
    assert!(
        !bundle_dest.join("results.sarif.json").exists(),
        "bundle should NOT contain SARIF when --include-sarif is not passed"
    );

    // Stdout should mention artifact count
    assert!(
        stdout.contains("artifacts"),
        "bundle output should mention artifact count.\nstdout:\n{stdout}"
    );
}

#[test]
fn bundle_include_sarif_adds_sarif() {
    let source_dir = setup_run_dir();
    let output_dir = TempDir::new().expect("create output dir");
    let bundle_dest = output_dir.path().join("bundle-sarif");

    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "bundle",
            "--source",
            &source_dir.path().display().to_string(),
            "--output",
            &bundle_dest.display().to_string(),
            "--include-sarif",
        ])
        .output()
        .expect("failed to execute CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "bundle --include-sarif should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // SARIF should be present
    assert!(
        bundle_dest.join("results.sarif.json").exists(),
        "bundle should contain results.sarif.json when --include-sarif is passed"
    );

    // Core artifacts should still be present
    assert!(
        bundle_dest.join("analysis.json").exists(),
        "bundle should still contain analysis.json"
    );
    assert!(
        bundle_dest.join("index.html").exists(),
        "bundle should still contain index.html"
    );
    assert!(
        bundle_dest.join("dossier.md").exists(),
        "bundle should still contain dossier.md"
    );
}

#[test]
fn bundle_without_include_sarif_excludes_sarif() {
    let source_dir = setup_run_dir();
    let output_dir = TempDir::new().expect("create output dir");
    let bundle_dest = output_dir.path().join("bundle-no-sarif");

    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "bundle",
            "--source",
            &source_dir.path().display().to_string(),
            "--output",
            &bundle_dest.display().to_string(),
        ])
        .output()
        .expect("failed to execute CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "bundle should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // SARIF should NOT be present
    assert!(
        !bundle_dest.join("results.sarif.json").exists(),
        "bundle should NOT contain results.sarif.json without --include-sarif"
    );
}

#[test]
fn bundle_format_tar_gz_creates_archive() {
    let source_dir = setup_run_dir();
    let output_dir = TempDir::new().expect("create output dir");
    let archive_path = output_dir.path().join("bundle.tar.gz");

    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "bundle",
            "--source",
            &source_dir.path().display().to_string(),
            "--output",
            &archive_path.display().to_string(),
            "--format",
            "tar-gz",
        ])
        .output()
        .expect("failed to execute CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "bundle --format tar-gz should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Archive file should exist and be non-empty
    assert!(
        archive_path.exists(),
        "tar.gz archive should exist at {}",
        archive_path.display()
    );
    let metadata = std::fs::metadata(&archive_path).expect("read archive metadata");
    assert!(
        metadata.len() > 0,
        "tar.gz archive should be non-empty (size: {})",
        metadata.len()
    );

    // Verify gzip magic bytes (1f 8b)
    let content = std::fs::read(&archive_path).expect("read archive content");
    assert!(
        content.len() >= 2 && content[0] == 0x1f && content[1] == 0x8b,
        "archive should start with gzip magic bytes (1f 8b), got: {:02x} {:02x}",
        content.get(0).copied().unwrap_or(0),
        content.get(1).copied().unwrap_or(0)
    );
}

#[test]
fn bundle_errors_on_empty_source() {
    let empty_dir = TempDir::new().expect("create empty dir");
    let output_dir = TempDir::new().expect("create output dir");
    let bundle_dest = output_dir.path().join("bundle-fail");

    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "bundle",
            "--source",
            &empty_dir.path().display().to_string(),
            "--output",
            &bundle_dest.display().to_string(),
        ])
        .output()
        .expect("failed to execute CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should exit with code 2 (ExecutionError)
    assert_eq!(
        output.status.code(),
        Some(2),
        "bundle on empty source should exit 2.\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("no loadable report"),
        "error should mention no loadable report.\nstderr:\n{stderr}"
    );
}
