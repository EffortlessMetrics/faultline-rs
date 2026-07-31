//! End-to-end tests for `inspect-run` and `bundle` CLI subcommands.
//!
//! Validates Requirements: 4.1, 4.2, 4.5, 4.6, 5.2, 5.3, 5.4, 5.5, 5.7
//!
//! These tests invoke the actual CLI binary via `std::process::Command`.
//!
//! All test functions return `Result<(), anyhow::Error>` and use the
//! fallible-helper macros from `faultline-fixtures` (`ensure`, `ensure_eq`,
//! `require_ok`, `require_some`) instead of `unwrap()`/`expect()`/`panic!()`,
//! per the no-panic policy.

use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

use faultline_fixtures::{ensure, ensure_eq, require_ok, require_some};

/// Locate the faultline-cli binary built by cargo.
fn cli_binary() -> anyhow::Result<PathBuf> {
    let path = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("current_exe has no parent"))?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("no parent of deps dir"))?
        .to_path_buf();
    Ok(if cfg!(windows) {
        path.join("faultline-cli.exe")
    } else {
        path.join("faultline-cli")
    })
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
fn setup_run_dir() -> anyhow::Result<TempDir> {
    let dir = TempDir::new()?;
    require_ok!(std::fs::write(
        dir.path().join("report.json"),
        minimal_report_json()
    ));
    require_ok!(std::fs::write(
        dir.path().join("observations.json"),
        r#"[{"commit":"aaaa1111"},{"commit":"bbbb2222"}]"#,
    ));
    require_ok!(std::fs::write(
        dir.path().join("metadata.json"),
        r#"{"schema_version":"0.3.0","tool_version":"0.1.0"}"#,
    ));
    // Create logs directory with a couple of files
    let logs_dir = dir.path().join("logs");
    require_ok!(std::fs::create_dir(&logs_dir));
    require_ok!(std::fs::write(
        logs_dir.join("probe-0.log"),
        "log content 0"
    ));
    require_ok!(std::fs::write(
        logs_dir.join("probe-1.log"),
        "log content 1"
    ));
    Ok(dir)
}

// ============================================================================
// inspect-run tests — Requirements 4.1, 4.2, 4.5
// ============================================================================

#[test]
fn inspect_run_lists_files_with_descriptions() -> anyhow::Result<()> {
    let dir = setup_run_dir()?;
    let bin = cli_binary()?;
    ensure!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "inspect-run",
            "--run-dir",
            &dir.path().display().to_string(),
        ])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    ensure_eq!(
        output.status.code(),
        Some(0),
        "inspect-run should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Should list known files with descriptions
    ensure!(
        stdout.contains("report.json"),
        "should list report.json.\nstdout:\n{stdout}"
    );
    ensure!(
        stdout.contains("observations.json"),
        "should list observations.json.\nstdout:\n{stdout}"
    );
    ensure!(
        stdout.contains("metadata.json"),
        "should list metadata.json.\nstdout:\n{stdout}"
    );
    ensure!(
        stdout.contains("logs/"),
        "should list logs/ directory.\nstdout:\n{stdout}"
    );
    // Descriptions should be present (at least one known description)
    ensure!(
        stdout.contains("Full unredacted AnalysisReport")
            || stdout.contains("Cached probe observations"),
        "should include file descriptions.\nstdout:\n{stdout}"
    );
    Ok(())
}

#[test]
fn inspect_run_extracts_report_metadata() -> anyhow::Result<()> {
    let dir = setup_run_dir()?;
    let bin = cli_binary()?;
    ensure!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "inspect-run",
            "--run-dir",
            &dir.path().display().to_string(),
        ])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    ensure_eq!(
        output.status.code(),
        Some(0),
        "inspect-run should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Should extract and display report metadata
    ensure!(
        stdout.contains("test-run-inspect-001"),
        "should display run ID.\nstdout:\n{stdout}"
    );
    ensure!(
        stdout.contains("0.3.0"),
        "should display schema version.\nstdout:\n{stdout}"
    );
    ensure!(
        stdout.contains("FirstBad"),
        "should display outcome type.\nstdout:\n{stdout}"
    );
    ensure!(
        stdout.contains("1700000000"),
        "should display created_at timestamp.\nstdout:\n{stdout}"
    );
    Ok(())
}

#[test]
fn inspect_run_json_emits_valid_json() -> anyhow::Result<()> {
    let dir = setup_run_dir()?;
    let bin = cli_binary()?;
    ensure!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "inspect-run",
            "--run-dir",
            &dir.path().display().to_string(),
            "--json",
        ])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    ensure_eq!(
        output.status.code(),
        Some(0),
        "inspect-run --json should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Parse the output as JSON
    let parsed: serde_json::Value = require_ok!(
        serde_json::from_str(&stdout),
        "output should be valid JSON\nstdout:\n{stdout}"
    );

    // Verify expected fields exist
    ensure!(
        parsed.get("discovered_files").is_some(),
        "JSON should have discovered_files field.\nparsed:\n{parsed}"
    );
    ensure!(
        parsed.get("report_summary").is_some(),
        "JSON should have report_summary field.\nparsed:\n{parsed}"
    );

    // Verify report_summary has expected content
    let summary = require_some!(parsed.get("report_summary"), "missing report_summary");
    ensure_eq!(
        summary.get("run_id").and_then(|v| v.as_str()),
        Some("test-run-inspect-001")
    );
    ensure_eq!(
        summary.get("schema_version").and_then(|v| v.as_str()),
        Some("0.3.0")
    );
    ensure_eq!(
        summary.get("outcome_type").and_then(|v| v.as_str()),
        Some("FirstBad")
    );
    ensure_eq!(
        summary.get("observation_count").and_then(|v| v.as_u64()),
        Some(2)
    );

    // Verify observation_count from observations.json
    ensure!(
        parsed.get("observation_count").is_some(),
        "JSON should have observation_count field"
    );

    // Verify log_file_count
    ensure_eq!(
        parsed.get("log_file_count").and_then(|v| v.as_u64()),
        Some(2),
        "should report 2 log files"
    );

    // report_parse_error should be null (report parsed OK)
    let rpe = require_some!(
        parsed.get("report_parse_error"),
        "missing report_parse_error"
    );
    ensure!(
        rpe.is_null(),
        "report_parse_error should be null when report parses OK"
    );
    Ok(())
}

#[test]
fn inspect_run_json_with_unparseable_report_has_parse_error_field() -> anyhow::Result<()> {
    let dir = TempDir::new()?;
    // Write invalid JSON as report.json
    require_ok!(std::fs::write(
        dir.path().join("report.json"),
        "{ not valid json !!!"
    ));

    let bin = cli_binary()?;
    ensure!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "inspect-run",
            "--run-dir",
            &dir.path().display().to_string(),
            "--json",
        ])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should still exit 0 in --json mode even with unparseable report
    ensure_eq!(
        output.status.code(),
        Some(0),
        "inspect-run --json should exit 0 even with unparseable report.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Parse the output as JSON (should always be well-formed)
    let parsed: serde_json::Value = require_ok!(
        serde_json::from_str(&stdout),
        "output should be valid JSON\nstdout:\n{stdout}"
    );

    // report_summary should be null
    let rs = require_some!(parsed.get("report_summary"), "missing report_summary");
    ensure!(
        rs.is_null(),
        "report_summary should be null when report is unparseable"
    );

    // report_parse_error should be a non-null string
    let parse_error = require_some!(
        parsed.get("report_parse_error"),
        "missing report_parse_error"
    );
    ensure!(
        parse_error.is_string(),
        "report_parse_error should be a string.\nparsed:\n{parsed}"
    );
    let pe_str = require_some!(parse_error.as_str(), "report_parse_error not a string");
    ensure!(
        pe_str.contains("failed to parse"),
        "report_parse_error should describe the parse failure.\nvalue: {parse_error}"
    );
    Ok(())
}

#[test]
fn inspect_run_errors_on_missing_directory() -> anyhow::Result<()> {
    let dir = TempDir::new()?;
    let nonexistent = dir.path().join("does-not-exist");

    let bin = cli_binary()?;
    ensure!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "inspect-run",
            "--run-dir",
            &nonexistent.display().to_string(),
        ])
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should exit with code 2 (ExecutionError)
    ensure_eq!(
        output.status.code(),
        Some(2),
        "inspect-run on missing dir should exit 2.\nstderr:\n{stderr}"
    );
    ensure!(
        stderr.contains("does not exist"),
        "error should mention directory does not exist.\nstderr:\n{stderr}"
    );
    Ok(())
}

// ============================================================================
// bundle tests — Requirements 5.2, 5.3, 5.4, 5.5, 5.7
// ============================================================================

#[test]
fn bundle_generates_all_core_artifacts_fresh() -> anyhow::Result<()> {
    let source_dir = setup_run_dir()?;
    let output_dir = TempDir::new()?;
    let bundle_dest = output_dir.path().join("bundle-out");

    let bin = cli_binary()?;
    ensure!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "bundle",
            "--source",
            &source_dir.path().display().to_string(),
            "--output",
            &bundle_dest.display().to_string(),
        ])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    ensure_eq!(
        output.status.code(),
        Some(0),
        "bundle should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Core artifacts should exist in the bundle output
    ensure!(
        bundle_dest.join("analysis.json").exists(),
        "bundle should contain analysis.json"
    );
    ensure!(
        bundle_dest.join("index.html").exists(),
        "bundle should contain index.html"
    );
    ensure!(
        bundle_dest.join("dossier.md").exists(),
        "bundle should contain dossier.md"
    );

    // SARIF should NOT be present (not requested)
    ensure!(
        !bundle_dest.join("results.sarif.json").exists(),
        "bundle should NOT contain SARIF when --include-sarif is not passed"
    );

    // Stdout should mention artifact count
    ensure!(
        stdout.contains("artifacts"),
        "bundle output should mention artifact count.\nstdout:\n{stdout}"
    );
    Ok(())
}

#[test]
fn bundle_include_sarif_adds_sarif() -> anyhow::Result<()> {
    let source_dir = setup_run_dir()?;
    let output_dir = TempDir::new()?;
    let bundle_dest = output_dir.path().join("bundle-sarif");

    let bin = cli_binary()?;
    ensure!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "bundle",
            "--source",
            &source_dir.path().display().to_string(),
            "--output",
            &bundle_dest.display().to_string(),
            "--include-sarif",
        ])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    ensure_eq!(
        output.status.code(),
        Some(0),
        "bundle --include-sarif should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // SARIF should be present
    ensure!(
        bundle_dest.join("results.sarif.json").exists(),
        "bundle should contain results.sarif.json when --include-sarif is passed"
    );

    // Core artifacts should still be present
    ensure!(
        bundle_dest.join("analysis.json").exists(),
        "bundle should still contain analysis.json"
    );
    ensure!(
        bundle_dest.join("index.html").exists(),
        "bundle should still contain index.html"
    );
    ensure!(
        bundle_dest.join("dossier.md").exists(),
        "bundle should still contain dossier.md"
    );
    Ok(())
}

#[test]
fn bundle_without_include_sarif_excludes_sarif() -> anyhow::Result<()> {
    let source_dir = setup_run_dir()?;
    let output_dir = TempDir::new()?;
    let bundle_dest = output_dir.path().join("bundle-no-sarif");

    let bin = cli_binary()?;
    ensure!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "bundle",
            "--source",
            &source_dir.path().display().to_string(),
            "--output",
            &bundle_dest.display().to_string(),
        ])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    ensure_eq!(
        output.status.code(),
        Some(0),
        "bundle should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // SARIF should NOT be present
    ensure!(
        !bundle_dest.join("results.sarif.json").exists(),
        "bundle should NOT contain results.sarif.json without --include-sarif"
    );
    Ok(())
}

#[test]
fn bundle_format_tar_gz_creates_archive() -> anyhow::Result<()> {
    let source_dir = setup_run_dir()?;
    let output_dir = TempDir::new()?;
    let archive_path = output_dir.path().join("bundle.tar.gz");

    let bin = cli_binary()?;
    ensure!(bin.exists(), "CLI binary not found at {}", bin.display());

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
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    ensure_eq!(
        output.status.code(),
        Some(0),
        "bundle --format tar-gz should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Archive file should exist and be non-empty
    ensure!(
        archive_path.exists(),
        "tar.gz archive should exist at {}",
        archive_path.display()
    );
    let metadata = require_ok!(std::fs::metadata(&archive_path), "read archive metadata");
    ensure!(
        metadata.len() > 0,
        "tar.gz archive should be non-empty (size: {})",
        metadata.len()
    );

    // Verify gzip magic bytes (1f 8b)
    let content = require_ok!(std::fs::read(&archive_path), "read archive content");
    ensure!(
        content.len() >= 2 && content[0] == 0x1f && content[1] == 0x8b,
        "archive should start with gzip magic bytes (1f 8b), got: {:02x} {:02x}",
        content.first().copied().unwrap_or(0),
        content.get(1).copied().unwrap_or(0)
    );
    Ok(())
}

#[test]
fn bundle_errors_on_empty_source() -> anyhow::Result<()> {
    let empty_dir = TempDir::new()?;
    let output_dir = TempDir::new()?;
    let bundle_dest = output_dir.path().join("bundle-fail");

    let bin = cli_binary()?;
    ensure!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "bundle",
            "--source",
            &empty_dir.path().display().to_string(),
            "--output",
            &bundle_dest.display().to_string(),
        ])
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should exit with code 2 (ExecutionError)
    ensure_eq!(
        output.status.code(),
        Some(2),
        "bundle on empty source should exit 2.\nstderr:\n{stderr}"
    );
    ensure!(
        stderr.contains("no loadable report"),
        "error should mention no loadable report.\nstderr:\n{stderr}"
    );
    Ok(())
}
