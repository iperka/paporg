//! Git progress broadcaster for real-time git operation streaming.

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::gitops::progress::{GitOperationType, GitProgressEvent, OperationProgress};

/// Broadcasts git operation progress events for streaming.
#[derive(Clone)]
pub struct GitProgressBroadcaster {
    sender: Arc<broadcast::Sender<GitProgressEvent>>,
}

impl GitProgressBroadcaster {
    /// Creates a new git progress broadcaster with the specified channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender: Arc::new(sender),
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
    pub fn start_operation(&self, operation_type: GitOperationType) -> OperationProgress {
        OperationProgress::new(operation_type, Arc::clone(&self.sender))
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
    fn test_default_capacity() {
        let broadcaster = GitProgressBroadcaster::default();
        let _rx = broadcaster.subscribe();
    }
}
