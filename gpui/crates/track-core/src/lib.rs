//! Shared core for the Track GPUI app and CLI: SQLite storage, validation, reports, AI.
pub mod db;
pub mod error;
pub mod guard;
pub mod ids;
pub mod migrate;
pub mod time;

pub use db::Db;
pub use error::CoreError;
#[cfg(test)]
mod db_tests;
