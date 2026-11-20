use super::CapturedProbe;
use faultline_types::{FaultlineError, Result};
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus};
use std::thread;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(5);
const SHUTDOWN_LIMIT: Duration = Duration::from_secs(1);

struct Pipe<R> {
    reader: R,
    bytes: Vec<u8>,
    eof: bool,
}

impl<R: AsRawFd + Read> Pipe<R> {
    fn new(reader: R) -> io::Result<Self> {
        let fd = reader.as_raw_fd();
        // SAFETY: The reader owns a live pipe descriptor. F_GETFL takes no extra argument.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: The descriptor is still live. F_SETFL takes an integer flags argument.
        if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            reader,
            bytes: Vec::new(),
            eof: false,
        })
    }

    fn poll_fd(&self) -> libc::pollfd {
        libc::pollfd {
            fd: if self.eof {
                -1
            } else {
                self.reader.as_raw_fd()
            },
            events: libc::POLLIN,
            revents: 0,
        }
    }

    fn drain_chunk(&mut self) -> io::Result<bool> {
        if self.eof {
            return Ok(false);
        }
        // Read one chunk per pipe so active writers cannot starve each other or the deadline.
        let mut buffer = [0; 16 * 1024];
        match self.reader.read(&mut buffer) {
            Ok(0) => self.eof = true,
            Ok(size) => {
                self.bytes.extend(buffer.into_iter().take(size));
                return Ok(true);
            }
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(err) => return Err(err),
        }
        Ok(false)
    }
}

struct Pipes {
    stdout: Pipe<ChildStdout>,
    stderr: Pipe<ChildStderr>,
}

impl Pipes {
    fn new(child: &mut Child) -> io::Result<Self> {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("probe stdout pipe missing"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("probe stderr pipe missing"))?;
        Ok(Self {
            stdout: Pipe::new(stdout)?,
            stderr: Pipe::new(stderr)?,
        })
    }

    fn drain_chunk(&mut self) -> io::Result<bool> {
        let stdout_progress = self.stdout.drain_chunk()?;
        let stderr_progress = self.stderr.drain_chunk()?;
        Ok(stdout_progress || stderr_progress)
    }

    fn wait_for_output(&self, remaining: Duration) -> io::Result<()> {
        let interval = remaining.min(POLL_INTERVAL);
        let mut fds = [self.stdout.poll_fd(), self.stderr.poll_fd()];
        let millis = i32::try_from(interval.as_millis()).unwrap_or(0);
        // SAFETY: The array contains two initialized descriptors and remains live
        // throughout poll. Negative descriptors are ignored.
        if unsafe { libc::poll(fds.as_mut_ptr(), 2, millis) } < 0 {
            let err = io::Error::last_os_error();
            if err.kind() != io::ErrorKind::Interrupted {
                return Err(err);
            }
        }
        Ok(())
    }

    fn finished(&self) -> bool {
        self.stdout.eof && self.stderr.eof
    }
}

enum Outcome {
    Exited(ExitStatus),
    TimedOut,
    Cancelled,
}

fn observe(
    child: &mut Child,
    pipes: &mut Pipes,
    started: Instant,
    timeout: Duration,
    cancelled: impl Fn() -> bool,
) -> io::Result<Outcome> {
    loop {
        let progress = pipes.drain_chunk()?;
        // Keep the group leader unreaped while the pipes are open. Even if it exits,
        // its PID continues to identify this group until we signal it or finish.
        if pipes.finished()
            && let Some(status) = child.try_wait()?
        {
            return Ok(Outcome::Exited(status));
        }
        if cancelled() {
            return Ok(Outcome::Cancelled);
        }
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Ok(Outcome::TimedOut);
        }
        if !progress {
            pipes.wait_for_output(remaining)?;
        }
    }
}

fn kill_group(child: &Child) -> io::Result<()> {
    let leader = libc::pid_t::try_from(child.id())
        .ok()
        .filter(|pid| *pid > 1)
        .ok_or_else(|| io::Error::other("invalid probe group leader PID"))?;
    // SAFETY: process_group(0) created this group. We haven't reaped its leader,
    // so its PID can't be recycled in this runner. Values below 2 are rejected
    // because kill(0, ...) and kill(-1, ...) would signal outside the probe group.
    if unsafe { libc::kill(-leader, libc::SIGKILL) } == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        // The leader and any remaining group members have already exited.
        return Ok(());
    }
    Err(err)
}

fn shutdown(
    child: &mut Child,
    mut pipes: Option<&mut Pipes>,
    terminate_group: impl FnOnce(&Child) -> io::Result<()>,
) -> io::Result<ExitStatus> {
    let started = Instant::now();
    let mut failure = terminate_group(child).err().map(|err| {
        // Still try to reap the immediate child when the group signal fails.
        let fallback = child.kill();
        match fallback {
            Ok(()) => io::Error::other(format!("group termination failed: {err}")),
            Err(direct) => io::Error::other(format!(
                "group termination failed ({err}); direct termination failed ({direct})"
            )),
        }
    });

    let mut status = None;
    loop {
        if status.is_none() {
            status = child.try_wait()?;
        }
        let mut progress = false;
        if let Some(active) = pipes.as_mut() {
            match active.drain_chunk() {
                Ok(drained) => progress = drained,
                Err(err) => {
                    failure.get_or_insert(err);
                    pipes = None;
                }
            }
        }
        if let Some(status) = status
            && pipes.as_ref().is_none_or(|pipes| pipes.finished())
        {
            return failure.map_or(Ok(status), Err);
        }
        let remaining = SHUTDOWN_LIMIT.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(io::Error::other(format!(
                "shutdown deadline elapsed (child reaped: {}, output drained: {}, error: {failure:?})",
                status.is_some(),
                pipes.as_ref().is_none_or(|pipes| pipes.finished())
            )));
        }
        if !progress {
            if let Some(active) = pipes.as_ref() {
                if let Err(err) = active.wait_for_output(remaining) {
                    failure.get_or_insert(err);
                    pipes = None;
                }
            } else {
                thread::sleep(POLL_INTERVAL.min(remaining));
            }
        }
    }
}

pub(super) fn capture(
    command: &mut Command,
    started: Instant,
    timeout: Duration,
    cancelled: impl Fn() -> bool,
) -> Result<CapturedProbe> {
    if cancelled() {
        return Err(FaultlineError::Probe(
            "probe cancelled before launch".into(),
        ));
    }
    let mut child = command
        .process_group(0)
        .spawn()
        .map_err(|err| FaultlineError::Probe(err.to_string()))?;

    let mut pipes = match Pipes::new(&mut child) {
        Ok(pipes) => pipes,
        Err(err) => {
            cleanup(&mut child, None, "output setup failure")?;
            return Err(FaultlineError::Probe(format!(
                "probe output setup failed: {err}"
            )));
        }
    };
    let (status, timed_out) = match observe(&mut child, &mut pipes, started, timeout, cancelled) {
        Ok(Outcome::Exited(status)) => (status, false),
        Ok(Outcome::TimedOut) => (cleanup(&mut child, Some(&mut pipes), "timeout")?, true),
        Ok(Outcome::Cancelled) => {
            cleanup(&mut child, Some(&mut pipes), "cancellation")?;
            return Err(FaultlineError::Probe("probe cancelled".into()));
        }
        Err(err) if err.raw_os_error() == Some(libc::ECHILD) => {
            // A foreign child reaper invalidates the leader PID as an ownership anchor.
            return Err(FaultlineError::Probe(format!(
                "incomplete probe cleanup: lost child ownership: {err}"
            )));
        }
        Err(err) => {
            cleanup(&mut child, None, "observation failure")?;
            return Err(FaultlineError::Probe(format!(
                "probe observation failed: {err}"
            )));
        }
    };
    Ok(CapturedProbe {
        status,
        timed_out,
        stdout: pipes.stdout.bytes,
        stderr: pipes.stderr.bytes,
    })
}

fn cleanup(child: &mut Child, pipes: Option<&mut Pipes>, reason: &str) -> Result<ExitStatus> {
    shutdown(child, pipes, kill_group).map_err(|err| {
        FaultlineError::Probe(format!("incomplete probe cleanup after {reason}: {err}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    #[test]
    fn shutdown_reports_failed_group_signal() -> io::Result<()> {
        let mut child = Command::new("sleep").arg("2").process_group(0).spawn()?;
        let result = shutdown(&mut child, None, |_| {
            Err(io::Error::from_raw_os_error(libc::EPERM))
        });
        assert!(matches!(
            result,
            Err(err) if err.to_string().contains("group termination failed")
        ));
        let status = child
            .try_wait()?
            .ok_or_else(|| io::Error::other("direct child was not reaped"))?;
        assert_eq!(status.signal(), Some(libc::SIGKILL));
        Ok(())
    }
}
