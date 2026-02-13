//! Database entities.

pub mod job;
pub mod oauth_token;
pub mod processed_email;

pub use job::Entity as Job;
pub use oauth_token::Entity as OauthToken;
pub use processed_email::Entity as ProcessedEmail;
