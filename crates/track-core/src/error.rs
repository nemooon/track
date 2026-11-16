use std::fmt;

/// Errors carry the stable string codes the CLI prints and the skills match on
/// (`invalid_input`, `exact_duplicate`, ...).
#[derive(Debug)]
pub enum CoreError {
    Code { code: &'static str, message: String, details: serde_json::Value },
    Sql(rusqlite::Error),
    Io(std::io::Error),
}

impl CoreError {
    pub fn code_msg(code: &'static str, message: impl Into<String>) -> Self {
        Self::Code { code, message: message.into(), details: serde_json::Value::Null }
    }

    pub fn with_details(code: &'static str, message: impl Into<String>, details: serde_json::Value) -> Self {
        Self::Code { code, message: message.into(), details }
    }

    pub fn code(&self) -> &str {
        match self {
            Self::Code { code, .. } => code,
            Self::Sql(_) => "database_error",
            Self::Io(_) => "io_error",
        }
    }

    pub fn details(&self) -> serde_json::Value {
        match self {
            Self::Code { details, .. } => details.clone(),
            _ => serde_json::Value::Null,
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Code { message, .. } => f.write_str(message),
            Self::Sql(e) => write!(f, "{e}"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<rusqlite::Error> for CoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sql(e)
    }
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
