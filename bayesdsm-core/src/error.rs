//! Typed errors for the bayesdsm-core crate.
//!
//! All STOP-rule violations are surfaced as `BayesDsmError::Stop` with a
//! `code` and `module`, which the CLI maps onto a `failures` row and a
//! non-zero exit code.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BayesDsmError {
    #[error("STOP in {module} ({code}): {message}")]
    Stop {
        module: String,
        code: String,
        message: String,
    },

    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid input: {0}")]
    Invalid(String),

    #[error("Missing required input: {0}")]
    MissingInput(String),

    #[error("Not initialized: {0}")]
    NotInitialized(String),
}

pub type Result<T> = std::result::Result<T, BayesDsmError>;
