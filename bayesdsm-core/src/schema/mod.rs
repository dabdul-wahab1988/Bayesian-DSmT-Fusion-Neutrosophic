//! SQL schema migrations, included as `&str` constants.

pub const MIGRATION_0001: &str = include_str!("0001_init.sql");
pub const MIGRATION_0002: &str = include_str!("0002_views.sql");
