use thiserror::Error;

#[derive(Error, Debug)]
pub enum OctError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSer(String),

    #[error("token invalid: {0}")]
    TokenInvalid(String),

    #[error("not initialized: run `oct init` first")]
    NotInitialized,

    #[error("no opencode config found")]
    NoConfigFound,

    #[error("server error: {code} {message}")]
    ServerError { code: String, message: String },

    #[error("path security violation: {0}")]
    PathSecurity(String),

    #[error("backup error: {0}")]
    Backup(String),

    #[error("bundle error: {0}")]
    Bundle(String),

    #[error("config error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, OctError>;
