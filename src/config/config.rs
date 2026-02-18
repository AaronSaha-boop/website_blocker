// src/config/config.rs

use std::path::PathBuf;
use serde::Deserialize;

use super::non_empty_vec::NonEmptyVec;
use super::error::ConfigError;

/// Raw config - mirrors TOML structure exactly
#[derive(Deserialize)]
struct RawConfig {
    hosts_file: String,
    socket_path: String,
    pid_path: String,
    blocked: Vec<String>,
}

/// Validated config - guaranteed valid if it exists
#[derive(Debug, PartialEq)]
pub struct Config {
    // TODO: strong types for paths
    pub hosts_file: PathBuf,
    pub socket_path: PathBuf,
    pub pid_path: PathBuf,
    pub blocked: NonEmptyVec<String>,
}

impl Config {
    pub fn from_toml(content: &str) -> Result<Self, ConfigError> {
        let raw: RawConfig = toml::from_str(content)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;
        
        let blocked = NonEmptyVec::new(raw.blocked)
            .ok_or(ConfigError::EmptyBlockedList)?;
        
        Ok(Config {
            hosts_file: expand_path(&raw.hosts_file)?,
            socket_path: expand_path(&raw.socket_path)?,
            pid_path: expand_path(&raw.pid_path)?,
            blocked,
        })
    }
    pub fn load() -> Result<Self, ConfigError> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| ConfigError::ParseError("No config directory found".into()))?;
        let config_path = config_dir.join("focusd").join("config.toml");
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| ConfigError::ParseError(format!("Failed to read {}: {}", config_path.display(), e)))?;
        Self::from_toml(&content)
    }
}

fn expand_path(path: &str) -> Result<PathBuf, ConfigError> {
    if let Some(stripped) = path.strip_prefix("~/") {
        let home = dirs::home_dir()
            .ok_or_else(|| ConfigError::ParseError("HOME directory not found".into()))?;
        Ok(home.join(stripped))
    } else {
        Ok(path.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_config_parses() {
        let toml = r#"
        hosts_file = "/etc/hosts"
        socket_path = "/tmp/focusd.sock"
        pid_path = "/tmp/focusd.pid"
        blocked = ["reddit.com", "youtube.com"]
        "#;
        let config = Config::from_toml(toml);
        assert!(config.is_ok());
        
        let config = config.unwrap();
        assert_eq!(config.blocked.as_slice().len(), 2);
    }

    #[test]
    fn empty_blocked_list_returns_error() {
        let toml = r#"
        hosts_file = "/etc/hosts"
        socket_path = "/tmp/focusd.sock"
        pid_path = "/tmp/focusd.pid"
        blocked = []
        "#;
        let config = Config::from_toml(toml);
        assert_eq!(config, Err(ConfigError::EmptyBlockedList));
    }

    #[test]
    fn invalid_toml_returns_parse_error() {
        let toml = "this is not valid toml {{{{";
        let config = Config::from_toml(toml);
        assert!(matches!(config, Err(ConfigError::ParseError(_))));
    }

    #[test]
    fn missing_field_returns_parse_error() {
        let toml = r#"
        hosts_file = "/etc/hosts"
        "#;
        let config = Config::from_toml(toml);
        assert!(matches!(config, Err(ConfigError::ParseError(_))));
    }

    #[test]
    fn tilde_paths_are_expanded() {
        let toml = r#"
        hosts_file = "~/hosts"
        socket_path = "~/focusd.sock"
        pid_path = "~/focusd.pid"
        blocked = ["reddit.com"]
        "#;
        let config = Config::from_toml(toml).unwrap();
        
        // Should NOT contain tilde
        assert!(!config.hosts_file.to_string_lossy().contains('~'));
        // Should contain home directory
        assert!(config.hosts_file.to_string_lossy().len() > 7);
    }
}