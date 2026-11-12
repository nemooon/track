//! Shared core for the Track GPUI app and CLI: SQLite storage, validation, reports, AI.
pub mod ai;
pub mod ai_registration;
pub mod dates;
pub mod db;
pub mod error;
pub mod guard;
pub mod integrations;
pub mod ids;
pub mod migrate;
pub mod prepare;
pub mod model;
pub mod report;
pub mod repo;
pub mod reports;
pub mod runtime;
pub mod validate;
pub mod time;

pub use db::Db;
pub use error::CoreError;
#[cfg(test)]
mod db_tests;
#[cfg(test)]
mod reports_tests;
#[cfg(test)]
mod integrations_tests;
