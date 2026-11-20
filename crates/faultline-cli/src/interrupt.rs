use std::io;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, Ordering};

static CANCELLED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_sigint(_: libc::c_int) {
    CANCELLED.store(true, Ordering::Relaxed);
}

/// Install the CLI's handler only while localization owns a cancellable probe.
pub(super) struct Interrupt(libc::sigaction);

impl Interrupt {
    pub(super) fn install() -> io::Result<Self> {
        CANCELLED.store(false, Ordering::Relaxed);
        // SAFETY: Zero is valid for the POSIX action fields on Linux. The kernel
        // initializes the saved action when sigaction succeeds.
        let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
        action.sa_sigaction = on_sigint as *const () as usize;
        let mut previous = MaybeUninit::uninit();
        // SAFETY: Both pointers refer to storage for a sigaction. The handler
        // only writes a lock-free atomic flag and does not call into the CLI.
        if unsafe { libc::sigaction(libc::SIGINT, &action, previous.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: Successful sigaction initialized the previous action.
        Ok(Self(unsafe { previous.assume_init() }))
    }

    pub(super) fn flag(&self) -> &'static AtomicBool {
        &CANCELLED
    }

    pub(super) fn received(&self) -> bool {
        self.flag().load(Ordering::Relaxed)
    }
}

impl Drop for Interrupt {
    fn drop(&mut self) {
        // SAFETY: self.0 is the valid action saved by install. The old action
        // output is optional and is not needed when restoring.
        let _restored = unsafe { libc::sigaction(libc::SIGINT, &self.0, std::ptr::null_mut()) };
    }
}
