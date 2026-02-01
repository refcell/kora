use std::fmt;

/// Error type for node runner operations.
#[derive(Debug)]
pub struct RunnerError(pub anyhow::Error);

impl fmt::Display for RunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for RunnerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl From<anyhow::Error> for RunnerError {
    fn from(e: anyhow::Error) -> Self {
        Self(e)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn runner_error_display_shows_inner_message() {
        let inner = anyhow::anyhow!("test error message");
        let error = RunnerError(inner);
        assert_eq!(format!("{error}"), "test error message");
    }

    #[test]
    fn runner_error_debug_contains_runner_error() {
        let inner = anyhow::anyhow!("debug test");
        let error = RunnerError(inner);
        let debug_str = format!("{error:?}");
        assert!(debug_str.contains("RunnerError"));
    }

    #[test]
    fn runner_error_from_anyhow_preserves_message() {
        let inner = anyhow::anyhow!("original message");
        let error: RunnerError = inner.into();
        assert_eq!(format!("{error}"), "original message");
    }

    #[test]
    fn runner_error_source_returns_none_for_simple_error() {
        let inner = anyhow::anyhow!("simple error");
        let error = RunnerError(inner);
        assert!(error.source().is_none());
    }

    #[test]
    fn runner_error_source_delegates_to_anyhow() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let inner = anyhow::Error::from(io_err);
        let inner_source_is_some = inner.source().is_some();
        let error = RunnerError(inner);
        assert_eq!(error.source().is_some(), inner_source_is_some);
    }
}
