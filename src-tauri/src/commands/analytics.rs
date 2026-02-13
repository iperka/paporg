//! Analytics/telemetry commands.

use std::sync::Arc;

use serde_json::Value;
use tauri::State;
use tokio::sync::RwLock;

use super::ApiResponse;
use crate::state::TauriAppState;

const MAX_EVENT_NAME_LEN: usize = 80;

fn normalize_event_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Event name is required".to_string());
    }

    if trimmed.len() > MAX_EVENT_NAME_LEN {
        return Err(format!(
            "Event name exceeds maximum length of {}",
            MAX_EVENT_NAME_LEN
        ));
    }

    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        return Err("Event name contains invalid characters".to_string());
    }

    Ok(trimmed.to_string())
}

fn format_event_message(name: &str, properties: Option<Value>) -> String {
    let payload = serde_json::json!({
        "event": name,
        "properties": properties.unwrap_or_else(|| serde_json::json!({}))
    });

    payload.to_string()
}

/// Track a frontend event by writing it to the log broadcaster.
#[tauri::command]
pub async fn track_event(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    name: String,
    properties: Option<Value>,
) -> Result<ApiResponse<()>, String> {
    let normalized = match normalize_event_name(&name) {
        Ok(value) => value,
        Err(err) => return Ok(ApiResponse::err(err)),
    };

    let message = format_event_message(&normalized, properties);
    let state = state.read().await;
    state.log_broadcaster.info("paporg::analytics", &message);

    Ok(ApiResponse::ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_event_name_trims_and_allows_simple_names() {
        let name = normalize_event_name("  rule_share_opened ").unwrap();
        assert_eq!(name, "rule_share_opened");
    }

    #[test]
    fn normalize_event_name_rejects_empty() {
        let err = normalize_event_name("   ").unwrap_err();
        assert_eq!(err, "Event name is required");
    }

    #[test]
    fn format_event_message_serializes_json() {
        let message = format_event_message(
            "rule_share_copied",
            Some(serde_json::json!({ "rule": "tax" })),
        );
        let value: serde_json::Value = serde_json::from_str(&message).unwrap();
        assert_eq!(value["event"], "rule_share_copied");
        assert_eq!(value["properties"]["rule"], "tax");
    }
}
