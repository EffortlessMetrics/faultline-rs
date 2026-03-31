//! Smoke test: builds a real Git repo via GitRepoBuilder,
//! runs the CLI binary via std::process::Command, and verifies artifacts.

use faultline_fixtures::{FileOp, GitRepoBuilder};
use std::process::Command;

/// Locate the faultline-cli binary built by cargo.
fn cli_binary() -> std::path::PathBuf {
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

#[test]
fn smoke_cli_produces_artifacts() {
    // Build a real Git repo with a clear pass→fail transition.
    // The predicate is a simple program that checks for a "PASS" file.
    // Commits 0 and 1 have the PASS file; commit 2 deletes it → fail.
    let repo = GitRepoBuilder::new()
        .expect("create fixture repo")
        .commit(
            "good commit",
            vec![FileOp::Write {
                path: "PASS".into(),
                content: "ok".into(),
            }],
        )
        .commit(
            "still good",
            vec![
                FileOp::Write {
                    path: "PASS".into(),
                    content: "still ok".into(),
                },
                FileOp::Write {
                    path: "other.txt".into(),
                    content: "noise".into(),
                },
            ],
        )
        .commit(
            "bad commit - removes PASS",
            vec![FileOp::Delete {
                path: "PASS".into(),
            }],
        )
        .build()
        .expect("build fixture repo");

    let good_sha = &repo.commits[0].0;
    let bad_sha = &repo.commits[2].0;
    let repo_path = repo.dir.path();
    let output_dir = repo_path.join("faultline-report");

    let bin = cli_binary();
    assert!(
        bin.exists(),
        "CLI binary not found at {}. Run `cargo build -p faultline-cli` first.",
        bin.display()
    );

    // Use --program mode with a cross-platform approach:
    // On Unix: test -f PASS (exit 0 if file exists, exit 1 if not)
    // On Windows: use cmd /C "if exist PASS (exit 0) else (exit 1)"
    let result = if cfg!(windows) {
        Command::new(&bin)
            .args([
                "--repo",
                &repo_path.display().to_string(),
                "--good",
                good_sha,
                "--bad",
                bad_sha,
                "--shell",
                "cmd",
                "--cmd",
                "if exist PASS (exit /b 0) else (exit /b 1)",
                "--kind",
                "test",
                "--timeout-seconds",
                "30",
                "--output-dir",
                &output_dir.display().to_string(),
            ])
            .output()
            .expect("failed to execute CLI binary")
    } else {
        Command::new(&bin)
            .args([
                "--repo",
                &repo_path.display().to_string(),
                "--good",
                good_sha,
                "--bad",
                bad_sha,
                "--cmd",
                "test -f PASS",
                "--kind",
                "test",
                "--timeout-seconds",
                "30",
                "--output-dir",
                &output_dir.display().to_string(),
            ])
            .output()
            .expect("failed to execute CLI binary")
    };

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    // Exit code 0 means FirstBad found (exact boundary).
    assert_eq!(
        result.status.code(),
        Some(0),
        "CLI should exit 0 (FirstBad).\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Verify analysis.json exists and contains schema_version.
    let analysis_path = output_dir.join("analysis.json");
    assert!(
        analysis_path.exists(),
        "analysis.json must exist at {}",
        analysis_path.display()
    );

    let analysis_content = std::fs::read_to_string(&analysis_path).expect("read analysis.json");
    let parsed: serde_json::Value =
        serde_json::from_str(&analysis_content).expect("analysis.json must be valid JSON");
    assert_eq!(
        parsed.get("schema_version").and_then(|v| v.as_str()),
        Some("0.1.0"),
        "analysis.json must contain schema_version 0.1.0"
    );

    // Verify index.html exists.
    let html_path = output_dir.join("index.html");
    assert!(
        html_path.exists(),
        "index.html must exist at {}",
        html_path.display()
    );
}
