// src/config/error.rs

use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum ConfigError {
    #[error("Config parse error: {0}")]
    ParseError(String),
}
