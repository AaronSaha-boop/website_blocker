use regex::Regex;
use std::sync::LazyLock;

static DURATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:(\d+)h)?(?:(\d+)m)?(?:(\d+)s)?$").unwrap()
});

#[derive(Debug, PartialEq)]
pub enum ParseError {
    Empty,
    InvalidFormat,
}

pub fn parse_time_to_seconds(input: &str) -> Result<u64, ParseError> {
    if input.is_empty() {
        return Err(ParseError::Empty);
    }
    
    // Anchored regex — must match entire input
    
    let caps = DURATION_RE.captures(input).ok_or(ParseError::InvalidFormat)?;
    
    let hours: u64 = caps.get(1).map_or(0, |m| m.as_str().parse().unwrap_or(0));
    let minutes: u64 = caps.get(2).map_or(0, |m| m.as_str().parse().unwrap_or(0));
    let seconds: u64 = caps.get(3).map_or(0, |m| m.as_str().parse().unwrap_or(0));

    // Ensure at least one component was captured
    let has_match = caps.get(1).is_some() || caps.get(2).is_some() || caps.get(3).is_some();
    if !has_match {
        return Err(ParseError::InvalidFormat);
    }

    Ok((hours * 3600) + (minutes * 60) + seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn just_hour() {
        assert_eq!(parse_time_to_seconds("2h"), Ok(7200));
    }

    #[test]
    fn just_minute() {
        assert_eq!(parse_time_to_seconds("30m"), Ok(1800));
    }

    #[test]
    fn just_second() {
        assert_eq!(parse_time_to_seconds("15s"), Ok(15));
    }

    #[test]
    fn hour_and_minute() {
        assert_eq!(parse_time_to_seconds("2h30m"), Ok(9000));
    }

    #[test]
    fn hour_and_second() {
        assert_eq!(parse_time_to_seconds("2h15s"), Ok(7215));
    }

    #[test]
    fn minute_and_second() {
        assert_eq!(parse_time_to_seconds("45m30s"), Ok(2730));
    }

    #[test] 
    fn hour_minute_second() {
        assert_eq!(parse_time_to_seconds("1h1m1s"), Ok(3661));
    }

    #[test]
    fn empty_string_returns_error() {
        assert_eq!(parse_time_to_seconds(""), Err(ParseError::Empty));
    }

    #[test]
    fn invalid_format_returns_error() {
        assert_eq!(parse_time_to_seconds("abc"), Err(ParseError::InvalidFormat));
    }

    #[test]
    fn number_without_unit_returns_error() {
        assert_eq!(parse_time_to_seconds("30"), Err(ParseError::InvalidFormat));
    }

    #[test]
    fn trailing_garbage_returns_error() {
        assert_eq!(parse_time_to_seconds("1h30mxyz"), Err(ParseError::InvalidFormat));
    }
    
}