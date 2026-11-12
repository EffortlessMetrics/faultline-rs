//! Thin local-file adapter. Never runs supplied commands or contacts a provider.

use clap::{Parser, Subcommand, ValueEnum};
use faultline_integration::{
    ChangeUnit, IntegrationSequence, IntegrationSnapshotId, IntegrationState, IntegrationStateId,
    StateEquivalence, StateProvenance, TreeId,
};
use faultline_queue::{
    DiagnosisInput, DiagnosisPolicy, DiagnosisReport, DiagnosisStatus, EvidenceSource,
    PhaseConclusion, PhaseReceipt, SCHEMA_VERSION, TargetIdentity, diagnose,
};
use faultline_types::CommitId;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

const MAX_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Parser)]
#[command(
    version,
    about = "Replay captured queue evidence; plan, never execute, probes"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Diagnose one captured experiment. Missing evidence is not a green result.
    Diagnose {
        #[arg(long)]
        input: PathBuf,
        /// A new file only; existing files are never overwritten.
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "json")]
        format: Format,
    },
    /// Print the input or report JSON Schema.
    Schema {
        #[arg(value_enum)]
        kind: SchemaKind,
    },
    /// Print explicitly synthetic 20-PR input with eligible provider endpoints.
    Demo,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Json,
    Markdown,
}

#[derive(Clone, Copy, ValueEnum)]
enum SchemaKind {
    Input,
    Report,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("faultline-queue: {error}");
            ExitCode::from(3)
        }
    }
}

fn run() -> Result<u8, Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Diagnose {
            input,
            output,
            format,
        } => {
            let file = File::open(input)?;
            let mut bytes = Vec::new();
            file.take(MAX_INPUT_BYTES + 1).read_to_end(&mut bytes)?;
            if u64::try_from(bytes.len())? > MAX_INPUT_BYTES {
                return Err("input exceeds the 16 MiB limit".into());
            }
            let packet: DiagnosisInput = serde_json::from_slice(&bytes)?;
            let report = diagnose(&packet)?;
            let rendered = match format {
                Format::Json => serde_json::to_string_pretty(&report)?,
                Format::Markdown => markdown(&report),
            };
            if let Some(path) = output {
                let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
                writeln!(file, "{rendered}")?;
                file.sync_all()?;
            } else {
                writeln!(io::stdout().lock(), "{rendered}")?;
            }
            Ok(match report.status {
                DiagnosisStatus::Localized | DiagnosisStatus::NoTargetFailure => 0,
                _ => 2,
            })
        }
        Command::Schema { kind } => {
            let schema = match kind {
                SchemaKind::Input => schemars::schema_for!(DiagnosisInput),
                SchemaKind::Report => schemars::schema_for!(DiagnosisReport),
            };
            writeln!(
                io::stdout().lock(),
                "{}",
                serde_json::to_string_pretty(&schema)?
            )?;
            Ok(0)
        }
        Command::Demo => {
            writeln!(
                io::stdout().lock(),
                "{}",
                serde_json::to_string_pretty(&demo()?)?
            )?;
            Ok(0)
        }
    }
}

fn safe_text(value: &str) -> String {
    let mut text = String::new();
    for ch in value.chars() {
        match ch {
            '&' => text.push_str("&amp;"),
            '<' => text.push_str("&lt;"),
            '>' => text.push_str("&gt;"),
            '`' | '*' | '_' | '[' | ']' | '(' | ')' | '|' | '#' | '\\' => {
                text.push('\\');
                text.push(ch);
            }
            ch if ch.is_control() => text.push(' '),
            _ => text.push(ch),
        }
    }
    text
}

fn markdown(report: &DiagnosisReport) -> String {
    let mut text = format!(
        "# Queue diagnosis\n\nStatus: **{:?}**\n\nSnapshot: {}\n\nTarget: {} / {}\n\nPredicate: {}\n\nContext: {}\n\nRecorded native launches: {}. Remaining launch budget: {}.\n",
        report.status,
        safe_text(&report.snapshot_id),
        safe_text(&report.target.phase),
        safe_text(&report.target.failure_signature),
        safe_text(&report.target.predicate_id),
        safe_text(&report.target.context_id),
        report.recorded_launches,
        report.remaining_launches,
    );
    if let Some(outcome) = &report.outcome {
        match outcome {
            faultline_integration::IntegrationOutcome::FirstFailingTransition {
                last_passing,
                first_failing,
                transition,
                basis,
                ..
            } => {
                text.push_str(&format!(
                    "\nBoundary: S{} passes; S{} fails. Basis: {:?}.\n",
                    last_passing.ordinal, first_failing.ordinal, basis,
                ));
                match transition {
                    ChangeUnit::PullRequest {
                        provider,
                        repository,
                        number,
                        head,
                    } => {
                        text.push_str(&format!(
                            "\nIntroduced PR: {} / {} #{} at captured head {}.\n",
                            safe_text(provider),
                            safe_text(repository),
                            number,
                            safe_text(&head.0)
                        ));
                    }
                    ChangeUnit::Commit { repository, commit } => {
                        text.push_str(&format!(
                            "\nIntroduced commit: {} at {}.\n",
                            safe_text(repository),
                            safe_text(&commit.0)
                        ));
                    }
                }
            }
            faultline_integration::IntegrationOutcome::SuspectTransitions {
                lower_bound_exclusive,
                upper_bound_inclusive,
                ..
            } => text.push_str(&format!(
                "\nSuspect transitions: S{} through S{}. No single-PR attribution.\n",
                lower_bound_exclusive.ordinal.saturating_add(1),
                upper_bound_inclusive.ordinal,
            )),
            _ => text.push_str("\nNo supported first-transition attribution.\n"),
        }
    }
    if let Some(plan) = &report.next_probe {
        text.push_str(&format!(
            "\nNext proposed probe: S{}, commit {}, tree {}. This command did not launch it.\n",
            plan.state.ordinal,
            safe_text(&plan.state.commit.0),
            safe_text(&plan.state.tree.0),
        ));
    }
    text.push_str(
        "\n## Evidence\n\n| State | Classification | Retained attempts |\n|---|---|---:|\n",
    );
    for state in &report.states {
        text.push_str(&format!(
            "| S{} | {:?} | {} |\n",
            state.state.ordinal,
            state.evidence,
            state.receipt_ids.len()
        ));
    }
    text.push_str(&format!(
        "\nExcluded receipts: {}. Pending launch receipts: {}.\n",
        report.excluded_receipts.len(),
        report.pending_attempts.len()
    ));
    for attempt in &report.pending_attempts {
        text.push_str(&format!("\nPending attempt: {}.\n", safe_text(attempt)));
    }
    for receipt in &report.excluded_receipts {
        text.push_str(&format!(
            "\nExcluded receipt: {}: {}.\n",
            safe_text(&receipt.attempt_id),
            safe_text(&receipt.reason)
        ));
    }
    for reason in &report.reason_codes {
        text.push_str(&format!("\nReason: {}.\n", safe_text(reason)));
    }
    text.push_str("\n## Limits\n");
    for limit in &report.limitations {
        text.push_str(&format!("\n{}\n", safe_text(limit)));
    }
    text
}

fn demo() -> Result<DiagnosisInput, Box<dyn std::error::Error>> {
    let snapshot = IntegrationSnapshotId("synthetic-demo-not-live-evidence".into());
    let mut changes = Vec::new();
    let mut states = Vec::new();
    for ordinal in 0..=20 {
        let transition = if ordinal == 0 {
            None
        } else {
            Some(ChangeUnit::PullRequest {
                provider: "synthetic".into(),
                repository: "fixture/queue".into(),
                number: u64::try_from(ordinal)?,
                head: CommitId(format!("{:040x}", ordinal + 100)),
            })
        };
        if let Some(change) = &transition {
            changes.push(change.clone());
        }
        states.push(IntegrationState {
            id: IntegrationStateId(format!("S{ordinal}")),
            ordinal,
            commit: CommitId(format!("{:040x}", ordinal + 1)),
            tree: TreeId(format!("{:040x}", ordinal + 1000)),
            included_changes: changes.clone(),
            transition,
            equivalence: StateEquivalence::ExactCommit,
            provenance: StateProvenance::Provider {
                provider: "synthetic".into(),
                snapshot: snapshot.clone(),
            },
        });
    }
    let sequence = IntegrationSequence::try_new("synthetic", snapshot, states)?;
    let target = TargetIdentity {
        phase: "tests".into(),
        failure_signature: "synthetic:assertion:queue-regression".into(),
        predicate_id: "synthetic-predicate-v1".into(),
        context_id: "synthetic-context-v1".into(),
    };
    let mut receipts = Vec::new();
    for state in sequence
        .states
        .iter()
        .filter(|s| s.ordinal == 0 || s.ordinal == 20)
    {
        let passing = state.ordinal == 0;
        receipts.push(PhaseReceipt {
            attempt_id: format!("synthetic-provider-{}", state.ordinal),
            snapshot_id: sequence.snapshot_id.0.clone(),
            state_id: state.id.0.clone(),
            commit: state.commit.0.clone(),
            tree: state.tree.0.clone(),
            phase: target.phase.clone(),
            predicate_id: target.predicate_id.clone(),
            context_id: target.context_id.clone(),
            source: EvidenceSource::Provider,
            conclusion: if passing {
                PhaseConclusion::Passed
            } else {
                PhaseConclusion::Failed
            },
            exit_code: Some(if passing { 0 } else { 1 }),
            failure_signature: if passing {
                None
            } else {
                Some(target.failure_signature.clone())
            },
            log_id: "synthetic-no-real-log".into(),
        });
    }
    Ok(DiagnosisInput {
        schema_version: SCHEMA_VERSION.into(),
        sequence,
        target,
        policy: DiagnosisPolicy {
            max_launches: 5,
            minimum_attempts: 1,
            assume_monotonic: true,
            required_equivalence: StateEquivalence::ExactCommit,
        },
        launches: Vec::new(),
        receipts,
    })
}
