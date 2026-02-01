/// Best-effort handler for raising the process file descriptor limit.
#[derive(Clone, Copy, Debug, Default)]
pub struct FileLimitHandler;

impl FileLimitHandler {
    /// Create a new file limit handler.
    pub const fn new() -> Self {
        Self
    }

    /// Raise the file descriptor limit to the system maximum when available.
    pub fn raise(&self) {
        raise_open_file_limit();
    }
}

#[cfg(unix)]
fn raise_open_file_limit() {
    use libc::{RLIMIT_NOFILE, getrlimit, rlimit, setrlimit};

    // Best effort: avoid hitting low per-process fd limits during simulations.
    // SAFETY: getrlimit and setrlimit are called with valid pointers to properly initialized
    // rlimit structures. These are standard POSIX functions with no preconditions beyond valid
    // pointers. Adjusting resource limits has no global side effects when called at startup.
    unsafe {
        let mut limits = rlimit { rlim_cur: 0, rlim_max: 0 };
        if getrlimit(RLIMIT_NOFILE, &mut limits) != 0 {
            return;
        }
        if limits.rlim_cur < limits.rlim_max {
            let updated = rlimit { rlim_cur: limits.rlim_max, rlim_max: limits.rlim_max };
            let _ = setrlimit(RLIMIT_NOFILE, &updated);
        }
    }
}

#[cfg(not(unix))]
fn raise_open_file_limit() {}
