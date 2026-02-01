//! Partition state machine for managing QMDB database lifecycle.

use crate::error::BackendError;

/// State of a partition database.
#[derive(Debug)]
pub enum PartitionState<D> {
    /// Database not yet initialized.
    Uninitialized,
    /// Database is ready for operations.
    Ready(D),
    /// Database has been closed.
    Closed,
}

impl<D> PartitionState<D> {
    /// Create a new uninitialized partition state.
    pub const fn new() -> Self {
        Self::Uninitialized
    }

    /// Initialize the partition with a database.
    pub fn initialize(&mut self, db: D) -> Result<(), BackendError> {
        match self {
            Self::Uninitialized => {
                *self = Self::Ready(db);
                Ok(())
            }
            Self::Ready(_) => Err(BackendError::Partition("already initialized".to_string())),
            Self::Closed => Err(BackendError::Partition("partition closed".to_string())),
        }
    }

    /// Get a reference to the database if ready.
    pub fn get(&self) -> Result<&D, BackendError> {
        match self {
            Self::Ready(db) => Ok(db),
            Self::Uninitialized => Err(BackendError::NotInitialized),
            Self::Closed => Err(BackendError::Partition("partition closed".to_string())),
        }
    }

    /// Get a mutable reference to the database if ready.
    pub fn get_mut(&mut self) -> Result<&mut D, BackendError> {
        match self {
            Self::Ready(db) => Ok(db),
            Self::Uninitialized => Err(BackendError::NotInitialized),
            Self::Closed => Err(BackendError::Partition("partition closed".to_string())),
        }
    }

    /// Close the partition and return the database.
    pub fn close(&mut self) -> Result<D, BackendError> {
        match std::mem::replace(self, Self::Closed) {
            Self::Ready(db) => Ok(db),
            Self::Uninitialized => Err(BackendError::NotInitialized),
            Self::Closed => Err(BackendError::Partition("already closed".to_string())),
        }
    }

    /// Check if the partition is ready.
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }
}

impl<D> Default for PartitionState<D> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_returns_uninitialized() {
        let state: PartitionState<i32> = PartitionState::new();
        assert!(!state.is_ready());
    }

    #[test]
    fn test_default_returns_uninitialized() {
        let state: PartitionState<i32> = PartitionState::default();
        assert!(!state.is_ready());
    }

    #[test]
    fn test_initialize_from_uninitialized_succeeds() {
        let mut state: PartitionState<i32> = PartitionState::new();
        assert!(state.initialize(42).is_ok());
        assert!(state.is_ready());
    }

    #[test]
    fn test_initialize_from_ready_fails() {
        let mut state: PartitionState<i32> = PartitionState::new();
        state.initialize(42).unwrap();
        assert!(state.initialize(100).is_err());
    }

    #[test]
    fn test_initialize_from_closed_fails() {
        let mut state: PartitionState<i32> = PartitionState::new();
        state.initialize(42).unwrap();
        state.close().unwrap();
        assert!(state.initialize(100).is_err());
    }

    #[test]
    fn test_get_from_ready_succeeds() {
        let mut state: PartitionState<i32> = PartitionState::new();
        state.initialize(42).unwrap();
        assert_eq!(*state.get().unwrap(), 42);
    }

    #[test]
    fn test_get_from_uninitialized_fails() {
        let state: PartitionState<i32> = PartitionState::new();
        assert!(state.get().is_err());
    }

    #[test]
    fn test_get_from_closed_fails() {
        let mut state: PartitionState<i32> = PartitionState::new();
        state.initialize(42).unwrap();
        state.close().unwrap();
        assert!(state.get().is_err());
    }

    #[test]
    fn test_get_mut_from_ready_succeeds() {
        let mut state: PartitionState<i32> = PartitionState::new();
        state.initialize(42).unwrap();
        *state.get_mut().unwrap() = 100;
        assert_eq!(*state.get().unwrap(), 100);
    }

    #[test]
    fn test_get_mut_from_uninitialized_fails() {
        let mut state: PartitionState<i32> = PartitionState::new();
        assert!(state.get_mut().is_err());
    }

    #[test]
    fn test_get_mut_from_closed_fails() {
        let mut state: PartitionState<i32> = PartitionState::new();
        state.initialize(42).unwrap();
        state.close().unwrap();
        assert!(state.get_mut().is_err());
    }

    #[test]
    fn test_close_from_ready_succeeds() {
        let mut state: PartitionState<i32> = PartitionState::new();
        state.initialize(42).unwrap();
        assert_eq!(state.close().unwrap(), 42);
        assert!(!state.is_ready());
    }

    #[test]
    fn test_close_from_uninitialized_fails() {
        let mut state: PartitionState<i32> = PartitionState::new();
        assert!(state.close().is_err());
    }

    #[test]
    fn test_close_twice_fails() {
        let mut state: PartitionState<i32> = PartitionState::new();
        state.initialize(42).unwrap();
        state.close().unwrap();
        assert!(state.close().is_err());
    }

    #[test]
    fn test_is_ready_reflects_state() {
        let mut state: PartitionState<i32> = PartitionState::new();
        assert!(!state.is_ready());
        state.initialize(42).unwrap();
        assert!(state.is_ready());
        state.close().unwrap();
        assert!(!state.is_ready());
    }

    #[test]
    fn test_debug_impl() {
        let state: PartitionState<i32> = PartitionState::new();
        let debug = format!("{:?}", state);
        assert!(debug.contains("Uninitialized"));
    }
}
