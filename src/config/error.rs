
#[derive(Debug, PartialEq)]
pub enum ConfigError {
    ParseError(String),
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