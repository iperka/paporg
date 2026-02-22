//! Log broadcasting for real-time log streaming.

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fmt;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

#[derive(Debug, Clone, Serialize)]
pub struct LogEvent {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
}

impl LogEvent {
    pub fn new(level: &str, target: &str, message: &str) -> Self {
        Self {
            timestamp: Utc::now(),
            level: level.to_string(),
            target: target.to_string(),
            message: message.to_string(),
        }
    }
}

#[derive(Clone)]
pub struct LogBroadcaster {
    sender: broadcast::Sender<LogEvent>,
}

impl LogBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn send(&self, event: LogEvent) {
        // Ignore errors - no active receivers is fine
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LogEvent> {
        self.sender.subscribe()
    }

    pub fn log(&self, level: &str, target: &str, message: &str) {
        self.send(LogEvent::new(level, target, message));
    }

    pub fn info(&self, target: &str, message: &str) {
        self.log("INFO", target, message);
    }

    pub fn warn(&self, target: &str, message: &str) {
        self.log("WARN", target, message);
    }

    pub fn error(&self, target: &str, message: &str) {
        self.log("ERROR", target, message);
    }

    pub fn debug(&self, target: &str, message: &str) {
        self.log("DEBUG", target, message);
    }
}

impl Default for LogBroadcaster {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// A tracing `Layer` that broadcasts log events via `LogBroadcaster`.
///
/// Replaces the old `BroadcastingLogWriter` (which parsed env_logger text
/// with regexes). This layer receives structured event data directly from
/// the tracing subscriber — no parsing needed.
pub struct BroadcastLayer {
    broadcaster: Arc<LogBroadcaster>,
}

impl BroadcastLayer {
    pub fn new(broadcaster: Arc<LogBroadcaster>) -> Self {
        Self { broadcaster }
    }
}

impl<S: Subscriber> Layer<S> for BroadcastLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        let level = match *metadata.level() {
            tracing::Level::ERROR => "ERROR",
            tracing::Level::WARN => "WARN",
            tracing::Level::INFO => "INFO",
            tracing::Level::DEBUG => "DEBUG",
            tracing::Level::TRACE => "TRACE",
        };

        let target = metadata.target();

        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        self.broadcaster.log(level, target, &visitor.message);
    }
}

/// Visitor that extracts the `message` field from a tracing event.
#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_event_creation() {
        let event = LogEvent::new("INFO", "test", "Hello world");
        assert_eq!(event.level, "INFO");
        assert_eq!(event.target, "test");
        assert_eq!(event.message, "Hello world");
    }

    #[test]
    fn test_broadcaster_send_receive() {
        let broadcaster = LogBroadcaster::new(10);
        let mut receiver = broadcaster.subscribe();

        broadcaster.info("test", "Hello");

        let event = receiver.try_recv().unwrap();
        assert_eq!(event.level, "INFO");
        assert_eq!(event.message, "Hello");
    }
}
