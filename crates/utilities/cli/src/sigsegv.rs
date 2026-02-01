//! Signal handler to extract a backtrace from stack overflow.
//!
//! Implementation modified from [reth](https://github.com/paradigmxyz/reth/blob/main/crates/cli/util/src/sigsegv_handler.rs).
//!
//! Implementation modified from [`rustc`](https://github.com/rust-lang/rust/blob/3dee9775a8c94e701a08f7b2df2c444f353d8699/compiler/rustc_driver_impl/src/signal_handler.rs).

use std::{
    alloc::{Layout, alloc},
    fmt, mem, ptr,
};

/// The SIGSEGV handler.
#[derive(Debug, Clone, Copy)]
pub struct SigsegvHandler;

impl SigsegvHandler {
    /// Installs a SIGSEGV handler.
    ///
    /// When SIGSEGV is delivered to the process, print a stack trace and then exit.
    pub fn install() {
        // SAFETY: Setting up a signal handler is safe because:
        // 1. The alternate stack is properly allocated with correct size and alignment
        // 2. sigaltstack() is called before sigemptyset() and sigaction()
        // 3. The signal handler (print_stack_trace) only performs async-signal-safe operations
        // 4. SA_NODEFER and SA_RESETHAND flags prevent recursive signal delivery
        // 5. This is called at program startup before any threads are created
        unsafe {
            let alt_stack_size: usize = min_sigstack_size() + 64 * 1024;
            let mut alt_stack: libc::stack_t = mem::zeroed();
            alt_stack.ss_sp = alloc(Layout::from_size_align(alt_stack_size, 1).unwrap()).cast();
            alt_stack.ss_size = alt_stack_size;
            libc::sigaltstack(&alt_stack, ptr::null_mut());

            let mut sa: libc::sigaction = mem::zeroed();
            sa.sa_sigaction = print_stack_trace as *const () as libc::sighandler_t;
            sa.sa_flags = libc::SA_NODEFER | libc::SA_RESETHAND | libc::SA_ONSTACK;
            libc::sigemptyset(&mut sa.sa_mask);
            libc::sigaction(libc::SIGSEGV, &sa, ptr::null_mut());
        }
    }
}

unsafe extern "C" {
    fn backtrace_symbols_fd(buffer: *const *mut libc::c_void, size: libc::c_int, fd: libc::c_int);
}

fn backtrace_stderr(buffer: &[*mut libc::c_void]) {
    let size = buffer.len().try_into().unwrap_or_default();
    // SAFETY: backtrace_symbols_fd is called with a valid pointer to an array of backtrace
    // symbols obtained from libc::backtrace(), a valid size, and STDERR_FILENO which is
    // always a valid file descriptor. Writing to stderr is safe at any time.
    unsafe { backtrace_symbols_fd(buffer.as_ptr(), size, libc::STDERR_FILENO) };
}

/// Unbuffered, unsynchronized writer to stderr.
///
/// Only acceptable because everything will end soon anyways.
struct RawStderr(());

impl fmt::Write for RawStderr {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        // SAFETY: libc::write is called with STDERR_FILENO (always valid), a valid string pointer
        // from s.as_ptr() with correct length s.len(). Writing to stderr does not violate any
        // invariants and is safe even in a signal handler context.
        let ret = unsafe { libc::write(libc::STDERR_FILENO, s.as_ptr().cast(), s.len()) };
        if ret == -1 { Err(fmt::Error) } else { Ok(()) }
    }
}

/// We don't really care how many bytes we actually get out. SIGSEGV comes for our head.
/// Splash stderr with letters of our own blood to warn our friends about the monster.
macro_rules! raw_errln {
    ($tokens:tt) => {
        let _ = ::core::fmt::Write::write_fmt(&mut RawStderr(()), format_args!($tokens));
        let _ = ::core::fmt::Write::write_char(&mut RawStderr(()), '\n');
    };
}

/// Signal handler installed for SIGSEGV
extern "C" fn print_stack_trace(_: libc::c_int) {
    const MAX_FRAMES: usize = 256;
    let mut stack_trace: [*mut libc::c_void; MAX_FRAMES] = [ptr::null_mut(); MAX_FRAMES];
    // SAFETY: libc::backtrace writes to a valid mutable array of MAX_FRAMES pointers.
    // The returned depth is guaranteed to be <= MAX_FRAMES, so slicing with [0..depth as usize]
    // is safe. backtrace() is async-signal-safe and can be called from a signal handler.
    let stack = unsafe {
        // Collect return addresses
        let depth = libc::backtrace(stack_trace.as_mut_ptr(), MAX_FRAMES as i32);
        if depth == 0 {
            return;
        }
        &stack_trace[0..depth as usize]
    };

    // Just a stack trace is cryptic. Explain what we're doing.
    raw_errln!("error: kora interrupted by SIGSEGV, printing backtrace\n");
    let mut written = 1;
    let mut consumed = 0;
    // Begin elaborating return addrs into symbols and writing them directly to stderr
    // Most backtraces are stack overflow, most stack overflows are from recursion
    // Check for cycles before writing 250 lines of the same ~5 symbols
    let cycled = |(runner, walker)| runner == walker;
    let mut cyclic = false;
    if let Some(period) = stack.iter().skip(1).step_by(2).zip(stack).position(cycled) {
        let period = period.saturating_add(1); // avoid "what if wrapped?" branches
        let Some(offset) = stack.iter().skip(period).zip(stack).position(cycled) else {
            // impossible.
            return;
        };

        // Count matching trace slices, else we could miscount "biphasic cycles"
        // with the same period + loop entry but a different inner loop
        let next_cycle = stack[offset..].chunks_exact(period).skip(1);
        let cycles = 1 + next_cycle
            .zip(stack[offset..].chunks_exact(period))
            .filter(|(next, prev)| next == prev)
            .count();
        backtrace_stderr(&stack[..offset]);
        written += offset;
        consumed += offset;
        if cycles > 1 {
            raw_errln!("\n### cycle encountered after {offset} frames with period {period}");
            backtrace_stderr(&stack[consumed..consumed + period]);
            raw_errln!("### recursed {cycles} times\n");
            written += period + 4;
            consumed += period * cycles;
            cyclic = true;
        };
    }
    let rem = &stack[consumed..];
    backtrace_stderr(rem);
    raw_errln!("");
    written += rem.len() + 1;

    let random_depth = || 8 * 16; // chosen by random diceroll (2d20)
    if cyclic || stack.len() > random_depth() {
        // technically speculation, but assert it with confidence anyway.
        // We only arrived in this signal handler because bad things happened
        // and this message is for explaining it's not the programmer's fault
        raw_errln!("note: kora unexpectedly overflowed its stack! this is a bug");
        written += 1;
    }
    if stack.len() == MAX_FRAMES {
        raw_errln!("note: maximum backtrace depth reached, frames may have been lost");
        written += 1;
    }
    raw_errln!("note: we would appreciate a report at https://github.com/refcell/kora");
    written += 1;
    if written > 24 {
        // We probably just scrolled the earlier "we got SIGSEGV" message off the terminal
        raw_errln!("note: backtrace dumped due to SIGSEGV! resuming signal");
    }
}

/// Modern kernels on modern hardware can have dynamic signal stack sizes.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn min_sigstack_size() -> usize {
    const AT_MINSIGSTKSZ: core::ffi::c_ulong = 51;
    // SAFETY: getauxval is a safe way to query auxiliary vector entries. It returns 0 if the
    // entry is not found, which is the correct behavior. No unsafe preconditions are violated.
    // getauxval does not access memory based on user input and is async-signal-safe.
    let dynamic_sigstksz = unsafe { libc::getauxval(AT_MINSIGSTKSZ) };
    // If getauxval couldn't find the entry, it returns 0,
    // so take the higher of the "constant" and auxval.
    // This transparently supports older kernels which don't provide AT_MINSIGSTKSZ
    libc::MINSIGSTKSZ.max(dynamic_sigstksz as _)
}

/// Not all OS support hardware where this is needed.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const fn min_sigstack_size() -> usize {
    libc::MINSIGSTKSZ
}
