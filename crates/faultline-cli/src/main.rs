use clap::{Parser, Subcommand};
use faultline_app::FaultlineApp;
use faultline_codes::{OperatorCode, ProbeKind};
use faultline_git::GitAdapter;
use faultline_probe_exec::ExecProbeAdapter;
use faultline_render::ReportRenderer;
use faultline_store::FileRunStore;
use faultline_types::{
    AnalysisReport, AnalysisRequest, FaultlineError, FlakePolicy, HistoryMode, LocalizationOutcome,
    ProbeSpec, RevisionSpec, RunComparison, SearchPolicy, ShellKind,
};
use std::io;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "faultline")]
#[command(version)]
#[command(about = "Local-first regression localization for Git repos")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(long, default_value = ".")]
    repo: PathBuf,

    #[arg(long)]
    good: Option<String>,

    #[arg(long)]
    bad: Option<String>,

    #[arg(long, default_value_t = false)]
    first_parent: bool,

    #[arg(long)]
    cmd: Option<String>,

    #[arg(long)]
    program: Option<String>,

    #[arg(long = "arg", allow_hyphen_values = true)]
    args: Vec<String>,

    #[arg(long, default_value = "custom")]
    kind: String,

    #[arg(long, default_value_t = 300)]
    timeout_seconds: u64,

    #[arg(long, default_value = "faultline-report")]
    output_dir: PathBuf,

    #[arg(long, default_value_t = 64)]
    max_probes: usize,

    /// Explicit resume from cached observations (default behavior)
    #[arg(long, default_value_t = false)]
    resume: bool,

    /// Discard cached observations and re-probe
    #[arg(long, default_value_t = false)]
    force: bool,

    /// Delete entire run directory and start from scratch
    #[arg(long, default_value_t = false)]
    fresh: bool,

    /// Skip HTML report generation
    #[arg(long, default_value_t = false)]
    no_render: bool,

    /// Shell to use for --cmd predicates (sh, cmd, powershell)
    #[arg(long)]
    shell: Option<String>,

    /// Environment variable injection (KEY=VALUE), repeatable
    #[arg(long = "env")]
    envs: Vec<String>,

    /// Number of probe retries for flake detection (0 = no retries)
    #[arg(long, default_value_t = 0)]
    retries: u32,

    /// Minimum proportion of consistent results to classify as stable (0.0–1.0)
    #[arg(long, default_value_t = 1.0)]
    stability_threshold: f64,

    /// Also write a Markdown dossier alongside HTML/JSON
    #[arg(long, default_value_t = false)]
    markdown: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Extract a reproduction capsule from a completed run
    Reproduce {
        /// Path to the run directory containing report.json
        #[arg(long)]
        run_dir: PathBuf,

        /// Target commit SHA (defaults to boundary commits)
        #[arg(long)]
        commit: Option<String>,

        /// Emit shell script to stdout instead of summary
        #[arg(long, default_value_t = false)]
        shell: bool,
    },
    /// Compare two analysis runs side-by-side
    DiffRuns {
        /// Path to the left (baseline) report JSON file
        #[arg(long)]
        left: PathBuf,

        /// Path to the right (new) report JSON file
        #[arg(long)]
        right: PathBuf,

        /// Emit JSON output instead of human-readable summary
        #[arg(long, default_value_t = false, conflicts_with = "markdown_diff")]
        json: bool,

        /// Emit Markdown output suitable for PR comments
        #[arg(
            long = "markdown",
            id = "markdown_diff",
            default_value_t = false,
            conflicts_with = "json"
        )]
        markdown: bool,
    },
    /// Export a Markdown dossier from a completed run
    ExportMarkdown {
        /// Path to the run directory containing report.json
        #[arg(long)]
        run_dir: PathBuf,
    },
}

fn main() {
    match try_main() {
        Ok(code) => {
            std::process::exit(code);
        }
        Err(err) => {
            // InvalidBoundary errors already have detailed output from try_main;
            // other errors get a generic prefix
            let exit_code = if err
                .downcast_ref::<FaultlineError>()
                .is_some_and(|e| matches!(e, FaultlineError::InvalidBoundary(_)))
            {
                exit_code_for_operator_code(OperatorCode::InvalidInput)
            } else {
                eprintln!("faultline: {err}");
                exit_code_for_operator_code(OperatorCode::ExecutionError)
            };
            std::process::exit(exit_code);
        }
    }
}

fn outcome_to_operator_code(outcome: &LocalizationOutcome) -> OperatorCode {
    match outcome {
        LocalizationOutcome::FirstBad { .. } => OperatorCode::Success,
        LocalizationOutcome::SuspectWindow { .. } => OperatorCode::SuspectWindow,
        LocalizationOutcome::Inconclusive { .. } => OperatorCode::Inconclusive,
    }
}

fn exit_code_for_operator_code(code: OperatorCode) -> i32 {
    match code {
        OperatorCode::Success => 0,
        OperatorCode::SuspectWindow => 1,
        OperatorCode::ExecutionError => 2,
        OperatorCode::Inconclusive => 3,
        OperatorCode::InvalidInput => 4,
    }
}

fn validate_cmd_program(cmd: &Option<String>, program: &Option<String>) -> Result<(), io::Error> {
    if cmd.is_some() && program.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "only one of --cmd or --program is allowed",
        ));
    }
    if cmd.is_none() && program.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "one of --cmd or --program is required",
        ));
    }
    Ok(())
}

fn validate_run_mode(resume: bool, force: bool, fresh: bool) -> Result<(), io::Error> {
    if resume && force {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--resume and --force are mutually exclusive",
        ));
    }
    if resume && fresh {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--resume and --fresh are mutually exclusive",
        ));
    }
    if force && fresh {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--force and --fresh are mutually exclusive",
        ));
    }
    Ok(())
}

fn validate_env_vars(envs: &[String]) -> Result<(), io::Error> {
    for entry in envs {
        if !entry.contains('=') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid --env value '{}': expected KEY=VALUE format", entry),
            ));
        }
    }
    Ok(())
}

fn validate_shell(shell: &Option<String>) -> Result<(), io::Error> {
    if let Some(s) = shell {
        match s.as_str() {
            "sh" | "cmd" | "powershell" => Ok(()),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "invalid --shell '{}': expected one of sh, cmd, powershell",
                    s
                ),
            )),
        }
    } else {
        Ok(())
    }
}

fn validate_stability_threshold(value: f64) -> std::result::Result<(), FaultlineError> {
    if !(0.0..=1.0).contains(&value) {
        return Err(FaultlineError::InvalidInput(format!(
            "invalid --stability-threshold '{}': must be between 0.0 and 1.0",
            value
        )));
    }
    Ok(())
}

fn try_main() -> Result<i32, Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Dispatch subcommands
    if let Some(command) = cli.command {
        return match command {
            Commands::Reproduce {
                run_dir,
                commit,
                shell,
            } => run_reproduce(run_dir, commit, shell),
            Commands::DiffRuns {
                left,
                right,
                json,
                markdown,
            } => run_diff_runs(left, right, json, markdown),
            Commands::ExportMarkdown { run_dir } => run_export_markdown(run_dir),
        };
    }

    // Default: localize flow (requires --good and --bad)
    let good = cli.good.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "--good is required for localization",
        )
    })?;
    let bad = cli.bad.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "--bad is required for localization",
        )
    })?;

    validate_cmd_program(&cli.cmd, &cli.program)?;
    validate_run_mode(cli.resume, cli.force, cli.fresh)?;
    validate_env_vars(&cli.envs)?;
    validate_shell(&cli.shell)?;
    validate_stability_threshold(cli.stability_threshold)?;

    let probe_kind = cli.kind.parse::<ProbeKind>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "invalid --kind '{}': expected one of build, test, lint, perf-threshold, custom",
                cli.kind
            ),
        )
    })?;

    let env_pairs: Vec<(String, String)> = cli
        .envs
        .iter()
        .map(|entry| {
            let (key, value) = entry.split_once('=').expect("validated above");
            (key.to_string(), value.to_string())
        })
        .collect();

    let shell_kind = match cli.shell.as_deref() {
        Some("sh") => ShellKind::PosixSh,
        Some("cmd") => ShellKind::Cmd,
        Some("powershell") => ShellKind::PowerShell,
        None => ShellKind::Default,
        _ => unreachable!("validated above"),
    };

    let probe = match (cli.cmd, cli.program) {
        (Some(script), None) => ProbeSpec::Shell {
            kind: probe_kind,
            shell: shell_kind,
            script,
            env: env_pairs,
            timeout_seconds: cli.timeout_seconds,
        },
        (None, Some(program)) => ProbeSpec::Exec {
            kind: probe_kind,
            program,
            args: cli.args,
            env: env_pairs,
            timeout_seconds: cli.timeout_seconds,
        },
        _ => unreachable!("validated above"),
    };

    let request = AnalysisRequest {
        repo_root: cli.repo.clone(),
        good: RevisionSpec(good),
        bad: RevisionSpec(bad),
        history_mode: if cli.first_parent {
            HistoryMode::FirstParent
        } else {
            HistoryMode::AncestryPath
        },
        probe,
        policy: SearchPolicy {
            max_probes: cli.max_probes,
            flake_policy: FlakePolicy {
                retries: cli.retries,
                stability_threshold: cli.stability_threshold,
            },
        },
    };

    let history_mode_label = if cli.first_parent {
        "first-parent"
    } else {
        "ancestry-path"
    };

    let git = GitAdapter::new(&cli.repo)?;
    let store = FileRunStore::new(cli.repo.join(".faultline").join("runs"))?;
    let probe = ExecProbeAdapter;
    let app = FaultlineApp::new(&git, &git, &probe, &store);
    let localized = match app.localize(request) {
        Ok(result) => result,
        Err(err) => {
            if let FaultlineError::InvalidBoundary(msg) = &err {
                eprintln!("faultline: boundary validation failed");
                eprintln!("  {}", msg);
            }
            return Err(err.into());
        }
    };

    let renderer = ReportRenderer::new(&cli.output_dir);
    let rendered_html = if cli.no_render {
        renderer.render_json_only(&localized.report)?;
        false
    } else if cli.markdown {
        renderer.render_with_markdown(&localized.report)?;
        true
    } else {
        renderer.render(&localized.report)?;
        true
    };

    let analysis_path = renderer.output_dir().join("analysis.json");
    let html_path = renderer.output_dir().join("index.html");
    let md_path = renderer.output_dir().join("dossier.md");

    println!("run-id       {}", localized.report.run_id);
    println!("observations {}", localized.report.observations.len());
    println!("output-dir   {}", renderer.output_dir().display());
    println!("artifacts    {}", analysis_path.display());
    if rendered_html {
        println!("             {}", html_path.display());
    }
    if cli.markdown {
        println!("             {}", md_path.display());
    }
    println!("history      {}", history_mode_label);
    println!("outcome      {}", format_outcome(&localized.report.outcome));
    let code = exit_code_for_operator_code(outcome_to_operator_code(&localized.report.outcome));
    Ok(code)
}

/// Load a report from a run directory.
fn load_report_from_dir(
    run_dir: &std::path::Path,
) -> Result<AnalysisReport, Box<dyn std::error::Error>> {
    let report_path = run_dir.join("report.json");
    if !report_path.exists() {
        return Err(FaultlineError::Store(format!(
            "report.json not found in {}",
            run_dir.display()
        ))
        .into());
    }
    let raw = std::fs::read_to_string(&report_path)?;
    let report: AnalysisReport = serde_json::from_str(&raw)?;
    Ok(report)
}

/// Run the `reproduce` subcommand.
fn run_reproduce(
    run_dir: PathBuf,
    commit: Option<String>,
    shell: bool,
) -> Result<i32, Box<dyn std::error::Error>> {
    let report = load_report_from_dir(&run_dir)?;

    if report.reproduction_capsules.is_empty() {
        eprintln!("faultline: no reproduction capsules in report");
        return Ok(exit_code_for_operator_code(OperatorCode::ExecutionError));
    }

    let capsules: Vec<&faultline_types::ReproductionCapsule> = if let Some(ref sha) = commit {
        // Find capsule(s) matching the given commit
        report
            .reproduction_capsules
            .iter()
            .filter(|c| c.commit.0 == *sha)
            .collect()
    } else {
        // Default: boundary commits
        match report.outcome.boundary_pair() {
            Some((good, bad)) => report
                .reproduction_capsules
                .iter()
                .filter(|c| c.commit == *good || c.commit == *bad)
                .collect(),
            None => {
                // Inconclusive — emit all capsules
                report.reproduction_capsules.iter().collect()
            }
        }
    };

    if capsules.is_empty() {
        let msg = if let Some(sha) = commit {
            format!("no capsule found for commit {}", sha)
        } else {
            "no capsule found for boundary commits".to_string()
        };
        eprintln!("faultline: {}", msg);
        return Ok(exit_code_for_operator_code(OperatorCode::ExecutionError));
    }

    for capsule in &capsules {
        if shell {
            print!("{}", capsule.to_shell_script());
        } else {
            println!("commit    {}", capsule.commit);
            println!("timeout   {}s", capsule.timeout_seconds);
            println!("work-dir  {}", capsule.working_dir);
            match &capsule.predicate {
                ProbeSpec::Exec { program, args, .. } => {
                    let cmd_str = if args.is_empty() {
                        program.clone()
                    } else {
                        format!("{} {}", program, args.join(" "))
                    };
                    println!("predicate {}", cmd_str);
                }
                ProbeSpec::Shell { script, .. } => {
                    println!("predicate sh -c '{}'", script);
                }
            }
            if !capsule.env.is_empty() {
                for (key, value) in &capsule.env {
                    println!("env       {}={}", key, value);
                }
            }
            println!();
        }
    }

    Ok(0)
}

/// Load a report from a JSON file path.
fn load_report_from_file(
    path: &std::path::Path,
) -> Result<AnalysisReport, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Err(
            FaultlineError::Store(format!("report file not found: {}", path.display())).into(),
        );
    }
    let raw = std::fs::read_to_string(path)?;
    let report: AnalysisReport = serde_json::from_str(&raw)?;
    Ok(report)
}

/// Run the `diff-runs` subcommand.
fn run_diff_runs(
    left_path: PathBuf,
    right_path: PathBuf,
    json: bool,
    markdown: bool,
) -> Result<i32, Box<dyn std::error::Error>> {
    let left = load_report_from_file(&left_path)?;
    let right = load_report_from_file(&right_path)?;
    let cmp = faultline_types::compare_runs(&left, &right);

    if json {
        let output = serde_json::to_string_pretty(&cmp)?;
        println!("{}", output);
    } else if markdown {
        let output = faultline_render::render_run_comparison_markdown(&cmp);
        print!("{}", output);
    } else {
        print_run_comparison(&cmp);
    }

    Ok(0)
}

/// Run the `export-markdown` subcommand.
fn run_export_markdown(run_dir: PathBuf) -> Result<i32, Box<dyn std::error::Error>> {
    let report = load_report_from_dir(&run_dir)?;
    let md = faultline_render::render_markdown(&report);
    print!("{}", md);
    Ok(0)
}

fn print_run_comparison(cmp: &RunComparison) {
    println!("left-run     {}", cmp.left_run_id);
    println!("right-run    {}", cmp.right_run_id);
    println!(
        "outcome      {}",
        if cmp.outcome_changed {
            "CHANGED"
        } else {
            "unchanged"
        }
    );
    println!("confidence   {:+}", cmp.confidence_delta);
    println!("window       {:+}", cmp.window_width_delta);
    println!("probes-reused {}", cmp.probes_reused);
    if !cmp.suspect_paths_added.is_empty() {
        println!("paths-added  {}", cmp.suspect_paths_added.join(", "));
    }
    if !cmp.suspect_paths_removed.is_empty() {
        println!("paths-removed {}", cmp.suspect_paths_removed.join(", "));
    }
    if !cmp.ambiguity_reasons_added.is_empty() {
        let reasons: Vec<String> = cmp
            .ambiguity_reasons_added
            .iter()
            .map(|r| r.to_string())
            .collect();
        println!("reasons-added {}", reasons.join(", "));
    }
    if !cmp.ambiguity_reasons_removed.is_empty() {
        let reasons: Vec<String> = cmp
            .ambiguity_reasons_removed
            .iter()
            .map(|r| r.to_string())
            .collect();
        println!("reasons-removed {}", reasons.join(", "));
    }
}

fn format_outcome(outcome: &LocalizationOutcome) -> String {
    match outcome {
        LocalizationOutcome::FirstBad {
            last_good,
            first_bad,
            confidence,
        } => {
            format!(
                "FirstBad  last_good={} first_bad={} confidence={}({})",
                last_good, first_bad, confidence.score, confidence.label
            )
        }
        LocalizationOutcome::SuspectWindow {
            lower_bound_exclusive,
            upper_bound_inclusive,
            confidence,
            reasons,
        } => {
            let reasons_str: Vec<String> = reasons.iter().map(|r| r.to_string()).collect();
            format!(
                "SuspectWindow  lower={} upper={} confidence={}({}) reasons=[{}]",
                lower_bound_exclusive,
                upper_bound_inclusive,
                confidence.score,
                confidence.label,
                reasons_str.join(", ")
            )
        }
        LocalizationOutcome::Inconclusive { reasons } => {
            let reasons_str: Vec<String> = reasons.iter().map(|r| r.to_string()).collect();
            format!("Inconclusive  reasons=[{}]", reasons_str.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use faultline_codes::AmbiguityReason;
    use faultline_types::Confidence;
    use proptest::prelude::*;

    #[test]
    fn rejects_both_cmd_and_program() {
        let err = validate_cmd_program(&Some("echo ok".into()), &Some("./test".into()))
            .expect_err("should reject both --cmd and --program");
        assert!(
            err.to_string().contains("only one of --cmd or --program"),
            "unexpected error message: {}",
            err
        );
    }

    #[test]
    fn rejects_neither_cmd_nor_program() {
        let err = validate_cmd_program(&None, &None)
            .expect_err("should reject neither --cmd nor --program");
        assert!(
            err.to_string()
                .contains("one of --cmd or --program is required"),
            "unexpected error message: {}",
            err
        );
    }

    #[test]
    fn help_output_describes_all_flags() {
        let mut cmd = Cli::command();
        let mut buf = Vec::new();
        cmd.write_long_help(&mut buf).unwrap();
        let help = String::from_utf8(buf).unwrap();

        let expected_flags = [
            "--good",
            "--bad",
            "--repo",
            "--cmd",
            "--program",
            "--arg",
            "--kind",
            "--first-parent",
            "--timeout-seconds",
            "--output-dir",
            "--max-probes",
            "--resume",
            "--force",
            "--fresh",
            "--no-render",
            "--shell",
            "--env",
            "--retries",
            "--stability-threshold",
            "--markdown",
        ];

        for flag in &expected_flags {
            assert!(
                help.contains(flag),
                "--help output missing expected flag '{flag}'.\nFull help:\n{help}"
            );
        }
    }

    #[test]
    fn help_output_describes_reproduce_subcommand() {
        let mut cmd = Cli::command();
        let mut buf = Vec::new();
        cmd.write_long_help(&mut buf).unwrap();
        let help = String::from_utf8(buf).unwrap();

        assert!(
            help.contains("reproduce"),
            "--help output missing 'reproduce' subcommand.\nFull help:\n{help}"
        );
        assert!(
            help.contains("diff-runs"),
            "--help output missing 'diff-runs' subcommand.\nFull help:\n{help}"
        );
        assert!(
            help.contains("export-markdown"),
            "--help output missing 'export-markdown' subcommand.\nFull help:\n{help}"
        );
    }

    // Req 3.4: Golden snapshot test for CLI --help text
    #[test]
    fn golden_cli_help() {
        let mut cmd = Cli::command();
        let mut buf = Vec::new();
        cmd.write_long_help(&mut buf).unwrap();
        let help = String::from_utf8(buf).unwrap();
        insta::assert_snapshot!("cli_help", help);
    }

    #[test]
    fn exit_code_0_for_first_bad() {
        let outcome = LocalizationOutcome::FirstBad {
            last_good: faultline_types::CommitId("aaa".into()),
            first_bad: faultline_types::CommitId("bbb".into()),
            confidence: Confidence::high(),
        };
        assert_eq!(outcome_to_operator_code(&outcome), OperatorCode::Success);
        assert_eq!(exit_code_for_operator_code(OperatorCode::Success), 0);
    }

    #[test]
    fn exit_code_1_for_suspect_window() {
        let outcome = LocalizationOutcome::SuspectWindow {
            lower_bound_exclusive: faultline_types::CommitId("aaa".into()),
            upper_bound_inclusive: faultline_types::CommitId("bbb".into()),
            confidence: Confidence::medium(),
            reasons: vec![AmbiguityReason::NonMonotonicEvidence],
        };
        assert_eq!(
            outcome_to_operator_code(&outcome),
            OperatorCode::SuspectWindow
        );
        assert_eq!(exit_code_for_operator_code(OperatorCode::SuspectWindow), 1);
    }

    #[test]
    fn exit_code_3_for_inconclusive() {
        let outcome = LocalizationOutcome::Inconclusive {
            reasons: vec![AmbiguityReason::MissingPassBoundary],
        };
        assert_eq!(
            outcome_to_operator_code(&outcome),
            OperatorCode::Inconclusive
        );
        assert_eq!(exit_code_for_operator_code(OperatorCode::Inconclusive), 3);
    }

    #[test]
    fn exit_code_2_for_execution_error() {
        assert_eq!(exit_code_for_operator_code(OperatorCode::ExecutionError), 2);
    }

    #[test]
    fn exit_code_4_for_invalid_input() {
        assert_eq!(exit_code_for_operator_code(OperatorCode::InvalidInput), 4);
    }

    #[test]
    fn all_exit_codes_are_distinct() {
        let codes = [
            exit_code_for_operator_code(OperatorCode::Success),
            exit_code_for_operator_code(OperatorCode::SuspectWindow),
            exit_code_for_operator_code(OperatorCode::ExecutionError),
            exit_code_for_operator_code(OperatorCode::Inconclusive),
            exit_code_for_operator_code(OperatorCode::InvalidInput),
        ];
        let mut unique = codes.to_vec();
        unique.sort();
        unique.dedup();
        assert_eq!(
            codes.len(),
            unique.len(),
            "exit codes must be distinct: {:?}",
            codes
        );
    }

    // --- Run mode mutual exclusion tests ---

    #[test]
    fn rejects_resume_and_force() {
        let err =
            validate_run_mode(true, true, false).expect_err("should reject --resume + --force");
        assert!(
            err.to_string()
                .contains("--resume and --force are mutually exclusive"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn rejects_resume_and_fresh() {
        let err =
            validate_run_mode(true, false, true).expect_err("should reject --resume + --fresh");
        assert!(
            err.to_string()
                .contains("--resume and --fresh are mutually exclusive"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn rejects_force_and_fresh() {
        let err =
            validate_run_mode(false, true, true).expect_err("should reject --force + --fresh");
        assert!(
            err.to_string()
                .contains("--force and --fresh are mutually exclusive"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn accepts_single_run_modes() {
        assert!(validate_run_mode(true, false, false).is_ok());
        assert!(validate_run_mode(false, true, false).is_ok());
        assert!(validate_run_mode(false, false, true).is_ok());
        assert!(validate_run_mode(false, false, false).is_ok());
    }

    // --- --env validation tests ---

    #[test]
    fn accepts_valid_env_vars() {
        assert!(validate_env_vars(&["FOO=bar".into(), "BAZ=123".into()]).is_ok());
    }

    #[test]
    fn accepts_env_var_with_equals_in_value() {
        assert!(validate_env_vars(&["FOO=bar=baz".into()]).is_ok());
    }

    #[test]
    fn rejects_env_var_missing_equals() {
        let err = validate_env_vars(&["FOOBAR".into()]).expect_err("should reject --env without =");
        assert!(
            err.to_string().contains("expected KEY=VALUE format"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn accepts_empty_env_list() {
        assert!(validate_env_vars(&[]).is_ok());
    }

    // --- --shell validation tests ---

    #[test]
    fn accepts_valid_shell_kinds() {
        assert!(validate_shell(&Some("sh".into())).is_ok());
        assert!(validate_shell(&Some("cmd".into())).is_ok());
        assert!(validate_shell(&Some("powershell".into())).is_ok());
    }

    #[test]
    fn accepts_no_shell() {
        assert!(validate_shell(&None).is_ok());
    }

    #[test]
    fn rejects_unknown_shell() {
        let err = validate_shell(&Some("fish".into())).expect_err("should reject unknown shell");
        assert!(
            err.to_string()
                .contains("expected one of sh, cmd, powershell"),
            "unexpected error: {}",
            err
        );
    }

    // --- Proptest strategies for P32 ---

    fn arb_commit_id() -> impl Strategy<Value = faultline_types::CommitId> {
        "[a-f0-9]{8,40}".prop_map(faultline_types::CommitId)
    }

    fn arb_confidence() -> impl Strategy<Value = Confidence> {
        prop_oneof![
            Just(Confidence::high()),
            Just(Confidence::medium()),
            Just(Confidence::low()),
        ]
    }

    fn arb_ambiguity_reason() -> impl Strategy<Value = AmbiguityReason> {
        prop_oneof![
            Just(AmbiguityReason::MissingPassBoundary),
            Just(AmbiguityReason::MissingFailBoundary),
            Just(AmbiguityReason::NonMonotonicEvidence),
            Just(AmbiguityReason::SkippedRevision),
            Just(AmbiguityReason::IndeterminateRevision),
            Just(AmbiguityReason::UntestableWindow),
            Just(AmbiguityReason::BoundaryValidationFailed),
            Just(AmbiguityReason::NeedsMoreProbes),
            Just(AmbiguityReason::MaxProbesExhausted),
        ]
    }

    fn arb_localization_outcome() -> impl Strategy<Value = LocalizationOutcome> {
        prop_oneof![
            (arb_commit_id(), arb_commit_id(), arb_confidence()).prop_map(
                |(last_good, first_bad, confidence)| {
                    LocalizationOutcome::FirstBad {
                        last_good,
                        first_bad,
                        confidence,
                    }
                }
            ),
            (
                arb_commit_id(),
                arb_commit_id(),
                arb_confidence(),
                prop::collection::vec(arb_ambiguity_reason(), 1..4),
            )
                .prop_map(
                    |(lower_bound_exclusive, upper_bound_inclusive, confidence, reasons)| {
                        LocalizationOutcome::SuspectWindow {
                            lower_bound_exclusive,
                            upper_bound_inclusive,
                            confidence,
                            reasons,
                        }
                    }
                ),
            prop::collection::vec(arb_ambiguity_reason(), 1..4)
                .prop_map(|reasons| LocalizationOutcome::Inconclusive { reasons }),
        ]
    }

    // Feature: v01-hardening, Property 32: OperatorCode Exit Code Mapping
    // **Validates: Requirement 8.1**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_operator_code_exit_code_mapping(outcome in arb_localization_outcome()) {
            let code = outcome_to_operator_code(&outcome);
            let exit = exit_code_for_operator_code(code);

            // Verify correct mapping per outcome variant
            match &outcome {
                LocalizationOutcome::FirstBad { .. } => {
                    prop_assert_eq!(code, OperatorCode::Success);
                    prop_assert_eq!(exit, 0);
                }
                LocalizationOutcome::SuspectWindow { .. } => {
                    prop_assert_eq!(code, OperatorCode::SuspectWindow);
                    prop_assert_eq!(exit, 1);
                }
                LocalizationOutcome::Inconclusive { .. } => {
                    prop_assert_eq!(code, OperatorCode::Inconclusive);
                    prop_assert_eq!(exit, 3);
                }
            }

            // Verify all exit codes are distinct from each other and from error codes
            let all_codes = [
                exit_code_for_operator_code(OperatorCode::Success),
                exit_code_for_operator_code(OperatorCode::SuspectWindow),
                exit_code_for_operator_code(OperatorCode::ExecutionError),
                exit_code_for_operator_code(OperatorCode::Inconclusive),
                exit_code_for_operator_code(OperatorCode::InvalidInput),
            ];
            let mut unique = all_codes.to_vec();
            unique.sort();
            unique.dedup();
            prop_assert_eq!(
                all_codes.len(),
                unique.len(),
                "all exit codes must be distinct: {:?}",
                all_codes
            );
        }
    }

    // Feature: v01-hardening, Property 36: CLI Help Flag Completeness
    // **Validates: Requirement 9.6**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_cli_help_flag_completeness(_seed in any::<u32>()) {
            let mut cmd = Cli::command();
            let mut buf = Vec::new();
            cmd.write_long_help(&mut buf).unwrap();
            let help = String::from_utf8(buf).unwrap();

            // All flags added in the hardening pass
            let hardening_flags = [
                "--resume",
                "--force",
                "--fresh",
                "--no-render",
                "--shell",
                "--env",
                "--retries",
                "--stability-threshold",
                "--markdown",
            ];

            // All pre-existing flags
            let preexisting_flags = [
                "--good",
                "--bad",
                "--repo",
                "--cmd",
                "--program",
                "--arg",
                "--kind",
                "--first-parent",
                "--timeout-seconds",
                "--output-dir",
                "--max-probes",
            ];

            for flag in hardening_flags.iter().chain(preexisting_flags.iter()) {
                prop_assert!(
                    help.contains(flag),
                    "--help output missing expected flag '{}'\nFull help:\n{}",
                    flag,
                    help,
                );
            }
        }
    }
}
