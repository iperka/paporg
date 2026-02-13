//! Email attachment import source module.
//!
//! This module provides functionality for importing document attachments from email
//! accounts via IMAP. It supports both password and OAuth2 authentication.

pub mod client;
pub mod device_auth;
pub mod error;
pub mod parser;
pub mod scanner;
pub mod tracker;

pub use client::ImapClient;
pub use device_auth::{DeviceCodeResponse, DeviceFlowAuth, OAuth2Provider, TokenResponse};
pub use error::EmailError;
pub use parser::{EmailParser, ExtractedAttachment};
pub use scanner::EmailSourceScanner;
pub use tracker::EmailTracker;
