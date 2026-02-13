//! OAuth2 Device Flow authentication for email sources.
//!
//! This module implements the OAuth2 Device Authorization Grant (RFC 8628)
//! for authorizing email access without requiring a browser on the server.

use log::{debug, info, warn};
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::error::{EmailError, Result};

/// Maximum length for sanitized error bodies to prevent log flooding.
const MAX_ERROR_BODY_LENGTH: usize = 200;

/// Sanitizes an OAuth error response body by truncating to a reasonable length.
/// This prevents sensitive token data from appearing in logs while keeping
/// useful error context.
fn sanitize_oauth_error_body(body: &str) -> String {
    if body.len() > MAX_ERROR_BODY_LENGTH {
        format!("{}... (truncated)", &body[..MAX_ERROR_BODY_LENGTH])
    } else {
        body.to_string()
    }
}

/// OAuth2 provider presets with known endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuth2Provider {
    Gmail,
    Outlook,
    Custom,
}

impl OAuth2Provider {
    /// Returns the device authorization endpoint URL for this provider.
    pub fn device_auth_url(&self) -> Option<&'static str> {
        match self {
            OAuth2Provider::Gmail => Some("https://oauth2.googleapis.com/device/code"),
            OAuth2Provider::Outlook => {
                Some("https://login.microsoftonline.com/common/oauth2/v2.0/devicecode")
            }
            OAuth2Provider::Custom => None,
        }
    }

    /// Returns the token endpoint URL for this provider.
    pub fn token_url(&self) -> Option<&'static str> {
        match self {
            OAuth2Provider::Gmail => Some("https://oauth2.googleapis.com/token"),
            OAuth2Provider::Outlook => {
                Some("https://login.microsoftonline.com/common/oauth2/v2.0/token")
            }
            OAuth2Provider::Custom => None,
        }
    }

    /// Returns the default scopes for IMAP access for this provider.
    /// Note: Gmail requires the full mail scope for IMAP access.
    /// Outlook explicitly requires offline_access for refresh tokens.
    pub fn default_scopes(&self) -> &'static [&'static str] {
        match self {
            // Gmail: access_type=offline is added as a parameter, not a scope
            // But we still need the mail scope
            OAuth2Provider::Gmail => &["https://mail.google.com/"],
            OAuth2Provider::Outlook => &[
                "https://outlook.office.com/IMAP.AccessAsUser.All",
                "offline_access",
            ],
            OAuth2Provider::Custom => &[],
        }
    }
}

/// Response from the device authorization request.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCodeResponse {
    /// The device verification code.
    pub device_code: String,

    /// The end-user verification code to display to the user.
    pub user_code: String,

    /// The verification URI where the user should enter the user_code.
    pub verification_uri: String,

    /// Optional: A URI including the user_code (for QR codes).
    #[serde(default)]
    pub verification_uri_complete: Option<String>,

    /// Lifetime in seconds of the device_code and user_code.
    pub expires_in: u64,

    /// Minimum polling interval in seconds (default: 5).
    #[serde(default = "default_interval")]
    pub interval: u64,
}

fn default_interval() -> u64 {
    5
}

/// Response from the token endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    /// The access token.
    pub access_token: String,

    /// Token type (usually "Bearer").
    #[serde(default)]
    pub token_type: Option<String>,

    /// Lifetime in seconds of the access token.
    #[serde(default)]
    pub expires_in: Option<u64>,

    /// The refresh token (may not always be provided).
    #[serde(default)]
    pub refresh_token: Option<String>,

    /// Space-separated list of granted scopes.
    #[serde(default)]
    pub scope: Option<String>,
}

/// Error response from the token endpoint during polling.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenErrorResponse {
    /// Error code.
    pub error: String,

    /// Human-readable error description.
    #[serde(default)]
    pub error_description: Option<String>,
}

/// Status of the device flow authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthorizationStatus {
    /// Waiting for user to authorize.
    Pending,
    /// User has authorized, tokens received.
    Authorized,
    /// Authorization request expired.
    Expired,
    /// An error occurred.
    Error,
}

/// Result of checking authorization status.
#[derive(Debug, Clone, Serialize)]
pub struct AuthorizationStatusResponse {
    pub status: AuthorizationStatus,
    pub message: String,
}

/// OAuth2 Device Flow authentication handler.
pub struct DeviceFlowAuth {
    client: Client,
    provider: OAuth2Provider,
    device_auth_url: String,
    token_url: String,
}

/// Default connect timeout for HTTP requests (10 seconds).
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Default request timeout for HTTP requests (30 seconds).
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Minimum TTL for polling the token endpoint (5 seconds).
///
/// This ensures we don't immediately time out even if the server sends
/// a very short expires_in value or if there's clock skew between client
/// and server. RFC 8628 recommends servers use at least 600 seconds,
/// but we use a minimal floor to handle edge cases.
const MIN_POLL_TTL_SECS: u64 = 5;

/// Creates an HTTP client with appropriate timeouts.
fn create_http_client() -> Result<Client> {
    Client::builder()
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .timeout(DEFAULT_REQUEST_TIMEOUT)
        .build()
        .map_err(|e| EmailError::OAuth2Error(format!("Failed to create HTTP client: {}", e)))
}

impl DeviceFlowAuth {
    /// Creates a new DeviceFlowAuth for a known provider.
    pub fn new(provider: OAuth2Provider) -> Result<Self> {
        let device_auth_url = provider.device_auth_url().ok_or_else(|| {
            EmailError::OAuth2Error("Custom provider requires explicit URLs".to_string())
        })?;
        let token_url = provider.token_url().ok_or_else(|| {
            EmailError::OAuth2Error("Custom provider requires explicit URLs".to_string())
        })?;

        Ok(Self {
            client: create_http_client()?,
            provider,
            device_auth_url: device_auth_url.to_string(),
            token_url: token_url.to_string(),
        })
    }

    /// Creates a new DeviceFlowAuth with custom URLs.
    pub fn with_custom_urls(device_auth_url: String, token_url: String) -> Result<Self> {
        Ok(Self {
            client: create_http_client()?,
            provider: OAuth2Provider::Custom,
            device_auth_url,
            token_url,
        })
    }

    /// Creates a new DeviceFlowAuth for refresh-only operations.
    ///
    /// Use this constructor when you only need to refresh tokens and don't need
    /// to initiate new device authorization flows. The device_auth_url is not needed.
    pub fn for_refresh(token_url: String) -> Result<Self> {
        Ok(Self {
            client: create_http_client()?,
            provider: OAuth2Provider::Custom,
            device_auth_url: String::new(), // Not used for refresh operations
            token_url,
        })
    }

    /// Creates a new DeviceFlowAuth for refresh-only operations with a known provider.
    pub fn for_refresh_with_provider(provider: OAuth2Provider) -> Result<Self> {
        let token_url = provider.token_url().ok_or_else(|| {
            EmailError::OAuth2Error("Custom provider requires explicit token URL".to_string())
        })?;

        Ok(Self {
            client: create_http_client()?,
            provider,
            device_auth_url: String::new(), // Not used for refresh operations
            token_url: token_url.to_string(),
        })
    }

    /// Step 1: Request a device code from the authorization server.
    pub async fn request_device_code(
        &self,
        client_id: &str,
        scopes: Option<&[&str]>,
    ) -> Result<DeviceCodeResponse> {
        let scopes = scopes.unwrap_or_else(|| self.provider.default_scopes());
        let scope = scopes.join(" ");

        info!(
            "Requesting device code from {} for scopes: {}",
            self.device_auth_url, scope
        );

        // Build parameters - Gmail needs access_type=offline to get refresh tokens
        let mut params: Vec<(&str, &str)> = vec![("client_id", client_id), ("scope", &scope)];

        // For Gmail, request offline access to get refresh tokens
        if self.provider == OAuth2Provider::Gmail {
            params.push(("access_type", "offline"));
        }

        let response = self
            .client
            .post(&self.device_auth_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| {
                EmailError::OAuth2Error(format!("Failed to request device code: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(EmailError::OAuth2Error(format!(
                "Device code request failed ({}): {}",
                status, body
            )));
        }

        let device_code: DeviceCodeResponse = response
            .json()
            .await
            .map_err(|e| EmailError::OAuth2Error(format!("Failed to parse device code: {}", e)))?;

        info!(
            "Device code received. User code: {}, verification URL: {}",
            device_code.user_code, device_code.verification_uri
        );

        Ok(device_code)
    }

    /// Step 2: Poll for the token after user has authorized.
    ///
    /// This method polls the token endpoint with exponential backoff
    /// until the user authorizes, the code expires, or an error occurs.
    pub async fn poll_for_token(
        &self,
        device_code: &DeviceCodeResponse,
        client_id: &str,
        client_secret: &str,
    ) -> Result<TokenResponse> {
        // Apply minimum bound to prevent immediate expiration on edge cases.
        // We don't apply an upper bound since providers like Microsoft use 900s.
        let ttl_secs = device_code.expires_in.max(MIN_POLL_TTL_SECS);
        let deadline = std::time::Instant::now() + Duration::from_secs(ttl_secs);

        // Ensure polling interval has a sensible minimum (at least 1 second)
        let min_interval = Duration::from_secs(1);
        let max_interval = Duration::from_secs(30);
        let mut interval = Duration::from_secs(device_code.interval).max(min_interval);

        // RFC 8628 device authorization grant type (same for all providers)
        const DEVICE_CODE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

        info!("Polling for token authorization (expires in {}s)", ttl_secs);

        loop {
            if std::time::Instant::now() > deadline {
                return Err(EmailError::OAuth2Error(
                    "Device code expired before authorization".to_string(),
                ));
            }

            tokio::time::sleep(interval).await;

            let params = [
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("device_code", &device_code.device_code),
                ("grant_type", DEVICE_CODE_GRANT_TYPE),
            ];

            let response = self
                .client
                .post(&self.token_url)
                .form(&params)
                .send()
                .await
                .map_err(|e| EmailError::OAuth2Error(format!("Token request failed: {}", e)))?;

            if response.status().is_success() {
                let token: TokenResponse = response.json().await.map_err(|e| {
                    EmailError::OAuth2Error(format!("Failed to parse token response: {}", e))
                })?;
                info!("Successfully obtained access token");
                return Ok(token);
            }

            // Check for expected polling errors
            let error: TokenErrorResponse = response.json().await.map_err(|e| {
                EmailError::OAuth2Error(format!("Failed to parse error response: {}", e))
            })?;

            match error.error.as_str() {
                "authorization_pending" => {
                    debug!("Authorization pending, continuing to poll...");
                }
                "slow_down" => {
                    // RFC 8628 section 3.5: add 5 seconds to the polling interval
                    interval += Duration::from_secs(5);
                    interval = interval.min(max_interval);
                    warn!("Server requested slow down, new interval: {:?}", interval);
                }
                "expired_token" => {
                    return Err(EmailError::OAuth2Error(
                        "Device code expired before authorization".to_string(),
                    ));
                }
                "access_denied" => {
                    return Err(EmailError::OAuth2Error(
                        "User denied the authorization request".to_string(),
                    ));
                }
                _ => {
                    return Err(EmailError::OAuth2Error(format!(
                        "Token request error: {} - {}",
                        error.error,
                        error.error_description.unwrap_or_default()
                    )));
                }
            }
        }
    }

    /// Refresh an access token using a refresh token.
    pub async fn refresh_access_token(
        &self,
        refresh_token: &SecretString,
        client_id: &str,
        client_secret: &str,
    ) -> Result<TokenResponse> {
        info!("Refreshing access token");

        let params = [
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token.expose_secret()),
            ("grant_type", "refresh_token"),
        ];

        let response = self
            .client
            .post(&self.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| EmailError::OAuth2Error(format!("Token refresh failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let sanitized_body = sanitize_oauth_error_body(&body);
            return Err(EmailError::OAuth2Error(format!(
                "Token refresh failed ({}): {}",
                status, sanitized_body
            )));
        }

        let token: TokenResponse = response.json().await.map_err(|e| {
            EmailError::OAuth2Error(format!("Failed to parse refresh response: {}", e))
        })?;

        info!("Successfully refreshed access token");
        Ok(token)
    }

    /// Get the token URL for this provider.
    pub fn token_url(&self) -> &str {
        &self.token_url
    }

    /// Get the provider.
    pub fn provider(&self) -> OAuth2Provider {
        self.provider
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gmail_provider_urls() {
        let provider = OAuth2Provider::Gmail;
        assert_eq!(
            provider.device_auth_url(),
            Some("https://oauth2.googleapis.com/device/code")
        );
        assert_eq!(
            provider.token_url(),
            Some("https://oauth2.googleapis.com/token")
        );
        assert_eq!(provider.default_scopes(), &["https://mail.google.com/"]);
    }

    #[test]
    fn test_outlook_provider_urls() {
        let provider = OAuth2Provider::Outlook;
        assert!(provider.device_auth_url().is_some());
        assert!(provider.token_url().is_some());
        assert!(!provider.default_scopes().is_empty());
    }

    #[test]
    fn test_custom_provider_urls() {
        let provider = OAuth2Provider::Custom;
        assert!(provider.device_auth_url().is_none());
        assert!(provider.token_url().is_none());
    }

    #[test]
    fn test_device_flow_auth_creation() {
        let auth = DeviceFlowAuth::new(OAuth2Provider::Gmail);
        assert!(auth.is_ok());

        let auth = DeviceFlowAuth::new(OAuth2Provider::Custom);
        assert!(auth.is_err());
    }

    #[test]
    fn test_device_flow_auth_custom() {
        let auth = DeviceFlowAuth::with_custom_urls(
            "https://example.com/device".to_string(),
            "https://example.com/token".to_string(),
        )
        .expect("should create custom auth");
        assert_eq!(auth.provider(), OAuth2Provider::Custom);
        assert_eq!(auth.token_url(), "https://example.com/token");
    }
}
