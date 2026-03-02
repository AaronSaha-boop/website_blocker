//pub fn lines(&self) -> Lines<'_>

const MARKER_START: &str = "# --- focusd start ---";
const MARKER_END: &str = "# --- focusd end ---";

/// Check if the hosts content contains our managed block
pub fn has_blocked_hosts(content: &str) -> bool {
    content.contains(MARKER_START)
}

pub fn add_blocked_hosts(content: &str, hosts: &[String]) -> String {
    let mut result = remove_blocked_hosts(content);
    
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }

    result.push_str(MARKER_START);
    result.push('\n');

    for host in hosts {
        result.push_str(&format!("127.0.0.1 {}\n", host));
        if !host.starts_with("www.") {
            result.push_str(&format!("127.0.0.1 www.{}\n", host));
        }
    }

    result.push_str(MARKER_END);
    result.push('\n');

    result
}

pub fn remove_blocked_hosts(content: &str) -> String {
    let mut result = String::new();
    let mut skip = false;

    for line in content.lines() {
        if line.trim() == MARKER_START {
            skip = true;
            continue;
        }
        
        if skip {
            if line.trim() == MARKER_END {
                skip = false;
            }
            continue;
        }
        
        result.push_str(line);
        result.push('\n');
    }

    // Remove trailing newline if original didn't have one
    if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_content_returns_false() {
        assert!(!has_blocked_hosts(""));
    }

    #[test]
    fn content_without_markers_returns_false() {
        let content = "127.0.0.1 localhost\n::1 localhost";
        assert!(!has_blocked_hosts(content));
    }

    #[test]
    fn content_with_markers_returns_true() {
        let content = "127.0.0.1 localhost\n# --- focusd start ---\n127.0.0.1\treddit.com\n# --- focusd end ---";
        assert!(has_blocked_hosts(content));
    }

    #[test]
    fn add_blocked_hosts_empty_content() {
        let hosts = vec!["example.com".to_string()];
        let result = add_blocked_hosts("", &hosts);
        
        assert!(result.contains(MARKER_START));
        assert!(result.contains(MARKER_END));
        assert!(result.contains("127.0.0.1 example.com"));
        assert!(result.contains("127.0.0.1 www.example.com"));
    }

    #[test]
    fn add_blocked_hosts_appends_to_existing() {
        let content = "127.0.0.1 localhost\n";
        let hosts = vec!["example.com".to_string()];
        let result = add_blocked_hosts(content, &hosts);
        
        assert!(result.starts_with("127.0.0.1 localhost\n"));
        assert!(result.contains(MARKER_START));
        assert!(result.contains("127.0.0.1 example.com"));
    }

    #[test]
    fn add_blocked_hosts_handles_www_variants() {
        let hosts = vec!["example.com".to_string(), "www.reddit.com".to_string()];
        let result = add_blocked_hosts("", &hosts);
        
        assert!(result.contains("127.0.0.1 example.com"));
        assert!(result.contains("127.0.0.1 www.example.com"));
        assert!(result.contains("127.0.0.1 www.reddit.com"));
        // Should not double add www
        assert!(!result.contains("127.0.0.1 www.www.reddit.com"));
    }

    #[test]
    fn remove_blocked_hosts_no_markers() {
        let content = "some content\nwithout markers";
        assert_eq!(remove_blocked_hosts(content), content);
    }

    #[test]
    fn remove_blocked_hosts_removes_block() {
        let content = format!("prefix\n{}\nblocked\n{}\nsuffix", MARKER_START, MARKER_END);
        let result = remove_blocked_hosts(&content);
        assert_eq!(result.trim(), "prefix\nsuffix");
        assert!(!result.contains(MARKER_START));
        assert!(!result.contains(MARKER_END));
    }

    #[test]
    fn remove_blocked_hosts_preserves_surrounding() {
        let content = format!("pre\n{}\nblock\n{}\npost", MARKER_START, MARKER_END);
        let result = remove_blocked_hosts(&content);
        assert!(result.contains("pre"));
        assert!(result.contains("post"));
        assert!(!result.contains("block"));
    }

    #[test]
    fn add_then_remove_roundtrip() {
        let original = "127.0.0.1 localhost\n";
        let hosts = vec!["reddit.com".to_string()];
        
        let added = add_blocked_hosts(original, &hosts);
        let removed = remove_blocked_hosts(&added);
        
        assert_eq!(removed, original);
    }

    #[test]
    fn add_twice_only_one_block() {
        let content = "127.0.0.1 localhost\n";
        let hosts = vec!["reddit.com".to_string()];
        
        let once = add_blocked_hosts(content, &hosts);
        let twice = add_blocked_hosts(&once, &hosts);
        
        // Should only have one marker block
        assert_eq!(once, twice);
    }
}
