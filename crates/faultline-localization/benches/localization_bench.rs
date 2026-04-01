use criterion::{Criterion, criterion_group, criterion_main};
use faultline_codes::{ObservationClass, ProbeKind};
use faultline_localization::LocalizationSession;
use faultline_types::{CommitId, ProbeObservation, RevisionSequence, SearchPolicy};

/// Build a `RevisionSequence` of `n` commits labeled "commit-0" .. "commit-{n-1}".
fn make_sequence(n: usize) -> RevisionSequence {
    RevisionSequence {
        revisions: (0..n).map(|i| CommitId(format!("commit-{i}"))).collect(),
    }
}

/// Create a minimal `ProbeObservation` for a given commit and class.
fn obs(commit: &str, class: ObservationClass) -> ProbeObservation {
    ProbeObservation {
        commit: CommitId(commit.to_string()),
        class,
        kind: ProbeKind::Test,
        exit_code: Some(match class {
            ObservationClass::Pass => 0,
            ObservationClass::Skip => 125,
            _ => 1,
        }),
        timed_out: false,
        duration_ms: 1,
        stdout: String::new(),
        stderr: String::new(),
        sequence_index: 0,
        signal_number: None,
        probe_command: String::new(),
        working_dir: String::new(),
        flake_signal: None,
    }
}

/// Run a full binary-narrowing loop: call `next_probe`, classify with a clean
/// pass/fail boundary at `fault_index`, record, repeat until convergence.
fn run_narrowing(n: usize) {
    let seq = make_sequence(n);
    let policy = SearchPolicy {
        max_probes: n, // allow enough probes
        ..SearchPolicy::default()
    };
    let mut session = LocalizationSession::new(seq, policy).unwrap();
    let fault_index = n / 2;

    while let Some(commit) = session.next_probe() {
        let idx: usize = commit.0.strip_prefix("commit-").unwrap().parse().unwrap();
        let class = if idx < fault_index {
            ObservationClass::Pass
        } else {
            ObservationClass::Fail
        };
        session.record(obs(&commit.0, class)).unwrap();
    }

    // Ensure we actually converged
    let _ = session.outcome();
}

/// Same as `run_narrowing` but 20 % of probes return `Skip` (every 5th probe).
fn run_narrowing_with_skips(n: usize) {
    let seq = make_sequence(n);
    let policy = SearchPolicy {
        max_probes: n,
        ..SearchPolicy::default()
    };
    let mut session = LocalizationSession::new(seq, policy).unwrap();
    let fault_index = n / 2;
    let mut probe_count: usize = 0;

    while let Some(commit) = session.next_probe() {
        let idx: usize = commit.0.strip_prefix("commit-").unwrap().parse().unwrap();
        probe_count += 1;
        let class = if probe_count.is_multiple_of(5) {
            ObservationClass::Skip
        } else if idx < fault_index {
            ObservationClass::Pass
        } else {
            ObservationClass::Fail
        };
        session.record(obs(&commit.0, class)).unwrap();
    }

    let _ = session.outcome();
}

/// Build a fully-observed session and then benchmark only `outcome()`.
fn build_fully_observed(n: usize) -> LocalizationSession {
    let seq = make_sequence(n);
    let policy = SearchPolicy {
        max_probes: n,
        ..SearchPolicy::default()
    };
    let mut session = LocalizationSession::new(seq, policy).unwrap();
    let fault_index = n / 2;

    while let Some(commit) = session.next_probe() {
        let idx: usize = commit.0.strip_prefix("commit-").unwrap().parse().unwrap();
        let class = if idx < fault_index {
            ObservationClass::Pass
        } else {
            ObservationClass::Fail
        };
        session.record(obs(&commit.0, class)).unwrap();
    }
    session
}

fn bench_binary_narrowing_10(c: &mut Criterion) {
    c.bench_function("binary_narrowing_10", |b| b.iter(|| run_narrowing(10)));
}

fn bench_binary_narrowing_100(c: &mut Criterion) {
    c.bench_function("binary_narrowing_100", |b| b.iter(|| run_narrowing(100)));
}

fn bench_binary_narrowing_1000(c: &mut Criterion) {
    c.bench_function("binary_narrowing_1000", |b| b.iter(|| run_narrowing(1000)));
}

fn bench_with_skips(c: &mut Criterion) {
    c.bench_function("binary_narrowing_100_with_skips", |b| {
        b.iter(|| run_narrowing_with_skips(100))
    });
}

fn bench_outcome_computation(c: &mut Criterion) {
    let session = build_fully_observed(100);
    c.bench_function("outcome_computation_100", |b| b.iter(|| session.outcome()));
}

criterion_group!(
    benches,
    bench_binary_narrowing_10,
    bench_binary_narrowing_100,
    bench_binary_narrowing_1000,
    bench_with_skips,
    bench_outcome_computation,
);
criterion_main!(benches);
