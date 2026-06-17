//! End-to-end loading tests for all CLI and xtask paths.
//!
//! Validates Requirement 17: every export command works from both
//! `report.json`-only and `analysis.json`-only directories, and errors
//! correctly on empty directories.
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

/// Locate the xtask binary built by cargo.
fn xtask_binary() -> PathBuf {
    let mut path = std::env::current_exe()
        .expect("current_exe")
        .parent()
        .expect("parent of test binary")
        .parent()
        .expect("parent of deps dir")
        .to_path_buf();
    if cfg!(windows) {
        path.push("xtask.exe");
    } else {
        path.push("xtask");
    }
    path
}

/// Create a minimal valid AnalysisReport JSON string.
/// Includes a reproduction capsule so `reproduce` has something to work with.
fn minimal_report_json() -> String {
    serde_json::json!({
        "schema_version": "0.1.0",
        "run_id": "test-run-001",
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
                    "env": [],
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
            },
            {
                "commit": "bbbb2222",
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

/// Write a report to a temp directory as `report.json` only.
fn setup_report_json_only() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    std::fs::write(dir.path().join("report.json"), minimal_report_json())
        .expect("write report.json");
    dir
}

/// Write a report to a temp directory as `analysis.json` only.
fn setup_analysis_json_only() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    std::fs::write(dir.path().join("analysis.json"), minimal_report_json())
        .expect("write analysis.json");
    dir
}

/// Create an empty temp directory (no report files).
fn setup_empty_dir() -> TempDir {
    TempDir::new().expect("create temp dir")
}

// ============================================================================
// CLI `reproduce` tests — Requirement 17.1, 17.2
// ============================================================================

#[test]
fn cli_reproduce_loads_from_report_json_only_directory() {
    let dir = setup_report_json_only();
    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args(["reproduce", "--run-dir", &dir.path().display().to_string()])
        .output()
        .expect("failed to execute CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "reproduce from report.json-only dir should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    // Should contain capsule output (commit info)
    assert!(
        stdout.contains("commit") || stdout.contains("aaaa1111") || stdout.contains("bbbb2222"),
        "reproduce output should contain commit info.\nstdout:\n{stdout}"
    );
}

#[test]
fn cli_reproduce_loads_from_analysis_json_only_directory() {
    let dir = setup_analysis_json_only();
    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args(["reproduce", "--run-dir", &dir.path().display().to_string()])
        .output()
        .expect("failed to execute CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "reproduce from analysis.json-only dir should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("commit") || stdout.contains("aaaa1111") || stdout.contains("bbbb2222"),
        "reproduce output should contain commit info.\nstdout:\n{stdout}"
    );
}

// ============================================================================
// CLI `export-markdown` tests — Requirement 17.1, 17.2
// ============================================================================

#[test]
fn cli_export_markdown_loads_from_report_json_only_directory() {
    let dir = setup_report_json_only();
    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "export-markdown",
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
        "export-markdown from report.json-only dir should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    // Markdown output should contain some content
    assert!(
        !stdout.is_empty(),
        "export-markdown should produce non-empty output"
    );
}

#[test]
fn cli_export_markdown_loads_from_analysis_json_only_directory() {
    let dir = setup_analysis_json_only();
    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "export-markdown",
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
        "export-markdown from analysis.json-only dir should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.is_empty(),
        "export-markdown should produce non-empty output"
    );
}

// ============================================================================
// CLI `diff-runs` tests — Requirement 17.3
// ============================================================================

#[test]
fn cli_diff_runs_loads_from_report_json_file_path() {
    let dir = setup_report_json_only();
    let report_path = dir.path().join("report.json");
    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "diff-runs",
            "--left",
            &report_path.display().to_string(),
            "--right",
            &report_path.display().to_string(),
        ])
        .output()
        .expect("failed to execute CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "diff-runs from report.json file path should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    // Should contain comparison output
    assert!(
        stdout.contains("left-run") || stdout.contains("outcome"),
        "diff-runs output should contain comparison info.\nstdout:\n{stdout}"
    );
}

#[test]
fn cli_diff_runs_loads_from_analysis_json_file_path() {
    let dir = setup_analysis_json_only();
    let analysis_path = dir.path().join("analysis.json");
    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "diff-runs",
            "--left",
            &analysis_path.display().to_string(),
            "--right",
            &analysis_path.display().to_string(),
        ])
        .output()
        .expect("failed to execute CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "diff-runs from analysis.json file path should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("left-run") || stdout.contains("outcome"),
        "diff-runs output should contain comparison info.\nstdout:\n{stdout}"
    );
}

// ============================================================================
// Xtask export tests — Requirement 17.4, 17.5
// ============================================================================

#[test]
fn xtask_export_markdown_loads_from_report_json_only_directory() {
    let dir = setup_report_json_only();
    let bin = xtask_binary();
    if !bin.exists() {
        eprintln!(
            "xtask binary not found at {}; skipping xtask test",
            bin.display()
        );
        return;
    }

    let output = Command::new(&bin)
        .args([
            "export-markdown",
            "--run-dir",
            &dir.path().display().to_string(),
        ])
        .output()
        .expect("failed to execute xtask");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "xtask export-markdown from report.json-only dir should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.is_empty(),
        "xtask export-markdown should produce non-empty output"
    );
}

#[test]
fn xtask_export_markdown_loads_from_analysis_json_only_directory() {
    let dir = setup_analysis_json_only();
    let bin = xtask_binary();
    if !bin.exists() {
        eprintln!(
            "xtask binary not found at {}; skipping xtask test",
            bin.display()
        );
        return;
    }

    let output = Command::new(&bin)
        .args([
            "export-markdown",
            "--run-dir",
            &dir.path().display().to_string(),
        ])
        .output()
        .expect("failed to execute xtask");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "xtask export-markdown from analysis.json-only dir should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.is_empty(),
        "xtask export-markdown should produce non-empty output"
    );
}

#[test]
fn xtask_export_sarif_loads_from_report_json_only_directory() {
    let dir = setup_report_json_only();
    let bin = xtask_binary();
    if !bin.exists() {
        eprintln!(
            "xtask binary not found at {}; skipping xtask test",
            bin.display()
        );
        return;
    }

    let output = Command::new(&bin)
        .args([
            "export-sarif",
            "--run-dir",
            &dir.path().display().to_string(),
        ])
        .output()
        .expect("failed to execute xtask");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "xtask export-sarif from report.json-only dir should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.is_empty(),
        "xtask export-sarif should produce non-empty output"
    );
}

#[test]
fn xtask_export_sarif_loads_from_analysis_json_only_directory() {
    let dir = setup_analysis_json_only();
    let bin = xtask_binary();
    if !bin.exists() {
        eprintln!(
            "xtask binary not found at {}; skipping xtask test",
            bin.display()
        );
        return;
    }

    let output = Command::new(&bin)
        .args([
            "export-sarif",
            "--run-dir",
            &dir.path().display().to_string(),
        ])
        .output()
        .expect("failed to execute xtask");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "xtask export-sarif from analysis.json-only dir should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.is_empty(),
        "xtask export-sarif should produce non-empty output"
    );
}

#[test]
fn xtask_export_junit_loads_from_report_json_only_directory() {
    let dir = setup_report_json_only();
    let bin = xtask_binary();
    if !bin.exists() {
        eprintln!(
            "xtask binary not found at {}; skipping xtask test",
            bin.display()
        );
        return;
    }

    let output = Command::new(&bin)
        .args([
            "export-junit",
            "--run-dir",
            &dir.path().display().to_string(),
        ])
        .output()
        .expect("failed to execute xtask");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "xtask export-junit from report.json-only dir should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.is_empty(),
        "xtask export-junit should produce non-empty output"
    );
}

#[test]
fn xtask_export_junit_loads_from_analysis_json_only_directory() {
    let dir = setup_analysis_json_only();
    let bin = xtask_binary();
    if !bin.exists() {
        eprintln!(
            "xtask binary not found at {}; skipping xtask test",
            bin.display()
        );
        return;
    }

    let output = Command::new(&bin)
        .args([
            "export-junit",
            "--run-dir",
            &dir.path().display().to_string(),
        ])
        .output()
        .expect("failed to execute xtask");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "xtask export-junit from analysis.json-only dir should succeed.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.is_empty(),
        "xtask export-junit should produce non-empty output"
    );
}

// ============================================================================
// Error on empty directory tests — Requirement 17.6
// ============================================================================

#[test]
fn cli_reproduce_errors_on_empty_directory() {
    let dir = setup_empty_dir();
    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args(["reproduce", "--run-dir", &dir.path().display().to_string()])
        .output()
        .expect("failed to execute CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_ne!(
        output.status.code(),
        Some(0),
        "reproduce on empty dir should fail.\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("no report.json or analysis.json found"),
        "error message should mention missing files.\nstderr:\n{stderr}"
    );
}

#[test]
fn cli_export_markdown_errors_on_empty_directory() {
    let dir = setup_empty_dir();
    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "export-markdown",
            "--run-dir",
            &dir.path().display().to_string(),
        ])
        .output()
        .expect("failed to execute CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_ne!(
        output.status.code(),
        Some(0),
        "export-markdown on empty dir should fail.\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("no report.json or analysis.json found"),
        "error message should mention missing files.\nstderr:\n{stderr}"
    );
}

#[test]
fn cli_diff_runs_errors_on_nonexistent_file() {
    let dir = setup_empty_dir();
    let nonexistent = dir.path().join("nonexistent.json");
    let bin = cli_binary();
    assert!(bin.exists(), "CLI binary not found at {}", bin.display());

    let output = Command::new(&bin)
        .args([
            "diff-runs",
            "--left",
            &nonexistent.display().to_string(),
            "--right",
            &nonexistent.display().to_string(),
        ])
        .output()
        .expect("failed to execute CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_ne!(
        output.status.code(),
        Some(0),
        "diff-runs on nonexistent file should fail.\nstderr:\n{stderr}"
    );
    // Should mention the path doesn't exist or isn't accessible
    assert!(
        stderr.contains("does not exist")
            || stderr.contains("not accessible")
            || stderr.contains("path"),
        "error message should indicate file not found.\nstderr:\n{stderr}"
    );
}

#[test]
fn xtask_export_markdown_errors_on_empty_directory() {
    let dir = setup_empty_dir();
    let bin = xtask_binary();
    if !bin.exists() {
        eprintln!(
            "xtask binary not found at {}; skipping xtask test",
            bin.display()
        );
        return;
    }

    let output = Command::new(&bin)
        .args([
            "export-markdown",
            "--run-dir",
            &dir.path().display().to_string(),
        ])
        .output()
        .expect("failed to execute xtask");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_ne!(
        output.status.code(),
        Some(0),
        "xtask export-markdown on empty dir should fail.\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("no report.json or analysis.json found"),
        "error message should mention missing files.\nstderr:\n{stderr}"
    );
}

#[test]
fn xtask_export_sarif_errors_on_empty_directory() {
    let dir = setup_empty_dir();
    let bin = xtask_binary();
    if !bin.exists() {
        eprintln!(
            "xtask binary not found at {}; skipping xtask test",
            bin.display()
        );
        return;
    }

    let output = Command::new(&bin)
        .args([
            "export-sarif",
            "--run-dir",
            &dir.path().display().to_string(),
        ])
        .output()
        .expect("failed to execute xtask");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_ne!(
        output.status.code(),
        Some(0),
        "xtask export-sarif on empty dir should fail.\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("no report.json or analysis.json found"),
        "error message should mention missing files.\nstderr:\n{stderr}"
    );
}

#[test]
fn xtask_export_junit_errors_on_empty_directory() {
    let dir = setup_empty_dir();
    let bin = xtask_binary();
    if !bin.exists() {
        eprintln!(
            "xtask binary not found at {}; skipping xtask test",
            bin.display()
        );
        return;
    }

    let output = Command::new(&bin)
        .args([
            "export-junit",
            "--run-dir",
            &dir.path().display().to_string(),
        ])
        .output()
        .expect("failed to execute xtask");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_ne!(
        output.status.code(),
        Some(0),
        "xtask export-junit on empty dir should fail.\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("no report.json or analysis.json found"),
        "error message should mention missing files.\nstderr:\n{stderr}"
    );
}
