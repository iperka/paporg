//! Shared test utilities for paporg integration tests.
//!
//! This module provides:
//! - `TestHarness` for isolated test execution with temp directories
//! - Builder patterns for creating test configurations programmatically

pub mod builders;
pub mod harness;

pub use builders::*;
pub use harness::TestHarness;
