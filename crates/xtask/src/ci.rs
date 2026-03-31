use anyhow::{Result, bail};
use std::process::Command;

use crate::tools::has_tool;

/// Build a contract-broken error message.
/// Separated for testability (Property 46).
pub fn contract_broken_message(contract: &str) -> String {
    format!("contract broken: {contract}")
}

/// Build a golden artifact failure message with remediation instructions.
pub fn golden_failure_message(artifact: &str) -> String {
    format!(
        "contract broken: golden artifact — {artifact}\n  run: cargo insta review\n  see: TESTING.md#golden-tests"
    )
}

/// Build a schema drift failure message with remediation instructions.
pub fn schema_drift_message() -> String {
    "contract broken: schema drift\n  run: cargo xtask generate-schema\n  see: TESTING.md#schema-checks".to_string()
}

/// Build a missing scenario failure message with remediation instructions.
pub fn missing_scenario_message(files: &str) -> String {
    format!(
        "contract broken: scenario atlas\n  missing entries for: {files}\n  see: TESTING.md#scenario-atlas"
    )
}

/// Run a command, returning an error with a contract-aware message on failure.
fn run_contract(contract: &str, cmd: &str, args: &[&str]) -> Result<()> {
    println!("=> {cmd} {}", args.join(" "));
    let status = Command::new(cmd)
        .args(args)
        .status()
        .map_err(|e| anyhow::anyhow!("failed to execute {cmd}: {e}"))?;

    if !status.success() {
        bail!("{}", contract_broken_message(contract));
    }
    Ok(())
}

/// ci-fast: fmt + clippy + test (with nextest fallback)
pub fn ci_fast() -> Result<()> {
    println!("=== ci-fast ===\n");

    run_contract("code formatting", "cargo", &["fmt", "--check"])?;

    run_contract(
        "lint warnings",
        "cargo",
        &["clippy", "--workspace", "--", "-D", "warnings"],
    )?;

    // Detect cargo-nextest and use it if available, fall back to cargo test
    if has_tool("cargo-nextest") {
        run_contract("test suite", "cargo", &["nextest", "run", "--workspace"])?;
    } else {
        run_contract("test suite", "cargo", &["test", "--workspace"])?;
    }

    println!("\n=== ci-fast passed ===");
    Ok(())
}

/// ci-full: ci-fast + golden test check + schema check
pub fn ci_full() -> Result<()> {
    println!("=== ci-full ===\n");

    // Run all ci-fast steps first
    ci_fast()?;

    // Golden test check via cargo insta test
    println!("\n=> cargo insta test");
    let status = Command::new("cargo")
        .args(["insta", "test"])
        .status()
        .map_err(|e| anyhow::anyhow!("failed to execute cargo insta test: {e}"))?;

    if !status.success() {
        eprintln!("{}", golden_failure_message("snapshot test changed"));
        bail!("contract broken: golden artifact — run `cargo insta review` to inspect changes");
    }

    // Schema drift check
    println!("\n=> schema check");
    if let Err(e) = crate::schema::check_schema() {
        let msg = format!("{e}");
        if msg.contains("schema drift detected") {
            eprintln!("{}", schema_drift_message());
        }
        bail!(e);
    }

    // Scenario atlas check
    println!("\n=> check-scenarios");
    check_scenarios()?;

    println!("\n=== ci-full passed ===");
    Ok(())
}

/// Verify the scenario atlas is consistent with workspace test functions.
pub fn check_scenarios() -> Result<()> {
    let root = crate::scaffold::workspace_root()?;
    let workspace_tests = crate::scenarios::scan_workspace_tests(&root);
    let index_entries = crate::scenarios::read_scenario_index(&root);
    let result = crate::scenarios::check_consistency(&workspace_tests, &index_entries);

    if !result.missing_from_index.is_empty() {
        let files = result
            .missing_from_index
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("{}", missing_scenario_message(&files));
    }

    if !result.stale_in_index.is_empty() {
        let stale = result
            .stale_in_index
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!(
            "contract broken: scenario atlas\n  stale entries for: {stale}\n  see: TESTING.md#scenario-atlas"
        );
    }

    if !result.is_ok() {
        bail!("contract broken: scenario atlas");
    }

    Ok(())
}
