//! Shared core for the Track GPUI app and CLI: SQLite storage, validation, reports, AI.
pub mod ai_registration;
pub mod db;
pub mod error;
pub mod guard;
pub mod ids;
pub mod migrate;
pub mod model;
pub mod repo;
pub mod validate;
pub mod time;

pub use db::Db;
pub use error::CoreError;
#[cfg(test)]
mod db_tests;
