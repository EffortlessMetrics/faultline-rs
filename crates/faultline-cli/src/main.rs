use clap::Parser;
use faultline_app::FaultlineApp;
use faultline_codes::ProbeKind;
use faultline_git::GitAdapter;
use faultline_probe_exec::ExecProbeAdapter;
use faultline_render::ReportRenderer;
use faultline_store::FileRunStore;
use faultline_types::{AnalysisRequest, HistoryMode, ProbeSpec, RevisionSpec, SearchPolicy, ShellKind};
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

fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    if cli.cmd.is_none() && cli.program.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "either --cmd or --program must be provided",
        )
        .into());
    }
    if cli.cmd.is_some() && cli.program.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "use either --cmd or --program, not both",
        )
        .into());
    }

    let probe_kind = cli
        .kind
        .parse::<ProbeKind>()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

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

    println!("run-id      {}", localized.report.run_id);
    println!("observations {}", localized.report.observations.len());
    println!("artifacts   {}", renderer.output_dir().display());
    println!("outcome     {:?}", localized.report.outcome);
    Ok(())
}
