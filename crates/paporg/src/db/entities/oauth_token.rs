//! OAuth token entity for storing OAuth2 access and refresh tokens.

use sea_orm::entity::prelude::*;

/// Maximum expires_in value we accept (1 year in seconds).
/// This prevents overflow when casting u64 to i64.
const MAX_EXPIRES_IN_SECONDS: u64 = 365 * 24 * 60 * 60;

/// Enum for specifying refresh token update behavior.
///
/// This provides a clearer API than `Option<Option<String>>`:
/// - `Keep` = leave refresh_token unchanged
/// - `Clear` = clear the refresh_token (set to NULL)
/// - `Set(value)` = set a new refresh_token value
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshTokenUpdate {
    /// Leave the refresh token unchanged.
    Keep,
    /// Clear the refresh token (set to NULL).
    Clear,
    /// Set a new refresh token value.
    Set(String),
}

/// OAuth token entity model.
///
/// Stores OAuth2 tokens for email sources that use OAuth2 authentication.
/// Tokens are associated with a specific import source by name.
///
/// Note: Tokens are stored in plaintext in the database. For production deployments,
/// consider using database-level encryption or a secrets manager.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "oauth_tokens")]
pub struct Model {
    /// Name of the import source this token belongs to.
    #[sea_orm(primary_key, auto_increment = false)]
    pub source_name: String,

    /// OAuth2 provider (gmail, outlook, custom).
    pub provider: String,

    /// The current access token.
    pub access_token: String,

    /// The refresh token for obtaining new access tokens.
    pub refresh_token: Option<String>,

    /// When the access token expires.
    pub expires_at: DateTimeUtc,

    /// When this token record was created.
    pub created_at: DateTimeUtc,

    /// When this token record was last updated.
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Checks if the access token is expired or about to expire.
    ///
    /// Returns true if the token expires within the given buffer duration.
    /// Uses u64 for buffer_seconds since negative buffers don't make sense.
    pub fn is_expired(&self, buffer_seconds: u64) -> bool {
        let now = chrono::Utc::now();
        let buffer = Self::safe_duration_seconds(buffer_seconds);
        self.expires_at <= now + buffer
    }

    /// Checks if the token can be refreshed.
    pub fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }

    /// Safely converts expires_in_seconds to a Duration, clamping to prevent overflow.
    fn safe_duration_seconds(expires_in_seconds: u64) -> chrono::Duration {
        // Clamp to prevent i64 overflow (chrono::Duration::seconds takes i64)
        let clamped = expires_in_seconds.min(MAX_EXPIRES_IN_SECONDS);
        chrono::Duration::seconds(clamped as i64)
    }
}

/// Active model helpers for creating and updating tokens.
impl ActiveModel {
    /// Safely converts expires_in_seconds to a Duration, clamping to prevent overflow.
    fn safe_duration_seconds(expires_in_seconds: u64) -> chrono::Duration {
        // Clamp to prevent i64 overflow (chrono::Duration::seconds takes i64)
        let clamped = expires_in_seconds.min(MAX_EXPIRES_IN_SECONDS);
        chrono::Duration::seconds(clamped as i64)
    }

    /// Creates a new OAuth token active model.
    pub fn new_token(
        source_name: String,
        provider: String,
        access_token: String,
        refresh_token: Option<String>,
        expires_in_seconds: u64,
    ) -> Self {
        let now = chrono::Utc::now();
        let expires_at = now + Self::safe_duration_seconds(expires_in_seconds);

        Self {
            source_name: sea_orm::ActiveValue::Set(source_name),
            provider: sea_orm::ActiveValue::Set(provider),
            access_token: sea_orm::ActiveValue::Set(access_token),
            refresh_token: sea_orm::ActiveValue::Set(refresh_token),
            expires_at: sea_orm::ActiveValue::Set(expires_at),
            created_at: sea_orm::ActiveValue::Set(now),
            updated_at: sea_orm::ActiveValue::Set(now),
        }
    }

    /// Updates the access token and expiry.
    ///
    /// Use `RefreshTokenUpdate` to specify how the refresh token should be handled:
    /// - `RefreshTokenUpdate::Keep` = leave refresh_token unchanged
    /// - `RefreshTokenUpdate::Clear` = clear the refresh_token (set to NULL)
    /// - `RefreshTokenUpdate::Set(value)` = set a new refresh_token value
    pub fn update_access_token(
        mut self,
        access_token: String,
        refresh_token: RefreshTokenUpdate,
        expires_in_seconds: u64,
    ) -> Self {
        let now = chrono::Utc::now();
        let expires_at = now + Self::safe_duration_seconds(expires_in_seconds);

        self.access_token = sea_orm::ActiveValue::Set(access_token);
        match refresh_token {
            RefreshTokenUpdate::Keep => {}
            RefreshTokenUpdate::Clear => {
                self.refresh_token = sea_orm::ActiveValue::Set(None);
            }
            RefreshTokenUpdate::Set(value) => {
                self.refresh_token = sea_orm::ActiveValue::Set(Some(value));
            }
        }
        self.expires_at = sea_orm::ActiveValue::Set(expires_at);
        self.updated_at = sea_orm::ActiveValue::Set(now);
        self
    }
}
