//! Email import source error types.

use thiserror::Error;

/// Errors that can occur during email import operations.
#[derive(Error, Debug)]
pub enum EmailError {
    /// Failed to connect to the IMAP server.
    #[error("IMAP connection failed: {0}")]
    ConnectionFailed(String),

    /// TLS/SSL error during connection.
    #[error("TLS error: {0}")]
    TlsError(String),

    /// Authentication failed.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Failed to retrieve credentials from environment variable.
    #[error("Credentials not found: environment variable '{0}' is not set")]
    CredentialsNotFound(String),

    /// IMAP protocol error.
    #[error("IMAP protocol error: {0}")]
    ProtocolError(String),

    /// Failed to parse email message.
    #[error("Failed to parse email: {0}")]
    ParseError(String),

    /// Failed to extract attachment.
    #[error("Failed to extract attachment: {0}")]
    AttachmentError(String),

    /// OAuth2 token refresh failed.
    #[error("OAuth2 token refresh failed: {0}")]
    OAuth2Error(String),

    /// Database error for tracking processed emails.
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// IO error when saving attachments.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Folder not found.
    #[error("IMAP folder '{0}' not found")]
    FolderNotFound(String),

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    /// UIDVALIDITY changed - folder was recreated.
    #[error("UIDVALIDITY changed for folder '{0}': was {1}, now {2}")]
    UidValidityChanged(String, u32, u32),

    /// Operation timed out.
    #[error("Operation timed out: {0}")]
    Timeout(String),
}

impl From<async_native_tls::Error> for EmailError {
    fn from(err: async_native_tls::Error) -> Self {
        EmailError::TlsError(err.to_string())
    }
}

impl From<crate::db::DatabaseError> for EmailError {
    fn from(err: crate::db::DatabaseError) -> Self {
        EmailError::DatabaseError(err.to_string())
    }
}

/// Result type for email operations.
pub type Result<T> = std::result::Result<T, EmailError>;
