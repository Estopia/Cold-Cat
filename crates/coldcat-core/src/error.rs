use thiserror::Error;

pub type Result<T> = std::result::Result<T, ColdCatError>;

#[derive(Debug, Error)]
pub enum ColdCatError {
    #[error("invalid duration: {0}")]
    InvalidDuration(String),
    #[error("invalid rule: {0}")]
    InvalidRule(String),
    #[error("state has been tampered with or was signed by a different key")]
    StateTampered,
    #[error("unsupported sealed state version: {0}")]
    UnsupportedStateVersion(u32),
    #[error("malformed hosts managed block")]
    MalformedHostsBlock,
    #[error("system clock is before the Unix epoch")]
    SystemTime,
    #[error("I/O failed: {0}")]
    Io(String),
    #[error("serialization failed: {0}")]
    Serialization(String),
    #[error("daemon protocol failed: {0}")]
    Protocol(String),
    #[error("cryptographic operation failed")]
    Crypto,
}

impl From<serde_json::Error> for ColdCatError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value.to_string())
    }
}

impl From<std::io::Error> for ColdCatError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<base64::DecodeError> for ColdCatError {
    fn from(_: base64::DecodeError) -> Self {
        Self::StateTampered
    }
}
