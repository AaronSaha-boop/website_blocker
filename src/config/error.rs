use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum ConfigError {
    #[error("Config parse error: {0}")]
    ParseError(String),
    #[error("Blocked list cannot be empty")]
    EmptyBlockedList,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_error_is_debug_and_partial_eq() {
        let err1 = ConfigError::EmptyBlockedList;
        let err2 = ConfigError::EmptyBlockedList;
        assert_eq!(err1, err2);
        println!("{:?}", err1);
    }
}
