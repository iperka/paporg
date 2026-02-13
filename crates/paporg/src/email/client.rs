//! IMAP client for connecting to email servers.

use async_imap::Session;
use async_native_tls::TlsConnector;
use futures_util::StreamExt;
use log::{debug, info, warn};
use secrecy::{ExposeSecret, SecretString};

use crate::gitops::resource::{EmailAuthSettings, EmailAuthType, EmailSourceConfig};

use super::error::{EmailError, Result};

/// Type alias for the underlying async stream (using async-std compatible TcpStream).
type AsyncTcpStream = async_io::Async<std::net::TcpStream>;

/// Simple authenticator for XOAUTH2.
struct XOAuth2Authenticator {
    response: String,
}

impl async_imap::Authenticator for XOAuth2Authenticator {
    type Response = String;

    fn process(&mut self, _data: &[u8]) -> Self::Response {
        // Return the pre-computed base64 response
        std::mem::take(&mut self.response)
    }
}

/// Type alias for the TLS stream used by the IMAP session.
type TlsStream = async_native_tls::TlsStream<AsyncTcpStream>;

/// IMAP client for fetching emails and attachments.
pub struct ImapClient {
    session: Option<Session<TlsStream>>,
    config: EmailSourceConfig,
    current_folder: Option<String>,
    current_uidvalidity: Option<u32>,
}

impl ImapClient {
    /// Creates a new IMAP client with the given configuration.
    pub fn new(config: EmailSourceConfig) -> Self {
        Self {
            session: None,
            config,
            current_folder: None,
            current_uidvalidity: None,
        }
    }

    /// Connects to the IMAP server and authenticates.
    pub async fn connect(&mut self) -> Result<()> {
        if self.session.is_some() {
            debug!("Already connected to IMAP server");
            return Ok(());
        }

        if !self.config.use_tls {
            return Err(EmailError::ConfigError(
                "TLS is required for secure email connections".to_string(),
            ));
        }

        let addr = format!("{}:{}", self.config.host, self.config.port);
        info!("Connecting to IMAP server at {}", addr);

        // Establish TCP connection using std::net and wrap with async-io
        let std_stream = std::net::TcpStream::connect(&addr)
            .map_err(|e| EmailError::ConnectionFailed(e.to_string()))?;
        std_stream
            .set_nonblocking(true)
            .map_err(|e| EmailError::ConnectionFailed(e.to_string()))?;
        let tcp_stream = async_io::Async::new(std_stream)
            .map_err(|e| EmailError::ConnectionFailed(e.to_string()))?;

        // Wrap with TLS
        let tls = TlsConnector::new();
        let tls_stream = tls
            .connect(&self.config.host, tcp_stream)
            .await
            .map_err(|e| EmailError::TlsError(e.to_string()))?;

        // Create IMAP client
        let client = async_imap::Client::new(tls_stream);

        // Authenticate based on auth type
        let session = match self.config.auth.auth_type {
            EmailAuthType::Password => self.authenticate_password(client).await?,
            EmailAuthType::OAuth2 => self.authenticate_oauth2(client).await?,
        };

        info!("Successfully authenticated to IMAP server");
        self.session = Some(session);
        Ok(())
    }

    /// Authenticates using password from environment variable.
    async fn authenticate_password(
        &self,
        client: async_imap::Client<TlsStream>,
    ) -> Result<Session<TlsStream>> {
        let password = self.get_password(&self.config.auth)?;

        client
            .login(&self.config.username, password.expose_secret())
            .await
            .map_err(|(e, _)| EmailError::AuthenticationFailed(e.to_string()))
    }

    /// Authenticates using OAuth2 XOAUTH2 mechanism.
    async fn authenticate_oauth2(
        &self,
        client: async_imap::Client<TlsStream>,
    ) -> Result<Session<TlsStream>> {
        let oauth2_settings = self.config.auth.oauth2.as_ref().ok_or_else(|| {
            EmailError::ConfigError(
                "OAuth2 settings required for OAuth2 authentication".to_string(),
            )
        })?;

        // Get access token (would need to refresh if expired)
        let access_token = self.get_oauth2_access_token(oauth2_settings).await?;

        // Build XOAUTH2 authentication string
        // Format: base64("user=" + user + "^Aauth=Bearer " + token + "^A^A")
        let auth_string = format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.config.username,
            access_token.expose_secret()
        );

        // Use the base64-encoded auth string directly
        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            auth_string.as_bytes(),
        );

        // For XOAUTH2, we use the authenticate method with a simple struct
        client
            .authenticate("XOAUTH2", XOAuth2Authenticator { response: encoded })
            .await
            .map_err(|(e, _)| EmailError::AuthenticationFailed(e.to_string()))
    }

    /// Gets the password from configured sources (direct value, file, or env var).
    fn get_password(&self, auth: &EmailAuthSettings) -> Result<SecretString> {
        // Log warning if insecure direct password is used
        if auth.password_insecure.is_some() {
            log::warn!(
                "Using direct password value (passwordInsecure) is not recommended. \
                 Consider using passwordEnvVar or passwordFile instead."
            );
        }
        crate::secrets::resolve_secret(
            auth.password_insecure.as_deref(),
            auth.password_file.as_deref(),
            auth.password_env_var.as_deref(),
        )
        .map_err(|e| EmailError::CredentialsNotFound(e.to_string()))
    }

    /// Gets an OAuth2 access token.
    ///
    /// This method attempts to get an access token in the following order:
    /// 1. From configured sources (direct value, file, or env var) for refresh token
    /// 2. From a database-stored token (if using Device Flow)
    /// 3. From an environment variable (for manual token management)
    ///
    /// Note: For OAuth2 authentication with Device Flow, tokens are stored in the database
    /// and automatically refreshed. For manual setups, tokens must be managed externally.
    async fn get_oauth2_access_token(
        &self,
        oauth2: &crate::gitops::resource::OAuth2Settings,
    ) -> Result<SecretString> {
        use crate::secrets::resolve_secret;

        // Resolve client credentials once upfront
        let client_id = resolve_secret(
            oauth2.client_id_insecure.as_deref(),
            oauth2.client_id_file.as_deref(),
            oauth2.client_id_env_var.as_deref(),
        )
        .map_err(|_| {
            EmailError::CredentialsNotFound(
                "OAuth2 client ID not configured (need clientId, clientIdFile, or clientIdEnvVar)"
                    .to_string(),
            )
        })?;

        let client_secret = resolve_secret(
            oauth2.client_secret_insecure.as_deref(),
            oauth2.client_secret_file.as_deref(),
            oauth2.client_secret_env_var.as_deref(),
        )
        .map_err(|_| {
            EmailError::CredentialsNotFound(
                "OAuth2 client secret not configured (need clientSecret, clientSecretFile, or clientSecretEnvVar)".to_string(),
            )
        })?;

        // Try to get refresh token from configured sources
        let refresh_token_result = resolve_secret(
            oauth2.refresh_token_insecure.as_deref(),
            oauth2.refresh_token_file.as_deref(),
            oauth2.refresh_token_env_var.as_deref(),
        );

        // If we have a refresh token, use it to obtain a new access token
        if let Ok(refresh_token) = refresh_token_result {
            // Create device flow auth for refresh-only operation
            let device_auth = match oauth2.provider {
                crate::gitops::resource::OAuth2Provider::Gmail => {
                    crate::email::device_auth::DeviceFlowAuth::for_refresh_with_provider(
                        crate::email::device_auth::OAuth2Provider::Gmail,
                    )
                    .map_err(|e| EmailError::OAuth2Error(e.to_string()))?
                }
                crate::gitops::resource::OAuth2Provider::Outlook => {
                    crate::email::device_auth::DeviceFlowAuth::for_refresh_with_provider(
                        crate::email::device_auth::OAuth2Provider::Outlook,
                    )
                    .map_err(|e| EmailError::OAuth2Error(e.to_string()))?
                }
                crate::gitops::resource::OAuth2Provider::Custom => {
                    if let Some(ref token_url) = oauth2.token_url {
                        crate::email::device_auth::DeviceFlowAuth::for_refresh(token_url.clone())
                            .map_err(|e| EmailError::OAuth2Error(e.to_string()))?
                    } else {
                        return Err(EmailError::OAuth2Error(
                            "Custom OAuth2 provider requires token_url".to_string(),
                        ));
                    }
                }
            };

            // Perform the token refresh using already-resolved credentials
            let token_response = device_auth
                .refresh_access_token(
                    &refresh_token,
                    client_id.expose_secret(),
                    client_secret.expose_secret(),
                )
                .await?;

            return Ok(SecretString::from(token_response.access_token));
        }

        // Build expected env var names based on refresh token env var
        // Validate that the env var follows the expected pattern
        let base_name = if let Some(ref env_var) = oauth2.refresh_token_env_var {
            if env_var.ends_with("_REFRESH_TOKEN") {
                env_var.trim_end_matches("_REFRESH_TOKEN")
            } else {
                // Env var doesn't follow expected pattern, warn and use provider fallback
                warn!(
                    "refreshTokenEnvVar '{}' does not end with '_REFRESH_TOKEN'. \
                     Using provider-based access token env var name.",
                    env_var
                );
                ""
            }
        } else {
            ""
        };

        // Map provider to a display-friendly name for env var construction
        let provider_name = match oauth2.provider {
            crate::gitops::resource::OAuth2Provider::Gmail => "GMAIL",
            crate::gitops::resource::OAuth2Provider::Outlook => "OUTLOOK",
            crate::gitops::resource::OAuth2Provider::Custom => "OAUTH2",
        };

        let access_token_env = if base_name.is_empty() {
            format!("{}_ACCESS_TOKEN", provider_name)
        } else {
            format!("{}_ACCESS_TOKEN", base_name)
        };

        // Try to get a dedicated access token env var
        if let Ok(token) = std::env::var(&access_token_env) {
            // Trim whitespace for consistency (env vars may have trailing newlines)
            return Ok(SecretString::from(token.trim()));
        }

        // Build helpful error message
        let token_url_hint = oauth2
            .token_url
            .as_deref()
            .or(match oauth2.provider {
                crate::gitops::resource::OAuth2Provider::Gmail => {
                    Some("https://oauth2.googleapis.com/token")
                }
                crate::gitops::resource::OAuth2Provider::Outlook => {
                    Some("https://login.microsoftonline.com/common/oauth2/v2.0/token")
                }
                crate::gitops::resource::OAuth2Provider::Custom => None,
            })
            .unwrap_or("(custom token URL)");

        Err(EmailError::OAuth2Error(format!(
            "OAuth2 access token not found. Either:\n\
             1. Set {} environment variable with a valid access token, or\n\
             2. Provide refreshToken, refreshTokenFile, or refreshTokenEnvVar, or\n\
             3. Use 'paporg email authorize <source>' to set up Device Flow authentication.\n\
             Token URL for manual refresh: {}",
            access_token_env, token_url_hint
        )))
    }

    /// Opens a folder in read-only mode using EXAMINE (not SELECT).
    /// This ensures the folder is not modified and emails are not marked as read.
    pub async fn examine_folder(&mut self, folder: &str) -> Result<u32> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| EmailError::ConnectionFailed("Not connected".to_string()))?;

        info!("Examining folder: {}", folder);

        let mailbox = session.examine(folder).await.map_err(|e| {
            if e.to_string().contains("Mailbox doesn't exist") || e.to_string().contains("NO") {
                EmailError::FolderNotFound(folder.to_string())
            } else {
                EmailError::ProtocolError(e.to_string())
            }
        })?;

        let uidvalidity = mailbox.uid_validity.ok_or_else(|| {
            EmailError::ProtocolError("Server did not provide UIDVALIDITY".to_string())
        })?;

        self.current_folder = Some(folder.to_string());
        self.current_uidvalidity = Some(uidvalidity);

        debug!(
            "Folder '{}' opened with UIDVALIDITY={}",
            folder, uidvalidity
        );
        Ok(uidvalidity)
    }

    /// Returns the current UIDVALIDITY value.
    pub fn uidvalidity(&self) -> Option<u32> {
        self.current_uidvalidity
    }

    /// Searches for unseen messages since a given UID.
    /// Returns a list of UIDs that match the search criteria.
    pub async fn search_since_uid(&mut self, last_uid: u32) -> Result<Vec<u32>> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| EmailError::ConnectionFailed("Not connected".to_string()))?;

        // Search for messages with UID greater than last_uid
        let query = format!("UID {}:*", last_uid + 1);
        debug!("Searching with query: {}", query);

        let uids = session
            .uid_search(&query)
            .await
            .map_err(|e| EmailError::ProtocolError(e.to_string()))?;

        let uid_list: Vec<u32> = uids.into_iter().collect();
        debug!("Found {} messages matching search", uid_list.len());
        Ok(uid_list)
    }

    /// Searches for messages received since a given date.
    /// Date should be in RFC 2822 format (e.g., "01-Jan-2024").
    pub async fn search_since_date(&mut self, date: &str) -> Result<Vec<u32>> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| EmailError::ConnectionFailed("Not connected".to_string()))?;

        let query = format!("SINCE {}", date);
        debug!("Searching with query: {}", query);

        let uids = session
            .uid_search(&query)
            .await
            .map_err(|e| EmailError::ProtocolError(e.to_string()))?;

        let uid_list: Vec<u32> = uids.into_iter().collect();
        debug!("Found {} messages since {}", uid_list.len(), date);
        Ok(uid_list)
    }

    /// Fetches an email message by UID using BODY.PEEK[] to avoid marking as read.
    pub async fn fetch_email_peek(&mut self, uid: u32) -> Result<Vec<u8>> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| EmailError::ConnectionFailed("Not connected".to_string()))?;

        debug!("Fetching email with UID {}", uid);

        // Use BODY.PEEK[] to fetch without marking as read
        let mut messages = session
            .uid_fetch(uid.to_string(), "BODY.PEEK[]")
            .await
            .map_err(|e| EmailError::ProtocolError(e.to_string()))?;

        // Get the first (and should be only) message from the stream
        let message = messages
            .next()
            .await
            .ok_or_else(|| {
                EmailError::ProtocolError(format!("Message with UID {} not found", uid))
            })?
            .map_err(|e| EmailError::ProtocolError(e.to_string()))?;

        let body = message
            .body()
            .ok_or_else(|| EmailError::ProtocolError("Message has no body".to_string()))?;

        Ok(body.to_vec())
    }

    /// Fetches multiple email messages by UID range.
    pub async fn fetch_emails_peek(&mut self, uids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }

        let session = self
            .session
            .as_mut()
            .ok_or_else(|| EmailError::ConnectionFailed("Not connected".to_string()))?;

        // Build UID set (e.g., "1,2,5,10")
        let uid_set = uids
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");

        debug!("Fetching {} emails with UIDs: {}", uids.len(), uid_set);

        let mut messages = session
            .uid_fetch(&uid_set, "(UID BODY.PEEK[])")
            .await
            .map_err(|e| EmailError::ProtocolError(e.to_string()))?;

        let mut results = Vec::new();
        while let Some(message_result) = messages.next().await {
            match message_result {
                Ok(message) => {
                    if let (Some(uid), Some(body)) = (message.uid, message.body()) {
                        results.push((uid, body.to_vec()));
                    } else {
                        warn!("Message missing UID or body");
                    }
                }
                Err(e) => {
                    warn!("Error fetching message: {}", e);
                }
            }
        }

        debug!("Successfully fetched {} emails", results.len());
        Ok(results)
    }

    /// Disconnects from the IMAP server gracefully.
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(mut session) = self.session.take() {
            info!("Disconnecting from IMAP server");
            session
                .logout()
                .await
                .map_err(|e| EmailError::ProtocolError(e.to_string()))?;
        }
        self.current_folder = None;
        self.current_uidvalidity = None;
        Ok(())
    }

    /// Checks if the client is currently connected.
    pub fn is_connected(&self) -> bool {
        self.session.is_some()
    }
}

impl Drop for ImapClient {
    fn drop(&mut self) {
        if self.session.is_some() {
            warn!("ImapClient dropped without explicit disconnect - session will be closed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitops::resource::{AttachmentFilters, EmailAuthSettings, EmailAuthType};

    fn create_test_config() -> EmailSourceConfig {
        EmailSourceConfig {
            host: "imap.example.com".to_string(),
            port: 993,
            use_tls: true,
            username: "test@example.com".to_string(),
            auth: EmailAuthSettings {
                auth_type: EmailAuthType::Password,
                password_env_var: Some("TEST_EMAIL_PASSWORD".to_string()),
                password_insecure: None,
                password_file: None,
                oauth2: None,
            },
            folder: "INBOX".to_string(),
            since_date: None,
            mime_filters: AttachmentFilters::default(),
            min_attachment_size: 0,
            max_attachment_size: 52_428_800,
            poll_interval: 300,
            batch_size: 50,
        }
    }

    #[test]
    fn test_client_creation() {
        let config = create_test_config();
        let client = ImapClient::new(config);
        assert!(!client.is_connected());
    }

    #[tokio::test]
    async fn test_tls_required() {
        let mut config = create_test_config();
        config.use_tls = false;

        let mut client = ImapClient::new(config);
        let result = client.connect().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EmailError::ConfigError(_)));
    }
}
