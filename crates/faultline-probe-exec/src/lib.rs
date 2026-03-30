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
            program,
            args,
            env,
            ..
        } => {
            let mut cmd = Command::new(program);
            cmd.args(args);
            for (key, value) in env {
                cmd.env(key, value);
            }
            cmd
        }
        ProbeSpec::Shell {
            shell, script, ..
        } => {
            let (program, args): (&str, Vec<&str>) = match shell {
                ShellKind::Default => {
                    if cfg!(windows) {
                        ("cmd", vec!["/C", script.as_str()])
                    } else {
                        ("sh", vec!["-lc", script.as_str()])
                    }
                }
                ShellKind::PosixSh => ("sh", vec!["-lc", script.as_str()]),
                ShellKind::Cmd => ("cmd", vec!["/C", script.as_str()]),
                ShellKind::PowerShell => ("powershell", vec!["-Command", script.as_str()]),
            };
            let mut cmd = Command::new(program);
            cmd.args(args);
            cmd
        }
    }
}

fn classify(exit_code: Option<i32>, timed_out: bool) -> ObservationClass {
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
