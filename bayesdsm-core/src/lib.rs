//! bayesdsm-core
//!
//! Reproducible Bayesian-DSmT-Neutrosophic sediment-metal hotspot prioritization.
//! SQLite is the single source of truth.

pub mod audit;
pub mod bayes;
pub mod belief;
pub mod clean;
pub mod db;
pub mod dsmt;
pub mod error;
pub mod export;
pub mod features;
pub mod ingest;
pub mod math;
pub mod neutrosophic;
pub mod schema;
pub mod validate;

pub use error::{BayesDsmError, Result};
