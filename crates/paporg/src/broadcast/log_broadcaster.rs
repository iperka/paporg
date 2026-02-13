//! Log broadcasting for real-time log streaming.

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;

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

/// A log writer that can be used with env_logger to broadcast logs.
pub struct BroadcastingLogWriter {
    broadcaster: Arc<LogBroadcaster>,
}

impl BroadcastingLogWriter {
    pub fn new(broadcaster: Arc<LogBroadcaster>) -> Self {
        Self { broadcaster }
    }
}

impl std::io::Write for BroadcastingLogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // Parse the log line and broadcast it
        if let Ok(line) = std::str::from_utf8(buf) {
            // Simple parsing - in practice you'd want more robust parsing
            let line = line.trim();
            if !line.is_empty() {
                // Try to parse level from env_logger format: [TIMESTAMP LEVEL target] message
                let (level, target, message) = parse_log_line(line);
                self.broadcaster.log(&level, &target, &message);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn parse_log_line(line: &str) -> (String, String, String) {
    // env_logger default format: [2024-01-15T10:30:00Z INFO  target] message
    // Try to extract level and target from bracketed prefix
    if let Some(bracket_end) = line.find(']') {
        let prefix = &line[..bracket_end];
        let message = line[bracket_end + 1..].trim().to_string();

        // Try to find level
        let level = if prefix.contains("ERROR") {
            "ERROR"
        } else if prefix.contains("WARN") {
            "WARN"
        } else if prefix.contains("INFO") {
            "INFO"
        } else if prefix.contains("DEBUG") {
            "DEBUG"
        } else if prefix.contains("TRACE") {
            "TRACE"
        } else {
            "INFO"
        };

        // Try to find target (last word before the bracket)
        let target = prefix
            .split_whitespace()
            .last()
            .unwrap_or("paporg")
            .to_string();

        (level.to_string(), target, message)
    } else {
        ("INFO".to_string(), "paporg".to_string(), line.to_string())
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

    #[test]
    fn test_parse_log_line() {
        let (level, target, message) =
            parse_log_line("[2024-01-15T10:30:00Z INFO  paporg::worker] Processing document");
        assert_eq!(level, "INFO");
        assert_eq!(target, "paporg::worker");
        assert_eq!(message, "Processing document");
    }
}
