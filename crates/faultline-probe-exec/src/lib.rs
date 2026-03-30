use faultline_codes::ObservationClass;
use faultline_ports::ProbePort;
use faultline_types::{
    CheckedOutRevision, FaultlineError, ProbeObservation, ProbeSpec, Result, ShellKind,
};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Default, Clone)]
pub struct ExecProbeAdapter;

impl ProbePort for ExecProbeAdapter {
    fn run(&self, checkout: &CheckedOutRevision, probe: &ProbeSpec) -> Result<ProbeObservation> {
        let kind = probe.kind();
        let timeout = Duration::from_secs(probe.timeout_seconds());
        let mut command = build_command(probe);
        command
            .current_dir(&checkout.path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let started = Instant::now();
        let mut child = command
            .spawn()
            .map_err(|err| FaultlineError::Probe(err.to_string()))?;

        let mut timed_out = false;
        loop {
            if child
                .try_wait()
                .map_err(|err| FaultlineError::Probe(err.to_string()))?
                .is_some()
            {
                break;
            }
            if started.elapsed() >= timeout {
                timed_out = true;
                let _ = child.kill();
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        let output = child
            .wait_with_output()
            .map_err(|err| FaultlineError::Probe(err.to_string()))?;
        let exit_code = output.status.code();
        let class = classify(exit_code, timed_out);

        Ok(ProbeObservation {
            commit: checkout.commit.clone(),
            class,
            kind,
            exit_code,
            timed_out,
            duration_ms: started.elapsed().as_millis() as u64,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

fn build_command(probe: &ProbeSpec) -> Command {
    match probe {
        ProbeSpec::Exec {
            program, args, env, ..
        } => {
            let mut cmd = Command::new(program);
            cmd.args(args);
            for (key, value) in env {
                cmd.env(key, value);
            }
            cmd
        }
        ProbeSpec::Shell { shell, script, .. } => {
            let (program, args): (&str, Vec<&str>) = match shell {
                ShellKind::Default => {
                    if cfg!(windows) {
                        ("cmd", vec!["/C", script.as_str()])
                    } else {
                        ("sh", vec!["-c", script.as_str()])
                    }
                }
                ShellKind::PosixSh => ("sh", vec!["-c", script.as_str()]),
                ShellKind::Cmd => ("cmd", vec!["/C", script.as_str()]),
                ShellKind::PowerShell => ("powershell", vec!["-Command", script.as_str()]),
            };
            let mut cmd = Command::new(program);
            cmd.args(args);
            cmd
        }
    }
}

pub fn classify(exit_code: Option<i32>, timed_out: bool) -> ObservationClass {
    if timed_out {
        ObservationClass::Indeterminate
    } else {
        match exit_code {
            Some(0) => ObservationClass::Pass,
            Some(125) => ObservationClass::Skip,
            Some(_) => ObservationClass::Fail,
            None => ObservationClass::Indeterminate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use faultline_codes::{ObservationClass, ProbeKind};
    use faultline_types::{CheckedOutRevision, CommitId, ShellKind};
    use proptest::prelude::*;

    // Feature: v01-release-train, Property 1: Exit Code Classification
    // **Validates: Requirements 2.3, 2.4, 2.5, 2.6**
    fn exit_code_strategy() -> impl Strategy<Value = Option<i32>> {
        prop_oneof![Just(None), any::<i32>().prop_map(Some),]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_exit_code_classification(exit_code in exit_code_strategy(), timed_out in any::<bool>()) {
            let result = classify(exit_code, timed_out);

            let expected = if timed_out {
                ObservationClass::Indeterminate
            } else {
                match exit_code {
                    Some(0) => ObservationClass::Pass,
                    Some(125) => ObservationClass::Skip,
                    Some(_) => ObservationClass::Fail,
                    None => ObservationClass::Indeterminate,
                }
            };

            prop_assert_eq!(result, expected,
                "classify({:?}, {}) returned {:?}, expected {:?}",
                exit_code, timed_out, result, expected
            );
        }
    }

    // Feature: v01-release-train, Property 2: Observation Structural Completeness
    // **Validates: Requirements 2.7, 4.7**

    fn arb_commit_id() -> impl Strategy<Value = CommitId> {
        "[a-f0-9]{8,40}".prop_map(CommitId)
    }

    fn arb_probe_kind() -> impl Strategy<Value = ProbeKind> {
        prop_oneof![
            Just(ProbeKind::Build),
            Just(ProbeKind::Test),
            Just(ProbeKind::Lint),
            Just(ProbeKind::PerfThreshold),
            Just(ProbeKind::Custom),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_observation_structural_completeness(
            commit_id in arb_commit_id(),
            kind in arb_probe_kind(),
        ) {
            let tmp_dir = tempfile::tempdir().expect("failed to create temp dir");

            let probe = ProbeSpec::Shell {
                kind,
                shell: ShellKind::Default,
                script: "echo hello".to_string(),
                timeout_seconds: 30,
            };

            let checkout = CheckedOutRevision {
                commit: commit_id.clone(),
                path: tmp_dir.path().to_path_buf(),
            };

            let adapter = ExecProbeAdapter;
            let obs = adapter.run(&checkout, &probe).expect("probe should succeed");

            // commit is non-empty
            prop_assert!(!obs.commit.0.is_empty(), "commit must be non-empty");
            // commit matches the input
            prop_assert_eq!(&obs.commit, &commit_id, "commit must match input");

            // exit_code is Some (process exited normally, no timeout)
            prop_assert!(obs.exit_code.is_some(), "exit_code must be Some for a normally exiting process");

            // timed_out is false (echo hello completes well within 30s)
            prop_assert!(!obs.timed_out, "timed_out must be false for a fast command");

            // duration_ms is non-negative (always true for u64, but verify it was set)
            // duration_ms >= 0 is trivially true for u64

            // stdout and stderr are captured strings
            // stdout should contain "hello" from echo
            prop_assert!(obs.stdout.contains("hello"), "stdout must capture echo output, got: {:?}", obs.stdout);
            // stderr is a String (may be empty, that's fine)

            // class matches the exit code classification
            let expected_class = classify(obs.exit_code, obs.timed_out);
            prop_assert_eq!(obs.class, expected_class,
                "class must match classify(exit_code={:?}, timed_out={})",
                obs.exit_code, obs.timed_out
            );

            // kind matches the input probe kind
            prop_assert_eq!(obs.kind, kind, "kind must match the probe spec kind");
        }
    }
}
