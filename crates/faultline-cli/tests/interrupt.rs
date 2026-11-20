#![cfg(target_os = "linux")]

use anyhow::Context;
use faultline_fixtures::{FileOp, GitRepoBuilder};
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::FromRawFd;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn terminal() -> io::Result<(File, File)> {
    let mut master = -1;
    let mut slave = -1;
    // SAFETY: Both descriptors are out parameters. Null optional parameters
    // request the default terminal settings with no window size override.
    let result = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: On success openpty transfers both live descriptors to this caller.
    let files = unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) };
    for fd in [master, slave] {
        // SAFETY: Both descriptors are live and owned by files. F_SETFD takes
        // an integer flags argument.
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(files)
}

fn attach_terminal(command: &mut Command) {
    // SAFETY: Only async-signal-safe system calls are made in the fork child.
    // Descriptor 0 is the private slave of the fixture pseudoterminal.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 || libc::ioctl(0, libc::TIOCSCTTY, 0) == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

fn reap_adopted_fixture_children() -> io::Result<usize> {
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut count = 0;
    while Instant::now() < deadline {
        // SAFETY: This single-test executable adopts only descendants of
        // its own fixture. WNOHANG requests a nonblocking child wait.
        let result = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
        if result > 0 {
            count += 1;
        } else if result == 0 {
            thread::sleep(Duration::from_millis(5));
        } else {
            let err = io::Error::last_os_error();
            match err.raw_os_error() {
                Some(libc::ECHILD) => return Ok(count),
                Some(libc::EINTR) => {}
                _ => return Err(err),
            }
        }
    }
    Err(io::Error::other("fixture descendants did not exit"))
}

// In error paths this guard terminates and reaps only the CLI it started.
// The fixture descendants also have a finite natural lifetime.
struct FixtureCli(Child);

impl Drop for FixtureCli {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _killed = self.0.kill();
            let _status = self.0.wait();
        }
    }
}

fn wait_for_exit(cli: &mut FixtureCli, limit: Duration) -> io::Result<Option<ExitStatus>> {
    let started = Instant::now();
    loop {
        if let Some(status) = cli.0.try_wait()? {
            return Ok(Some(status));
        }
        if started.elapsed() >= limit {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn linux_preserves_terminal_input_and_cancels_noninteractive_groups() -> anyhow::Result<()> {
    // SAFETY: Set adoption in this dedicated fixture executable, so it can
    // reap its own orphaned grandchildren even when running in a container.
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
    let repo = GitRepoBuilder::new()
        .map_err(anyhow::Error::msg)?
        .commit(
            "good",
            vec![FileOp::Write {
                path: "predicate.txt".into(),
                content: "good".into(),
            }],
        )
        .commit(
            "bad",
            vec![FileOp::Write {
                path: "predicate.txt".into(),
                content: "bad".into(),
            }],
        )
        .build()
        .map_err(anyhow::Error::msg)?;
    let good = repo.commits.first().context("fixture good commit")?;
    let bad = repo.commits.last().context("fixture bad commit")?;
    let dir = tempfile::tempdir()?;
    let command_for = |script| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_faultline-rs"));
        command
            .arg("--repo")
            .arg(repo.dir.path())
            .arg("--good")
            .arg(&good.0)
            .arg("--bad")
            .arg(&bad.0)
            .arg("--output-dir")
            .arg(dir.path().join("report"))
            .args(["--timeout-seconds", "3", "--cmd", script])
            .stdout(Stdio::null());
        command
    };

    let ready = dir.path().join("ready");
    let survived = dir.path().join("survived");
    let stderr = dir.path().join("stderr");
    let mut command =
        command_for("sh -c 'sleep 2 & : > \"$READY\"; wait; : > \"$SURVIVED\"' & wait");
    command
        .env("READY", &ready)
        .env("SURVIVED", &survived)
        .stdin(Stdio::null())
        .stderr(Stdio::from(File::create(&stderr)?));
    let mut cli = FixtureCli(command.spawn()?);
    let started = Instant::now();
    while !ready.exists() && started.elapsed() < Duration::from_secs(2) {
        thread::sleep(Duration::from_millis(5));
    }
    let was_ready = ready.exists();
    let signal = if was_ready {
        let pid = libc::pid_t::try_from(cli.0.id())?;
        // SAFETY: Signal only the direct CLI child while its Child handle is
        // still owned and unreaped. No process group or unrelated PID is targeted.
        if unsafe { libc::kill(pid, libc::SIGINT) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    } else {
        Ok(())
    };
    let status = wait_for_exit(&mut cli, Duration::from_secs(3))?;
    drop(cli);
    let reaped = reap_adopted_fixture_children()?;
    signal?;
    assert!(was_ready, "probe never reached its readiness marker");
    assert!(
        !survived.exists(),
        "nested probe survived noninteractive SIGINT"
    );
    assert!(reaped > 0, "fixture should reap adopted descendants");
    assert_eq!(status.and_then(|status| status.code()), Some(130));
    let diagnostic = std::fs::read_to_string(stderr)?;
    assert!(diagnostic.contains("probe cancelled"), "{diagnostic}");

    let received = dir.path().join("received");
    let (mut master, slave) = terminal()?;
    let mut command = command_for(
        "IFS= read -r answer || exit 2; printf '%s\n' \"$answer\" >> \"$RECEIVED\"; grep -qx good predicate.txt",
    );
    command
        .env("RECEIVED", &received)
        .stdin(Stdio::from(slave))
        .stderr(Stdio::null());
    attach_terminal(&mut command);
    let mut cli = FixtureCli(command.spawn()?);
    let input = master.write_all(b"first input\nsecond input\n");
    let status = wait_for_exit(&mut cli, Duration::from_secs(8))?;
    drop(cli);
    drop(master);
    reap_adopted_fixture_children()?;
    input?;
    let actual =
        std::fs::read_to_string(&received).context("predicate did not consume terminal input")?;
    assert_eq!(actual, "first input\nsecond input\n");
    assert!(status.is_some_and(|status| status.success()), "{status:?}");
    let terminal_ready = dir.path().join("terminal-ready");
    let terminal_sigint = dir.path().join("terminal-sigint");
    let (mut master, slave) = terminal()?;
    let mut command = command_for(
        "trap '' HUP; trap ': > \"$INTERRUPTED\"; exit 130' INT; : > \"$READY\"; IFS= read -r answer",
    );
    command
        .arg("--force")
        .env("READY", &terminal_ready)
        .env("INTERRUPTED", &terminal_sigint)
        .stdin(Stdio::from(slave))
        .stderr(Stdio::null());
    attach_terminal(&mut command);
    let mut cli = FixtureCli(command.spawn()?);
    let started = Instant::now();
    while !terminal_ready.exists() && started.elapsed() < Duration::from_secs(2) {
        thread::sleep(Duration::from_millis(5));
    }
    let interrupt = master.write_all(b"\x03");
    let status = wait_for_exit(&mut cli, Duration::from_secs(4))?;
    drop(cli);
    reap_adopted_fixture_children()?;
    interrupt?;
    assert!(
        terminal_ready.exists(),
        "terminal predicate never became ready"
    );
    assert_eq!(
        status.and_then(|status| status.signal()),
        Some(libc::SIGINT)
    );
    assert!(
        terminal_sigint.exists(),
        "foreground predicate did not receive Ctrl-C"
    );
    Ok(())
}
