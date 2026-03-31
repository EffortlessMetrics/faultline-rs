use clap::Parser;
use faultline_app::FaultlineApp;
use faultline_codes::ProbeKind;
use faultline_git::GitAdapter;
use faultline_probe_exec::ExecProbeAdapter;
use faultline_render::ReportRenderer;
use faultline_store::FileRunStore;
use faultline_types::{
    AnalysisRequest, HistoryMode, LocalizationOutcome, ProbeSpec, RevisionSpec, SearchPolicy,
    ShellKind,
};
use std::io;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "faultline")]
#[command(about = "Local-first regression localization for Git repos")]
struct Cli {
    #[arg(long, default_value = ".")]
    repo: PathBuf,

    #[arg(long)]
    good: String,

    #[arg(long)]
    bad: String,

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
}

fn main() {
    if let Err(err) = try_main() {
        eprintln!("faultline: {err}");
        std::process::exit(2);
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

fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    validate_cmd_program(&cli.cmd, &cli.program)?;

    let probe_kind = cli.kind.parse::<ProbeKind>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "invalid --kind '{}': expected one of build, test, lint, perf-threshold, custom",
                cli.kind
            ),
        )
    })?;

    let probe = match (cli.cmd, cli.program) {
        (Some(script), None) => ProbeSpec::Shell {
            kind: probe_kind,
            shell: ShellKind::Default,
            script,
            timeout_seconds: cli.timeout_seconds,
        },
        (None, Some(program)) => ProbeSpec::Exec {
            kind: probe_kind,
            program,
            args: cli.args,
            env: Vec::new(),
            timeout_seconds: cli.timeout_seconds,
        },
        _ => unreachable!("validated above"),
    };

    let request = AnalysisRequest {
        repo_root: cli.repo.clone(),
        good: RevisionSpec(cli.good),
        bad: RevisionSpec(cli.bad),
        history_mode: if cli.first_parent {
            HistoryMode::FirstParent
        } else {
            HistoryMode::AncestryPath
        },
        probe,
        policy: SearchPolicy {
            max_probes: cli.max_probes,
            edge_refine_threshold: 6,
        },
    };

    let git = GitAdapter::new(&cli.repo)?;
    let store = FileRunStore::new(cli.repo.join(".faultline").join("runs"))?;
    let probe = ExecProbeAdapter;
    let app = FaultlineApp::new(&git, &git, &probe, &store);
    let localized = app.localize(request)?;

    let renderer = ReportRenderer::new(cli.output_dir);
    renderer.render(&localized.report)?;

    println!("run-id       {}", localized.report.run_id);
    println!("observations {}", localized.report.observations.len());
    println!("output-dir   {}", renderer.output_dir().display());
    println!("outcome      {}", format_outcome(&localized.report.outcome));
    Ok(())
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
        ];

        for flag in &expected_flags {
            assert!(
                help.contains(flag),
                "--help output missing expected flag '{flag}'.\nFull help:\n{help}"
            );
        }
    }
}
