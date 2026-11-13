from pathlib import Path

p = Path('crates/faultline-cli/src/main.rs')
s = p.read_text()
old = 'use faultline_app::FaultlineApp;'
assert s.count(old) == 1
s = s.replace(old, 'use faultline_app::{FaultlineApp, LocalizeOptions};')
old = 'let localized = match app.localize(request) {'
assert s.count(old) == 1
s = s.replace(old, '''let localized = match app.localize_with_options(request, LocalizeOptions {
        force: cli.force, fresh: cli.fresh, no_render: cli.no_render,
    }) {''')
p.write_text(s)
p = Path('crates/faultline-cli/tests/smoke.rs')
s = p.read_text()
assert s.count('Some("0.1.0")') == 1
p.write_text(s.replace('Some("0.1.0")', 'Some("0.4.0")').replace('schema_version 0.1.0', 'schema_version 0.4.0'))

p = Path('crates/faultline-cli/tests/attempt_modes.rs')
p.write_text(r'''use faultline_fixtures::{FileOp, GitRepoBuilder};
use faultline_types::AnalysisReport;
use std::path::Path;
use std::process::Command;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn run(repo: &Path, good: &str, bad: &str, mode: Option<&str>) -> Result<AnalysisReport, Box<dyn std::error::Error>> {
    let output = repo.join("report");
    let mut command = Command::new(env!("CARGO_BIN_EXE_faultline-rs"));
    command.arg("--repo").arg(repo).args(["--good", good, "--bad", bad, "--program", "git", "--arg", "ls-files", "--arg", "--error-unmatch", "--arg", "PASS", "--kind", "test", "--retries", "2", "--timeout-seconds", "30", "--output-dir"]).arg(&output);
    if let Some(mode) = mode { command.arg(mode); }
    let result = command.output()?;
    assert!(result.status.success(), "actual CLI failed: {result:?}");
    Ok(serde_json::from_slice(&std::fs::read(output.join("analysis.json"))?)?)
}

#[test]
fn real_cli_preserves_retries_and_honors_resume_force_and_fresh() -> TestResult {
    let fixture = GitRepoBuilder::new()?
        .commit("base", vec![FileOp::Write { path: "PASS".into(), content: "one".into() }])
        .commit("passing prefix", vec![FileOp::Write { path: "PASS".into(), content: "two".into() }])
        .commit("regression", vec![FileOp::Delete { path: "PASS".into() }])
        .build()?;
    let good = &fixture.commits.first().ok_or("base")?.0;
    let bad = &fixture.commits.last().ok_or("head")?.0;
    let path = fixture.dir.path();
    let first = run(path, good, bad, None)?;
    assert_eq!(first.schema_version, "0.4.0");
    assert_eq!(first.probe_attempts.len(), 9);
    assert!(first.probe_attempts.iter().all(|a| a.flake_signal.is_none() && a.exit_code.is_some()));
    assert!(first.observations.iter().all(|a| a.flake_signal.is_some() && a.exit_code.is_none()));
    let resumed = run(path, good, bad, Some("--resume"))?;
    assert_eq!(resumed.probe_attempts, first.probe_attempts);
    let forced = run(path, good, bad, Some("--force"))?;
    assert_eq!(forced.probe_attempts.len(), 18, "force must reach the application instead of returning the cached report");
    assert!(forced.probe_attempts.starts_with(&first.probe_attempts));
    assert!(forced.probe_aggregates.iter().flat_map(|a| &a.attempt_ids).all(|id| *id >= 9));
    let fresh = run(path, good, bad, Some("--fresh"))?;
    assert_eq!(fresh.probe_attempts.len(), 9, "explicit fresh must create a new run journal");
    assert_eq!(fresh.probe_attempts.first().ok_or("fresh attempt")?.sequence_index, 0);
    assert_eq!(fresh.outcome, first.outcome);
    Ok(())
}
''')
p = Path('docs/scenarios/scenario_index.md')
p.write_text(p.read_text() + '\n| `real_cli_preserves_retries_and_honors_resume_force_and_fresh` | CLI run modes reach the execution application | Real Git repository and git predicate | faultline-rs | JSON and journal | Reuse, non-destructive force and explicit fresh | #55 #62 | cli | retry-evidence | v0.4 | faultline-rs | P0 | faultline-rs | no |\n')
p = Path('docs/probe-attempt-evidence.md')
p.write_text(p.read_text() + '\nThe CLI now passes `force`, `fresh` and `no_render` into `LocalizeOptions`;\npreviously those parsed run-mode flags never reached the application. A real\nGit/CLI regression exercises three attempts per state, no-execution resume,\nforce retaining previous attempts, and explicit fresh creating a new journal.\nThis repairs flag wiring, not the broader filesystem ownership work in #61.\n')
