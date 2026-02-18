use time_hms::TimeHms;

pub fn format_duration(seconds: u64) -> String {
    let t = TimeHms::new(seconds);
    let mut result = String::new();
    
    if t.h() > 0 {
        result.push_str(&format!("{}h", t.h())); 
    }
    if t.m() > 0 {
        result.push_str(&format!("{}m", t.m()));
    }
    if t.s() > 0 {
        result.push_str(&format!("{}s", t.s()));
    }
    if result.is_empty() {
        result.push_str("0s");
    }
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn just_hour() {
        assert_eq!(format_duration(7200), "2h");
    }

    #[test]
    fn just_minute() {
        assert_eq!(format_duration(60), "1m");
    }

    #[test]
    fn just_second() {
        assert_eq!(format_duration(1), "1s");
    }

    #[test]
    fn hour_and_minute() {
        assert_eq!(format_duration(7260), "2h1m");
    }

    #[test]
    fn hour_and_second() {
        assert_eq!(format_duration(7201), "2h1s");
    }

    #[test]
    fn minute_and_second() {
        assert_eq!(format_duration(61), "1m1s");
    }

    #[test]
    fn hour_minute_second() {
        assert_eq!(format_duration(3661), "1h1m1s");
    }

    #[test]
    fn zero_returns_0s() {
        assert_eq!(format_duration(0), "0s");
    }
}