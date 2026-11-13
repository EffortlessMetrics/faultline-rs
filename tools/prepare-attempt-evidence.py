from pathlib import Path
import os
import re
import sys


def edit(name, old, new):
    path = Path(name)
    text = path.read_text()
    if text.count(old) != 1:
        raise RuntimeError(f'{name}: expected one match, found {text.count(old)}: {old[:100]}')
    path.write_text(text.replace(old, new))


def write(name, text):
    path = Path(name)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def index_tests(names, owner):
    path = Path('docs/scenarios/scenario_index.md')
    text = path.read_text()
    for name in names:
        if f'`{name}`' not in text:
            text += f'\n| `{name}` | Retain original attempt evidence | Native probe fixtures | {owner} | Attempt journal | No lost or rewritten execution | #62 #92 | app/adapter | retry-evidence | v0.4 | {owner} | P0 | {owner} | no |'
    path.write_text(text + '\n')


def queue_fix():
    edit('crates/faultline-queue/src/lib.rs', '    input.sequence.validate()?;', '''    input.sequence.validate()?;
    identifier(&input.sequence.snapshot_id.0, "sequence snapshot_id")?;
    for state in &input.sequence.states {
        identifier(&state.id.0, "sequence state_id")?;
        identifier(&state.commit.0, "sequence commit")?;
        identifier(&state.tree.0, "sequence tree")?;
    }''')
    write('crates/faultline-queue/tests/identifier_contract.rs', r'''mod common;
use common::{endpoints, packet};
use faultline_integration::{IntegrationSnapshotId, StateProvenance};
use faultline_queue::diagnose;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn planned_state_identifiers_must_be_receiptable() -> TestResult {
    for field in ["state", "commit", "tree", "snapshot"] {
        for bad in ["bad\n".to_string(), "bad\0".to_string(), "x".repeat(1025)] {
            let mut input = packet(2)?;
            endpoints(&mut input)?;
            if field == "snapshot" {
                input.sequence.snapshot_id = IntegrationSnapshotId(bad.clone());
                for state in &mut input.sequence.states {
                    state.provenance = StateProvenance::Provider {
                        provider: "synthetic".into(), snapshot: IntegrationSnapshotId(bad.clone()),
                    };
                }
            } else {
                let state = input.sequence.states.get_mut(1).ok_or("midpoint")?;
                match field {
                    "state" => state.id.0 = bad,
                    "commit" => state.commit.0 = bad,
                    _ => state.tree.0 = bad,
                }
            }
            assert!(diagnose(&input).is_err(), "unreceiptable {field} must fail before planning");
        }
    }
    Ok(())
}

#[test]
fn bounded_unicode_state_identity_remains_usable() -> TestResult {
    let mut input = packet(2)?;
    input.sequence.states.get_mut(1).ok_or("midpoint")?.id.0 = "état-一".into();
    endpoints(&mut input)?;
    assert_eq!(diagnose(&input)?.next_probe.ok_or("probe")?.state.ordinal, 1);
    Ok(())
}
''')
    index_tests(['planned_state_identifiers_must_be_receiptable', 'bounded_unicode_state_identity_remains_usable'], 'faultline-queue')


def producer_fix():
    write('crates/faultline-types/src/attempts.rs', r'''//! Conservative aggregation of retained executions. Process metadata remains
//! on original attempts, never borrowed to describe an aggregate decision.
use crate::{CommitId, FaultlineError, FlakeSignal, ProbeObservation, Result, compute_flake_signal};
use faultline_codes::ObservationClass;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProbeAggregate {
    pub commit: CommitId,
    /// Run-local immutable attempt sequence numbers, in execution order.
    pub attempt_ids: Vec<u64>,
    pub class: ObservationClass,
    pub counts: FlakeSignal,
    pub decision_rule: String,
}

pub fn aggregate_attempts(attempts: &[ProbeObservation]) -> Result<ProbeAggregate> {
    let first = attempts.first().ok_or_else(|| FaultlineError::Domain("cannot aggregate no attempts".into()))?;
    let mut identities = BTreeSet::new();
    for attempt in attempts {
        if attempt.commit != first.commit || attempt.kind != first.kind || attempt.flake_signal.is_some() {
            return Err(FaultlineError::Domain("aggregate requires raw attempts for one commit and probe kind".into()));
        }
        if !identities.insert(attempt.sequence_index) {
            return Err(FaultlineError::Domain("duplicate attempt identity".into()));
        }
    }
    u32::try_from(attempts.len()).map_err(|_| FaultlineError::Domain("too many attempts".into()))?;
    let classes: Vec<_> = attempts.iter().map(|a| a.class).collect();
    let counts = compute_flake_signal(&classes, 1.0);
    let class = if counts.is_stable { first.class } else { ObservationClass::Indeterminate };
    Ok(ProbeAggregate {
        commit: first.commit.clone(), attempt_ids: attempts.iter().map(|a| a.sequence_index).collect(),
        class, counts, decision_rule: "unanimous_complete_v1".into(),
    })
}

/// Compatibility input for search, not a raw process result. Multi-attempt
/// summaries contain no invented exit code, duration, output or working directory.
pub fn aggregate_projection(attempts: &[ProbeObservation]) -> Result<ProbeObservation> {
    let aggregate = aggregate_attempts(attempts)?;
    let last = attempts.last().ok_or_else(|| FaultlineError::Domain("missing attempt".into()))?;
    if attempts.len() == 1 { return Ok(last.clone()); }
    Ok(ProbeObservation {
        commit: aggregate.commit, class: aggregate.class, kind: last.kind,
        exit_code: None, timed_out: false, duration_ms: 0, stdout: String::new(), stderr: String::new(),
        sequence_index: last.sequence_index, signal_number: None,
        probe_command: String::new(), working_dir: String::new(), flake_signal: Some(aggregate.counts),
    })
}
''')
    edit('crates/faultline-types/src/lib.rs', 'pub type Result<T> =', 'mod attempts;\npub use attempts::{ProbeAggregate, aggregate_attempts, aggregate_projection};\n\npub type Result<T> =')
    edit('crates/faultline-types/src/lib.rs', '    pub observations: Vec<ProbeObservation>,', '''    pub observations: Vec<ProbeObservation>,
    /// Original completed executions, including interrupted batches.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probe_attempts: Vec<ProbeObservation>,
    /// Derived decisions linked to their immutable execution identities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probe_aggregates: Vec<ProbeAggregate>,''')
    for path in Path('crates').rglob('*.rs'):
        text = path.read_text()
        changed = re.sub(r'^(\s*)provenance: (None|Some\()', r'\1probe_attempts: Vec::new(),\n\1probe_aggregates: Vec::new(),\n\1provenance: \2', text, flags=re.M)
        if changed != text:
            path.write_text(changed)
    edit('crates/faultline-types/src/lib.rs', '    for obs in &mut report.observations {', '    for obs in report.observations.iter_mut().chain(report.probe_attempts.iter_mut()) {')
    edit('crates/faultline-ports/src/lib.rs', '    fn load_observations(&self, run: &RunHandle) -> Result<Vec<ProbeObservation>>;', '''    fn load_observations(&self, run: &RunHandle) -> Result<Vec<ProbeObservation>>;
    /// Immutable completed raw attempts, distinct from the per-commit cache.
    /// Adapters must implement this before execution can proceed.
    fn load_probe_attempts(&self, _run: &RunHandle) -> Result<Vec<ProbeObservation>> {
        Err(faultline_types::FaultlineError::Store("attempt journal unsupported by this adapter".into()))
    }
    fn save_probe_attempt(&self, _run: &RunHandle, _attempt: &ProbeObservation) -> Result<()> {
        Err(faultline_types::FaultlineError::Store("attempt journal unsupported by this adapter".into()))
    }''')
    write('crates/faultline-store/src/attempts.rs', r'''use faultline_types::{FaultlineError, ProbeObservation, Result, RunHandle};
use std::fs::{self, OpenOptions};
use std::io::Write;

fn directory(run: &RunHandle) -> Result<std::path::PathBuf> {
    let path = run.root.join("attempts");
    match fs::symlink_metadata(&path) {
        Ok(meta) if !meta.file_type().is_dir() => return Err(FaultlineError::Store("attempt journal is not an owned directory".into())),
        Ok(_) => {},
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
        Err(error) => return Err(error.into()),
    }
    Ok(path)
}

pub(super) fn load(run: &RunHandle) -> Result<Vec<ProbeObservation>> {
    let root = directory(run)?;
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut attempts = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            return Err(FaultlineError::Store("non-regular attempt journal entry".into()));
        }
        let attempt: ProbeObservation = serde_json::from_slice(&fs::read(entry.path())?)?;
        let expected = format!("{:020}.json", attempt.sequence_index);
        if entry.file_name() != std::ffi::OsStr::new(&expected) || attempt.flake_signal.is_some() {
            return Err(FaultlineError::Store("attempt journal identity or raw-evidence mismatch".into()));
        }
        attempts.push(attempt);
    }
    attempts.sort_by_key(|a| a.sequence_index);
    Ok(attempts)
}

pub(super) fn save(run: &RunHandle, attempt: &ProbeObservation) -> Result<()> {
    if attempt.flake_signal.is_some() {
        return Err(FaultlineError::Store("cannot journal an aggregate as a raw attempt".into()));
    }
    let root = directory(run)?;
    fs::create_dir_all(&root)?;
    directory(run)?;
    let path = root.join(format!("{:020}.json", attempt.sequence_index));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(mut file) => {
            file.write_all(&serde_json::to_vec_pretty(attempt)?)?;
            file.sync_all()?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if !fs::symlink_metadata(&path)?.file_type().is_file() {
                return Err(FaultlineError::Store("non-regular attempt record".into()));
            }
            let existing: ProbeObservation = serde_json::from_slice(&fs::read(&path)?)?;
            if existing == *attempt { Ok(()) }
            else { Err(FaultlineError::Store("immutable attempt identity already has different evidence".into())) }
        }
        Err(error) => Err(error.into()),
    }
}
''')
    edit('crates/faultline-store/src/lib.rs', 'use faultline_ports::RunStorePort;', 'mod attempts;\nuse faultline_ports::RunStorePort;')
    edit('crates/faultline-store/src/lib.rs', 'impl RunStorePort for FileRunStore {', '''impl RunStorePort for FileRunStore {
    fn load_probe_attempts(&self, run: &RunHandle) -> Result<Vec<ProbeObservation>> { attempts::load(run) }
    fn save_probe_attempt(&self, run: &RunHandle, attempt: &ProbeObservation) -> Result<()> { attempts::save(run, attempt) }''')
    edit('crates/faultline-app/src/lib.rs', '    compute_flake_signal, now_epoch_seconds,', '    ProbeAggregate, ProbeObservation, aggregate_attempts, aggregate_projection, now_epoch_seconds,')
    path = Path('crates/faultline-app/src/lib.rs')
    text = path.read_text()
    start = text.index('        // Replay cached observations,')
    end = text.index('\n        self.ensure_boundary(', start)
    text = text[:start] + '''        // The journal is authoritative; cached decisions require their originals.
        let mut attempts = self.store.load_probe_attempts(&run)?;
        let mut next_sequence_index = match attempts.iter().map(|a| a.sequence_index).max() {
            Some(index) => index.checked_add(1).ok_or_else(|| FaultlineError::Store("attempt index overflow".into()))?,
            None => 0,
        };
        let mut aggregates = Vec::new();
        for observation in self.store.load_observations(&run)? {
            let required = usize::try_from(request.policy.flake_policy.retries)
                .map_err(|_| FaultlineError::InvalidInput("retry count overflow".into()))?
                .checked_add(1).ok_or_else(|| FaultlineError::InvalidInput("retry count overflow".into()))?;
            let mut original: Vec<_> = attempts.iter()
                .filter(|a| a.commit == observation.commit && a.sequence_index <= observation.sequence_index)
                .rev().take(required).cloned().collect();
            original.reverse();
            if original.len() != required || aggregate_projection(&original)? != observation {
                return Err(FaultlineError::Store("cached decision lacks matching raw attempts; recollect with --force (the journal is retained)".into()));
            }
            aggregates.push(aggregate_attempts(&original)?);
            session.record(observation)?;
        }
''' + text[end:]
    text = text.replace('            &mut next_sequence_index,\n        )?;', '            &mut next_sequence_index,\n            &mut attempts,\n            &mut aggregates,\n        )?;')
    start = text.index('        let retries = request.policy.flake_policy.retries;')
    end = text.index('\n        let outcome = session.outcome();', start)
    text = text[:start] + '''        let endpoints_eligible = sequence.revisions.first()
            .and_then(|c| session.get_observation(c)).is_some_and(|o| o.class == ObservationClass::Pass)
            && sequence.revisions.last().and_then(|c| session.get_observation(c)).is_some_and(|o| o.class == ObservationClass::Fail);
        while endpoints_eligible && probe_count < max_probes {
            let Some(commit) = session.next_probe() else { break; };
            let observation = self.collect_observation(&run, &request, &commit,
                &mut next_sequence_index, &mut attempts, &mut aggregates)?;
            session.record(observation)?;
            probe_count += 1;
        }
''' + text[end:]
    text = text.replace('            schema_version: "0.1.0".into(),\n            run_id: run.id.clone(),', '            schema_version: "0.4.0".into(),\n            run_id: run.id.clone(),', 1)
    text = text.replace('            probe_attempts: Vec::new(),\n            probe_aggregates: Vec::new(),\n            provenance: None,', '            probe_attempts: attempts,\n            probe_aggregates: aggregates,\n            provenance: None,', 1)
    text = text.replace('        next_sequence_index: &mut u64,\n    ) -> Result<()> {', '        next_sequence_index: &mut u64,\n        attempts: &mut Vec<ProbeObservation>,\n        aggregates: &mut Vec<ProbeAggregate>,\n    ) -> Result<()> {', 1)
    start = text.index('        if !session.has_observation(&commit) {')
    end = text.index('\n        let observed = session', start)
    text = text[:start] + '''        if !session.has_observation(&commit) {
            let observation = self.collect_observation(run, request, &commit,
                next_sequence_index, attempts, aggregates)?;
            session.record(observation)?;
        }
''' + text[end:]
    text = text.replace('        if observed.class != expected {', '''        if observed.class == ObservationClass::Indeterminate
            && observed.flake_signal.as_ref().is_some_and(|signal| !signal.is_stable) {
            return Ok(()); // Unstable endpoints produce an inconclusive report.
        }
        if observed.class != expected {''', 1)
    start = text.index('    /// If stdout or stderr ends with')
    end = text.index('\n}\n\n#[cfg(test)]', start)
    text = text[:start] + '''    fn collect_observation(
        &self, run: &RunHandle, request: &AnalysisRequest, commit: &faultline_types::CommitId,
        next_index: &mut u64, retained: &mut Vec<ProbeObservation>, aggregates: &mut Vec<ProbeAggregate>,
    ) -> Result<ProbeObservation> {
        let count = request.policy.flake_policy.retries.checked_add(1)
            .ok_or_else(|| FaultlineError::InvalidInput("retry count overflow".into()))?;
        let mut batch = Vec::new();
        for _ in 0..count {
            let index = *next_index;
            let following = index.checked_add(1).ok_or_else(|| FaultlineError::Store("attempt index overflow".into()))?;
            let checkout = self.checkout.checkout_revision(commit)?;
            let execution = self.probe.run(&checkout, &request.probe).and_then(|mut observation| {
                if observation.commit != *commit || observation.flake_signal.is_some() {
                    return Err(FaultlineError::Probe("probe returned another commit or an aggregate".into()));
                }
                observation.sequence_index = index;
                // Retain original output before cleanup, another retry or aggregation.
                self.store.save_probe_attempt(run, &observation)?;
                *next_index = following;
                retained.push(observation.clone());
                Ok(observation)
            });
            let cleanup = self.checkout.cleanup_checkout(&checkout);
            match (execution, cleanup) {
                (Ok(observation), Ok(())) => batch.push(observation),
                (Err(error), _) => return Err(error),
                (Ok(_), Err(error)) => return Err(error),
            }
        }
        let aggregate = aggregate_attempts(&batch)?;
        let observation = aggregate_projection(&batch)?;
        self.store.save_observation(run, &observation)?;
        aggregates.push(aggregate);
        Ok(observation)
    }
''' + text[end:]
    text = text.replace('    impl RunStorePort for MockRunStore {', '''    impl RunStorePort for MockRunStore {
        fn load_probe_attempts(&self, _run: &RunHandle) -> Result<Vec<ProbeObservation>> { Ok(Vec::new()) }
        fn save_probe_attempt(&self, _run: &RunHandle, _attempt: &ProbeObservation) -> Result<()> { Ok(()) }''')
    text = text.replace('    impl RunStorePort for CachedRunStore {', '''    impl RunStorePort for CachedRunStore {
        fn load_probe_attempts(&self, _run: &RunHandle) -> Result<Vec<ProbeObservation>> {
            let mut attempts = self.cached_observations.clone();
            attempts.sort_by_key(|a| a.sequence_index);
            Ok(attempts)
        }
        fn save_probe_attempt(&self, _run: &RunHandle, _attempt: &ProbeObservation) -> Result<()> { Ok(()) }''')
    text = text.replace('result.report.schema_version, "0.1.0",', 'result.report.schema_version, "0.4.0",').replace('schema_version should be 0.1.0 on resumed run', 'schema_version should be 0.4.0 on resumed run')
    text = text.replace('fn integration_flaky_commit_majority_vote()', 'fn integration_flaky_commit_preserves_conflict()')
    text = text.replace('// When a commit is flaky (mixed results), majority vote determines the class', '// Mixed attempts cannot create a boundary by majority vote')
    text = text.replace('// Majority vote: 2 Fail vs 1 Pass → Fail', '// Mixed results remain indeterminate')
    text = text.replace('obs_c2.class,\n            ObservationClass::Fail,\n            "c2 majority should be Fail (2 Fail vs 1 Pass)"', 'obs_c2.class,\n            ObservationClass::Indeterminate,\n            "mixed c2 attempts must not define an exact boundary"')
    path.write_text(text)
    edit('crates/faultline-app/src/lib.rs', '        let outcome = session.outcome();', '''        let mut outcome = session.outcome();
        if aggregates.iter().any(|a| !a.counts.is_stable)
            && matches!(outcome, faultline_types::LocalizationOutcome::FirstBad { .. }) {
            outcome = faultline_types::LocalizationOutcome::Inconclusive {
                reasons: vec![faultline_codes::AmbiguityReason::IndeterminateRevision],
            };
        }
        aggregates.sort_by_key(|a| a.attempt_ids.last().copied().unwrap_or(0));''')
    path = Path('docs/scenarios/scenario_index.md')
    path.write_text(path.read_text().replace('integration_flaky_commit_majority_vote', 'integration_flaky_commit_preserves_conflict'))
    edit('crates/faultline-app/Cargo.toml', '[dev-dependencies]\n', '[dev-dependencies]\nfaultline-store = { path = "../faultline-store" }\nfaultline-render = { path = "../faultline-render" }\nfaultline-sarif = { path = "../faultline-sarif" }\nfaultline-junit = { path = "../faultline-junit" }\nserde_json.workspace = true\ntempfile = "3"\n')
    edit('crates/faultline-render/src/lib.rs', '    // Reproduction command\n', '''    if !report.probe_attempts.is_empty() {
        md.push_str("## Retained attempts\\n\\n");
        for attempt in &report.probe_attempts {
            md.push_str(&format!("Attempt {}: {:?}, exit {:?}, {} ms.\\n\\n", attempt.sequence_index, attempt.class, attempt.exit_code, attempt.duration_ms));
        }
        for aggregate in &report.probe_aggregates {
            md.push_str(&format!("Aggregate: {:?}; attempts {:?}; {}.\\n\\n", aggregate.class, aggregate.attempt_ids, aggregate.decision_rule));
        }
    }
    // Reproduction command
''')
    edit('crates/faultline-render/src/lib.rs', '            log_section,\n', '''            if report.probe_attempts.is_empty() { log_section } else {
                let details = report.probe_attempts.iter().map(|a| format!("Attempt {}: {:?}, exit {:?}, {} ms", a.sequence_index, a.class, a.exit_code, a.duration_ms)).collect::<Vec<_>>().join("\\n");
                format!("{}<h2>Retained attempts</h2><pre>{}</pre>", log_section, escape_html(&details))
            },
''')
    edit('crates/faultline-junit/src/lib.rs', '    if !report.suspect_surface.is_empty() {', '''    for attempt in &report.probe_attempts {
        lines.push(format!("Attempt {}: {:?}, exit {:?}, {} ms", attempt.sequence_index, attempt.class, attempt.exit_code, attempt.duration_ms));
    }
    for aggregate in &report.probe_aggregates {
        lines.push(format!("Aggregate {:?}: attempts {:?}", aggregate.class, aggregate.attempt_ids));
    }
    if !report.suspect_surface.is_empty() {''')
    edit('crates/faultline-sarif/src/lib.rs', '        locations,\n    };', '''        locations,
        properties: if report.probe_attempts.is_empty() { None } else {
            Some(serde_json::json!({"probe_attempts": report.probe_attempts, "probe_aggregates": report.probe_aggregates}))
        },
    };''')
    edit('crates/faultline-sarif/src/lib.rs', '                    locations: suspect_locations,', '                    locations: suspect_locations,\n                    properties: None,')
    edit('crates/faultline-sarif/src/lib.rs', '    locations: Vec<SarifLocation>,\n}', '    locations: Vec<SarifLocation>,\n    #[serde(skip_serializing_if = "Option::is_none")]\n    properties: Option<serde_json::Value>,\n}')
    write('docs/probe-attempt-evidence.md', '''# Retained probe attempts

The history application journals each completed raw execution before cleanup,
the next retry, or aggregation. `attempts/<20-digit sequence_index>.json` uses
create-new writes and file synchronization. Identical writes are idempotent;
conflicting identities, non-regular entries and malformed files fail closed.
`observations.json` remains only a derived per-commit compatibility cache.

`probe_attempts` contains original metadata. Separate `probe_aggregates` link to
run-local attempt IDs and the `unanimous_complete_v1` rule. Multi-attempt search
projections borrow no exit code, output, working directory or duration. Mixed
outcomes are indeterminate even when the legacy stability threshold would admit
a majority. Unstable endpoints produce inconclusive reports, not exact blame.

New reports emit schema 0.4.0. Older JSON remains readable with empty new fields;
this does not invent originals for old aggregate records. Cached decisions without
matching original journal evidence are rejected. `--force` recollects while
retaining old attempts, and new IDs remain monotonic. A normal resume verifies
cached decisions against their retained originals. Interrupted batches retain
completed attempts but are recollected; they are not silently marked complete.
Explicit `--fresh` retains its existing destructive meaning and deletes the run.

The existing single-writer operating boundary still applies. This change is not
atomic launch admission, distributed locking, power-loss recovery, or complete
untruncated logging. A partial journal write blocks replay rather than being
silently skipped. Process errors with no returned observation and unfinished
workers require #59/#91 lifecycle records. Actual launch-budget enforcement is
still #64; the old revision-count cap is not an attempt budget. Reuse identities
remain #63 and filesystem/worktree ownership #61.

JSON, Markdown, HTML, SARIF and JUnit preserve the evidence/aggregate distinction.
The existing redaction path also scrubs the newly exported raw attempt output.
No registry release, live queue worker or required-check cutover is introduced.
''')
    write('crates/faultline-app/tests/attempt_evidence.rs', Path(os.environ['ATTEMPT_TEST_SOURCE']).read_text())
    index_tests(re.findall(r'#\[test\]\s*fn\s+(\w+)', Path('crates/faultline-app/tests/attempt_evidence.rs').read_text()), 'faultline-app')


if __name__ == '__main__':
    {'queue': queue_fix, 'producer': producer_fix}[sys.argv[1]]()
