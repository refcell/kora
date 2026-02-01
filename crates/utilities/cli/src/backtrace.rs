//! Minimal helper utility to set the backtrace environment variable if not set.

/// The backtracing utility.
#[derive(Debug, Clone, Copy)]
pub struct Backtracing;

impl Backtracing {
    /// Sets the RUST_BACKTRACE environment variable to 1 if it is not already set.
    pub fn enable() {
        // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
        if std::env::var_os("RUST_BACKTRACE").is_none() {
            // SAFETY: Setting environment variables is safe when called at program startup
            // before any threads are spawned. No other threads can be accessing the environment
            // at this point, and set_var has no other unsafe preconditions.
            unsafe { std::env::set_var("RUST_BACKTRACE", "1") };
        }
    }
}
