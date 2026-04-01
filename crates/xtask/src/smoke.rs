use anyhow::{Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Run a git command in the given directory, returning stdout on success.
fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run git {}: {e}", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git {} failed (exit {:?}): {}",
            args.join(" "),
            output.status.code(),
            stderr.trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Create a temporary Git repo with a known pass→fail regression.
///
/// Commit 0: writes a `PASS` file (predicate succeeds)
/// Commit 1: writes another file, keeps `PASS` (predicate succeeds)
/// Commit 2: deletes `PASS` (predicate fails)
///
/// Returns (temp_dir, good_sha, bad_sha).
fn build_fixture_repo() -> Result<(TempDir, String, String)> {
    let dir = TempDir::new()?;
    let repo = dir.path();

    git(repo, &["init", "--initial-branch", "main"])?;
    git(repo, &["config", "user.email", "smoke@test.local"])?;
    git(repo, &["config", "user.name", "Smoke"])?;

    // Commit 0: good — PASS file exists
    std::fs::write(repo.join("PASS"), "ok")?;
    git(repo, &["add", "."])?;
    git(repo, &["commit", "-m", "good commit"])?;
    let good_sha = git(repo, &["rev-parse", "HEAD"])?;

    // Commit 1: still good — PASS file still exists
    std::fs::write(repo.join("other.txt"), "noise")?;
    git(repo, &["add", "."])?;
    git(repo, &["commit", "-m", "still good"])?;

    // Commit 2: bad — PASS file deleted
    std::fs::remove_file(repo.join("PASS"))?;
    git(repo, &["add", "."])?;
    git(repo, &["commit", "-m", "bad commit - removes PASS"])?;
    let bad_sha = git(repo, &["rev-parse", "HEAD"])?;

    Ok((dir, good_sha, bad_sha))
}

/// Locate the faultline-cli binary from a cargo build.
fn find_cli_binary() -> Result<PathBuf> {
    // Build the CLI first
    println!("=> cargo build -p faultline-cli");
    let status = Command::new("cargo")
        .args(["build", "-p", "faultline-cli"])
        .status()
        .map_err(|e| anyhow::anyhow!("failed to build faultline-cli: {e}"))?;
    if !status.success() {
        bail!("contract broken: CLI build failed");
    }

    // Determine the binary path from cargo metadata
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|e| anyhow::anyhow!("cargo metadata failed: {e}"))?;

    let meta: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| anyhow::anyhow!("failed to parse cargo metadata: {e}"))?;

    let target_dir = meta["target_directory"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no target_directory in cargo metadata"))?;

    let bin_name = if cfg!(windows) {
        "faultline-cli.exe"
    } else {
        "faultline-cli"
    };

    let bin = PathBuf::from(target_dir).join("debug").join(bin_name);
    if !bin.exists() {
        bail!("CLI binary not found at {} after build", bin.display());
    }

    Ok(bin)
}

/// Run the real smoke test: build CLI, create fixture repo, run CLI, verify artifacts.
pub fn run_smoke() -> Result<()> {
    println!("=== smoke ===\n");

    // Step 1: Build the CLI binary
    let bin = find_cli_binary()?;
    println!("  CLI binary: {}", bin.display());

    // Step 2: Create fixture repo with known regression
    println!("=> creating fixture repo");
    let (dir, good_sha, bad_sha) = build_fixture_repo()?;
    let repo_path = dir.path();
    let output_dir = repo_path.join("faultline-report");
    println!("  fixture repo: {}", repo_path.display());
    println!("  good: {good_sha}");
    println!("  bad:  {bad_sha}");

    // Step 3: Run the CLI against the fixture repo
    println!("=> running faultline CLI");
    let cmd_args = if cfg!(windows) {
        vec![
            "--repo",
            repo_path.to_str().unwrap(),
            "--good",
            &good_sha,
            "--bad",
            &bad_sha,
            "--shell",
            "cmd",
            "--cmd",
            "if exist PASS (exit /b 0) else (exit /b 1)",
            "--kind",
            "test",
            "--timeout-seconds",
            "30",
            "--output-dir",
            output_dir.to_str().unwrap(),
        ]
    } else {
        vec![
            "--repo",
            repo_path.to_str().unwrap(),
            "--good",
            &good_sha,
            "--bad",
            &bad_sha,
            "--cmd",
            "test -f PASS",
            "--kind",
            "test",
            "--timeout-seconds",
            "30",
            "--output-dir",
            output_dir.to_str().unwrap(),
        ]
    };

    let result = Command::new(&bin)
        .args(&cmd_args)
        .output()
        .map_err(|e| anyhow::anyhow!("failed to execute CLI: {e}"))?;

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    // Step 4: Verify exit code 0 (FirstBad found)
    let exit_code = result.status.code().unwrap_or(-1);
    if exit_code != 0 {
        bail!(
            "contract broken: smoke — CLI exited with code {exit_code} (expected 0 = FirstBad)\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }
    println!("  exit code: 0 (FirstBad)");

    // Step 5: Verify artifacts exist
    let analysis_path = output_dir.join("analysis.json");
    if !analysis_path.exists() {
        bail!(
            "contract broken: smoke — analysis.json not found at {}",
            analysis_path.display()
        );
    }
    println!("  artifact: {}", analysis_path.display());

    let html_path = output_dir.join("index.html");
    if !html_path.exists() {
        bail!(
            "contract broken: smoke — index.html not found at {}",
            html_path.display()
        );
    }
    println!("  artifact: {}", html_path.display());

    // Bonus: verify analysis.json is valid JSON with schema_version
    let content = std::fs::read_to_string(&analysis_path)?;
    let parsed: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        anyhow::anyhow!("contract broken: smoke — analysis.json is not valid JSON: {e}")
    })?;
    if parsed
        .get("schema_version")
        .and_then(|v| v.as_str())
        .is_none()
    {
        bail!("contract broken: smoke — analysis.json missing schema_version field");
    }

    println!("\n=== smoke passed ===");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_repo_has_three_commits() {
        let (dir, good, bad) = build_fixture_repo().expect("build fixture repo");
        // good and bad should be 40-char hex SHAs
        assert_eq!(good.len(), 40, "good SHA should be 40 chars: {good}");
        assert_eq!(bad.len(), 40, "bad SHA should be 40 chars: {bad}");
        assert_ne!(good, bad, "good and bad SHAs must differ");

        // PASS file should NOT exist (deleted in last commit)
        assert!(!dir.path().join("PASS").exists());
        // other.txt should exist
        assert!(dir.path().join("other.txt").exists());
    }
}
