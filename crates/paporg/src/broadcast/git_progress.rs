//! Git progress broadcaster for real-time git operation streaming.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use crate::gitops::progress::{GitOperationType, GitProgressEvent, OperationProgress};

/// Broadcasts git operation progress events for streaming.
/// Also tracks active operations so they can be cancelled.
#[derive(Clone)]
pub struct GitProgressBroadcaster {
    sender: Arc<broadcast::Sender<GitProgressEvent>>,
    /// Registry of active operations for cancellation support.
    active_operations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl GitProgressBroadcaster {
    /// Creates a new git progress broadcaster with the specified channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender: Arc::new(sender),
            active_operations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Sends a progress event to all subscribers.
    pub fn send(&self, event: GitProgressEvent) {
        // Ignore errors - no active receivers is fine
        let _ = self.sender.send(event);
    }

    /// Creates a new subscriber for progress events.
    pub fn subscribe(&self) -> broadcast::Receiver<GitProgressEvent> {
        self.sender.subscribe()
    }

    /// Creates a new operation progress tracker for a git operation.
    /// The operation is registered so it can be cancelled via `cancel_operation`.
    pub fn start_operation(&self, operation_type: GitOperationType) -> OperationProgress {
        let progress = OperationProgress::new(operation_type, Arc::clone(&self.sender));
        let op_id = progress.operation_id().to_string();
        let token = progress.cancellation_token();
        if let Ok(mut ops) = self.active_operations.lock() {
            ops.insert(op_id, token);
        }
        progress
    }

    /// Cancels an active operation by its ID.
    /// Returns true if the operation was found and cancelled.
    pub fn cancel_operation(&self, operation_id: &str) -> bool {
        if let Ok(mut ops) = self.active_operations.lock() {
            if let Some(token) = ops.remove(operation_id) {
                token.store(true, std::sync::atomic::Ordering::Release);
                // Send a failed event
                let event = GitProgressEvent::failed(
                    operation_id,
                    GitOperationType::Commit, // Type doesn't matter much for cancel
                    "Operation cancelled",
                );
                let _ = self.sender.send(event);
                return true;
            }
        }
        false
    }

    /// Removes a completed operation from the registry.
    pub fn complete_operation(&self, operation_id: &str) {
        if let Ok(mut ops) = self.active_operations.lock() {
            ops.remove(operation_id);
        }
    }

    /// Gets the inner sender for creating operation trackers.
    pub fn sender(&self) -> Arc<broadcast::Sender<GitProgressEvent>> {
        Arc::clone(&self.sender)
    }
}

impl Default for GitProgressBroadcaster {
    fn default() -> Self {
        Self::new(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitops::progress::GitOperationPhase;

    #[test]
    fn test_broadcaster_creation() {
        let broadcaster = GitProgressBroadcaster::new(10);
        let _rx = broadcaster.subscribe();
    }

    #[test]
    fn test_broadcaster_send_receive() {
        let broadcaster = GitProgressBroadcaster::new(10);
        let mut rx = broadcaster.subscribe();

        let event = GitProgressEvent::new(
            "test-op",
            GitOperationType::Push,
            GitOperationPhase::Writing,
            "Test message",
        );

        broadcaster.send(event);

        let received = rx.try_recv().unwrap();
        assert_eq!(received.operation_id, "test-op");
        assert_eq!(received.operation_type, GitOperationType::Push);
    }

    #[test]
    fn test_start_operation() {
        let broadcaster = GitProgressBroadcaster::new(10);
        let mut rx = broadcaster.subscribe();

        let progress = broadcaster.start_operation(GitOperationType::Commit);
        progress.phase(GitOperationPhase::StagingFiles, "Staging test files");

        let received = rx.try_recv().unwrap();
        assert_eq!(received.operation_type, GitOperationType::Commit);
        assert_eq!(received.phase, GitOperationPhase::StagingFiles);
    }

    #[test]
    fn test_cancel_operation() {
        let broadcaster = GitProgressBroadcaster::new(10);
        let mut rx = broadcaster.subscribe();

        let progress = broadcaster.start_operation(GitOperationType::Pull);
        let op_id = progress.operation_id().to_string();

        assert!(!progress.is_cancelled());
        assert!(broadcaster.cancel_operation(&op_id));
        assert!(progress.is_cancelled());

        // Should have sent a failed event
        let received = rx.try_recv().unwrap();
        assert_eq!(received.operation_id, op_id);
        assert_eq!(received.phase, GitOperationPhase::Failed);
    }

    #[test]
    fn test_cancel_nonexistent() {
        let broadcaster = GitProgressBroadcaster::new(10);
        assert!(!broadcaster.cancel_operation("nonexistent"));
    }

    #[test]
    fn test_default_capacity() {
        let broadcaster = GitProgressBroadcaster::default();
        let _rx = broadcaster.subscribe();
    }
}
