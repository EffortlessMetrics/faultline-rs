#![cfg(target_os = "linux")]

use faultline_codes::{ObservationClass, ProbeKind};
use faultline_ports::ProbePort;
use faultline_probe_exec::ExecProbeAdapter;
use faultline_types::{CheckedOutRevision, CommitId, ProbeSpec, ShellKind};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

fn reap_fixture_descendants() -> io::Result<usize> {
    let started = Instant::now();
    let mut reaped = 0;
    while started.elapsed() < Duration::from_secs(6) {
        // SAFETY: This integration-test executable only has children from its fixture.
        // WNOHANG avoids waiting in the kernel, and no exit status is requested.
        let result = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
        if result > 0 {
            reaped += 1;
        } else if result == 0 {
            thread::sleep(Duration::from_millis(5));
        } else {
            let err = io::Error::last_os_error();
            match err.raw_os_error() {
                Some(libc::ECHILD) => return Ok(reaped),
                Some(libc::EINTR) => {}
                _ => return Err(err),
            }
        }
    }
    Err(io::Error::other("fixture descendants failed to exit"))
}

#[test]
fn timeout_and_cancellation_reap_owned_nested_probes() -> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: Set a process-local adoption policy in this single-test executable so
    // the test can reap its own orphaned descendants, including in containers.
    let subreaper = unsafe {
        libc::prctl(
            libc::PR_SET_CHILD_SUBREAPER,
            1 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
        )
    };
    if subreaper != 0 {
        return Err(io::Error::last_os_error().into());
    }

    for (cancel, leader_exits) in [(false, false), (false, true), (true, false)] {
        let dir = tempfile::tempdir()?;
        let checkout = CheckedOutRevision {
            commit: CommitId("cleanup-fixture".into()),
            path: dir.path().to_owned(),
        };
        let finish = if leader_exits { "exit 0" } else { "wait" };
        let probe = ProbeSpec::Shell {
            kind: ProbeKind::Custom,
            shell: ShellKind::PosixSh,
            // The nested shell and its sleep both inherit the captured pipes.
            // If cleanup regresses, they still exit naturally after four seconds.
            script: format!(
                "printf 'stdout marker'; printf 'stderr marker' >&2; \
                 sh -c 'sleep 4 & : > ready; wait; : > survived' & {finish}"
            ),
            env: vec![],
            timeout_seconds: if cancel { 10 } else { 1 },
        };

        let started = Instant::now();
        let result = if cancel {
            let cancelled = AtomicBool::new(false);
            thread::scope(|scope| {
                scope.spawn(|| {
                    while !checkout.path.join("ready").exists()
                        && started.elapsed() < Duration::from_secs(2)
                    {
                        thread::sleep(Duration::from_millis(5));
                    }
                    cancelled.store(true, Ordering::Relaxed);
                });
                ExecProbeAdapter.run_cancellable(&checkout, &probe, &cancelled)
            })
        } else {
            ExecProbeAdapter.run(&checkout, &probe)
        };
        let elapsed = started.elapsed();
        let reaped = reap_fixture_descendants()?;
        eprintln!(
            "cancel={cancel} leader_exits={leader_exits} elapsed={elapsed:?} reaped={reaped} error={:?}",
            result.as_ref().err()
        );

        assert!(dir.path().join("ready").exists(), "nested probe ran");
        assert!(
            !dir.path().join("survived").exists(),
            "nested probe survived"
        );
        assert!(reaped > 0, "fixture must exercise descendant reaping");
        assert!(
            elapsed < Duration::from_millis(2500),
            "probe took {elapsed:?}"
        );
        if cancel {
            assert!(matches!(
                result,
                Err(faultline_types::FaultlineError::Probe(message)) if message == "probe cancelled"
            ));
        } else {
            let observation = result?;
            assert!(observation.timed_out);
            assert_eq!(observation.class, ObservationClass::Indeterminate);
            assert_eq!(observation.stdout, "stdout marker");
            assert_eq!(observation.stderr, "stderr marker");
            if leader_exits {
                assert_eq!(observation.exit_code, Some(0));
                assert_eq!(observation.signal_number, None);
            } else {
                assert_eq!(observation.exit_code, None);
                assert_eq!(observation.signal_number, Some(libc::SIGKILL));
            }
        }
    }
    let dir = tempfile::tempdir()?;
    let checkout = CheckedOutRevision {
        commit: CommitId("quiet-fixture".into()),
        path: dir.path().to_owned(),
    };
    let probe = ProbeSpec::Shell {
        kind: ProbeKind::Custom,
        shell: ShellKind::PosixSh,
        script: "sleep 0.75; : > completed; exit 7".into(),
        env: vec![],
        timeout_seconds: 1,
    };
    let cancelled = AtomicBool::new(true);
    assert!(matches!(
        ExecProbeAdapter.run_cancellable(&checkout, &probe, &cancelled),
        Err(faultline_types::FaultlineError::Probe(message)) if message == "probe cancelled before launch"
    ));
    assert_eq!(reap_fixture_descendants()?, 0);
    assert!(!dir.path().join("completed").exists());

    let observation = ExecProbeAdapter.run(&checkout, &probe)?;
    assert_eq!(observation.class, ObservationClass::Fail);
    assert_eq!(observation.exit_code, Some(7));
    assert_eq!(observation.signal_number, None);
    assert!(!observation.timed_out);
    assert!(dir.path().join("completed").exists());
    assert_eq!(reap_fixture_descendants()?, 0);
    Ok(())
}
