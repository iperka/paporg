//! Email OAuth2 authorization commands.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use paporg::email::device_auth::{DeviceFlowAuth, OAuth2Provider};
use paporg::gitops::resource::ImportSourceType;
use paporg::secrets::resolve_secret;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::{Mutex, RwLock};

use super::ApiResponse;
use crate::state::TauriAppState;

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeResponse {
    pub user_code: String,
    pub verification_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationStatusResponse {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenStatusResponse {
    pub has_token: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub is_valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

// ============================================================================
// Pending Authorization State
// ============================================================================

struct PendingAuth {
    device_code: SecretString,
    user_code: String,
    verification_uri: String,
    expires_at: chrono::DateTime<Utc>,
    interval: u64,
    client_id: String,
    client_secret: SecretString,
    provider: OAuth2Provider,
}

impl Clone for PendingAuth {
    fn clone(&self) -> Self {
        Self {
            device_code: SecretString::from(self.device_code.expose_secret().to_string()),
            user_code: self.user_code.clone(),
            verification_uri: self.verification_uri.clone(),
            expires_at: self.expires_at,
            interval: self.interval,
            client_id: self.client_id.clone(),
            client_secret: SecretString::from(self.client_secret.expose_secret().to_string()),
            provider: self.provider,
        }
    }
}

use std::collections::HashMap;
use std::sync::LazyLock;

static PENDING_AUTHS: LazyLock<Mutex<HashMap<String, PendingAuth>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// In-memory storage for OAuth tokens.
/// Note: In production, tokens are persisted via the frontend SQL plugin.
/// This in-memory store is used for the current session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredToken {
    pub source_name: String,
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

static STORED_TOKENS: LazyLock<Mutex<HashMap<String, StoredToken>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ============================================================================
// Commands
// ============================================================================

/// Start Device Flow authorization for an email source.
#[tauri::command]
pub async fn start_email_authorization(
    state: State<'_, Arc<RwLock<TauriAppState>>>,
    source_name: String,
) -> Result<ApiResponse<DeviceCodeResponse>, String> {
    let state = state.read().await;

    let config = match state.config() {
        Some(c) => c,
        None => return Ok(ApiResponse::err("Configuration not loaded")),
    };

    // Find the import source
    let import_source = config
        .import_sources
        .iter()
        .find(|s| s.resource.metadata.name == source_name);

    let source = match import_source {
        Some(s) => s,
        None => {
            return Ok(ApiResponse::err(format!(
                "Import source not found: {}",
                source_name
            )))
        }
    };

    // Verify it's an email source with OAuth2
    if source.resource.spec.source_type != ImportSourceType::Email {
        return Ok(ApiResponse::err(format!(
            "Import source '{}' is not an email source",
            source_name
        )));
    }

    let email_config = match &source.resource.spec.email {
        Some(c) => c,
        None => {
            return Ok(ApiResponse::err(format!(
                "Email configuration missing for source '{}'",
                source_name
            )))
        }
    };

    let oauth2_settings = match &email_config.auth.oauth2 {
        Some(s) => s,
        None => {
            return Ok(ApiResponse::err(format!(
                "OAuth2 not configured for source '{}'",
                source_name
            )))
        }
    };

    // Get provider
    let provider = match oauth2_settings.provider {
        paporg::gitops::resource::OAuth2Provider::Gmail => OAuth2Provider::Gmail,
        paporg::gitops::resource::OAuth2Provider::Outlook => OAuth2Provider::Outlook,
        paporg::gitops::resource::OAuth2Provider::Custom => {
            return Ok(ApiResponse::err(
                "Custom OAuth2 providers are not yet supported for Device Flow",
            ));
        }
    };

    let device_auth = match DeviceFlowAuth::new(provider) {
        Ok(auth) => auth,
        Err(e) => return Ok(ApiResponse::err(e.to_string())),
    };

    // Get client credentials
    let client_id = match resolve_secret(
        oauth2_settings.client_id_insecure.as_deref(),
        oauth2_settings.client_id_file.as_deref(),
        oauth2_settings.client_id_env_var.as_deref(),
    ) {
        Ok(secret) => secret.expose_secret().to_string(),
        Err(e) => {
            return Ok(ApiResponse::err(format!(
                "Failed to resolve client ID: {}",
                e
            )))
        }
    };

    let client_secret = match resolve_secret(
        oauth2_settings.client_secret_insecure.as_deref(),
        oauth2_settings.client_secret_file.as_deref(),
        oauth2_settings.client_secret_env_var.as_deref(),
    ) {
        Ok(secret) => secret.expose_secret().to_string(),
        Err(e) => {
            return Ok(ApiResponse::err(format!(
                "Failed to resolve client secret: {}",
                e
            )))
        }
    };

    drop(state);

    // Request device code
    let device_code = match device_auth.request_device_code(&client_id, None).await {
        Ok(code) => code,
        Err(e) => return Ok(ApiResponse::err(e.to_string())),
    };

    // Store pending auth
    let pending = PendingAuth {
        device_code: SecretString::from(device_code.device_code.clone()),
        user_code: device_code.user_code.clone(),
        verification_uri: device_code.verification_uri.clone(),
        expires_at: Utc::now() + chrono::Duration::seconds(device_code.expires_in as i64),
        interval: device_code.interval,
        client_id,
        client_secret: SecretString::from(client_secret),
        provider,
    };

    {
        let mut auths = PENDING_AUTHS.lock().await;
        auths.insert(source_name.clone(), pending);
    }

    Ok(ApiResponse::ok(DeviceCodeResponse {
        user_code: device_code.user_code,
        verification_uri: device_code.verification_uri.clone(),
        verification_uri_complete: device_code.verification_uri_complete,
        expires_in: device_code.expires_in,
        interval: device_code.interval,
    }))
}

/// Check authorization status for a source.
#[tauri::command]
pub async fn check_authorization_status(
    _state: State<'_, Arc<RwLock<TauriAppState>>>,
    source_name: String,
) -> Result<ApiResponse<AuthorizationStatusResponse>, String> {
    // Check for pending auth
    let pending_data = {
        let auths = PENDING_AUTHS.lock().await;
        auths.get(&source_name).cloned()
    };

    let pending = match pending_data {
        Some(p) => p,
        None => {
            // Check if we already have a token in memory
            let tokens = STORED_TOKENS.lock().await;
            if let Some(token) = tokens.get(&source_name) {
                let is_valid = token.expires_at > Utc::now();
                return Ok(ApiResponse::ok(AuthorizationStatusResponse {
                    status: if is_valid {
                        "authorized".to_string()
                    } else {
                        "expired".to_string()
                    },
                    message: if is_valid {
                        "Token is valid".to_string()
                    } else {
                        "Token has expired, please re-authorize".to_string()
                    },
                }));
            }

            return Ok(ApiResponse::ok(AuthorizationStatusResponse {
                status: "not_started".to_string(),
                message: "No authorization in progress".to_string(),
            }));
        }
    };

    // Check if expired
    if Utc::now() > pending.expires_at {
        let mut auths = PENDING_AUTHS.lock().await;
        auths.remove(&source_name);

        return Ok(ApiResponse::ok(AuthorizationStatusResponse {
            status: "expired".to_string(),
            message: "Device code expired, please start again".to_string(),
        }));
    }

    // Create device flow auth and poll for token
    let device_auth = match DeviceFlowAuth::new(pending.provider) {
        Ok(auth) => auth,
        Err(e) => {
            return Ok(ApiResponse::ok(AuthorizationStatusResponse {
                status: "error".to_string(),
                message: e.to_string(),
            }))
        }
    };

    // Single poll attempt
    let params = [
        ("client_id", pending.client_id.as_str()),
        ("client_secret", pending.client_secret.expose_secret()),
        ("device_code", pending.device_code.expose_secret()),
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
    ];

    let client = match reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return Ok(ApiResponse::ok(AuthorizationStatusResponse {
                status: "error".to_string(),
                message: format!("Failed to create HTTP client: {}", e),
            }));
        }
    };

    let poll_result: Result<Option<paporg::email::device_auth::TokenResponse>, String> =
        match client
            .post(device_auth.token_url())
            .form(&params)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json().await {
                        Ok(token) => Ok(Some(token)),
                        Err(e) => Err(e.to_string()),
                    }
                } else {
                    match response.json::<serde_json::Value>().await {
                        Ok(error) => {
                            let error_code = error
                                .get("error")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");

                            match error_code {
                                "authorization_pending" => Ok(None),
                                "slow_down" => Ok(None),
                                "expired_token" => Err("expired".to_string()),
                                "access_denied" => Err("denied".to_string()),
                                _ => Err(error_code.to_string()),
                            }
                        }
                        Err(e) => Err(e.to_string()),
                    }
                }
            }
            Err(e) => Err(e.to_string()),
        };

    match poll_result {
        Ok(Some(token)) => {
            // Success! Store the token in memory
            let provider_str = match pending.provider {
                OAuth2Provider::Gmail => "gmail",
                OAuth2Provider::Outlook => "outlook",
                OAuth2Provider::Custom => "custom",
            };

            let now = Utc::now();
            let expires_in = token.expires_in.unwrap_or(3600) as i64;
            let stored_token = StoredToken {
                source_name: source_name.clone(),
                provider: provider_str.to_string(),
                access_token: token.access_token.clone(),
                refresh_token: token.refresh_token.clone(),
                expires_at: now + chrono::Duration::seconds(expires_in),
                created_at: now,
                updated_at: now,
            };

            {
                let mut tokens = STORED_TOKENS.lock().await;
                tokens.insert(source_name.clone(), stored_token);
            }

            // Clean up pending auth
            let mut auths = PENDING_AUTHS.lock().await;
            auths.remove(&source_name);

            Ok(ApiResponse::ok(AuthorizationStatusResponse {
                status: "authorized".to_string(),
                message: "Authorization successful".to_string(),
            }))
        }
        Ok(None) => Ok(ApiResponse::ok(AuthorizationStatusResponse {
            status: "pending".to_string(),
            message: "Waiting for user authorization".to_string(),
        })),
        Err(e) => {
            let mut auths = PENDING_AUTHS.lock().await;
            auths.remove(&source_name);

            let (status, message) = if e == "expired" {
                ("expired", "Device code expired")
            } else if e == "denied" {
                ("denied", "User denied authorization")
            } else {
                ("error", "Authorization failed")
            };

            Ok(ApiResponse::ok(AuthorizationStatusResponse {
                status: status.to_string(),
                message: message.to_string(),
            }))
        }
    }
}

/// Get token status for a source.
#[tauri::command]
pub async fn get_token_status(
    _state: State<'_, Arc<RwLock<TauriAppState>>>,
    source_name: String,
) -> Result<ApiResponse<TokenStatusResponse>, String> {
    let tokens = STORED_TOKENS.lock().await;

    match tokens.get(&source_name) {
        Some(t) => {
            let is_valid = t.expires_at > Utc::now();
            Ok(ApiResponse::ok(TokenStatusResponse {
                has_token: true,
                expires_at: Some(t.expires_at.to_rfc3339()),
                is_valid,
                provider: Some(t.provider.clone()),
            }))
        }
        None => Ok(ApiResponse::ok(TokenStatusResponse {
            has_token: false,
            expires_at: None,
            is_valid: false,
            provider: None,
        })),
    }
}

/// Revoke token for a source.
#[tauri::command]
pub async fn revoke_token(
    _state: State<'_, Arc<RwLock<TauriAppState>>>,
    source_name: String,
) -> Result<ApiResponse<()>, String> {
    // Clear any pending authorization
    {
        let mut auths = PENDING_AUTHS.lock().await;
        auths.remove(&source_name);
    }

    // Remove from in-memory token store
    {
        let mut tokens = STORED_TOKENS.lock().await;
        tokens.remove(&source_name);
    }

    Ok(ApiResponse::ok(()))
}
