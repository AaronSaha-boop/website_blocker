// src/config/config.rs

use std::path::PathBuf;
use serde::Deserialize;

use super::error::ConfigError;

/// Raw config — mirrors TOML structure exactly.
#[derive(Deserialize)]
struct RawConfig {
    hosts_file: String,
    socket_path: String,
    pid_path: String,
    db_path: String,
}

/// Validated config — guaranteed valid if it exists.
///
/// Contains only infrastructure paths. User-facing rules
/// (blocked websites, DOM rules, app rules) live in the database.
#[derive(Debug, PartialEq)]
pub struct Config {
    pub hosts_file: PathBuf,
    pub socket_path: PathBuf,
    pub pid_path: PathBuf,
    pub db_path: PathBuf,
}

impl Config {
    pub fn from_toml(content: &str) -> Result<Self, ConfigError> {
        let raw: RawConfig = toml::from_str(content)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;

        Ok(Config {
            hosts_file: expand_path(&raw.hosts_file)?,
            socket_path: expand_path(&raw.socket_path)?,
            pid_path: expand_path(&raw.pid_path)?,
            db_path: expand_path(&raw.db_path)?,
        })
    }

    pub fn load() -> Result<Self, ConfigError> {
        let config_path = PathBuf::from("/usr/local/etc/dark-pattern/config.toml");
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| ConfigError::ParseError(
                format!("Failed to read {}: {}", config_path.display(), e),
            ))?;
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

    const VALID_TOML: &str = r#"
        hosts_file = "/etc/hosts"
        socket_path = "/tmp/dark-pattern.sock"
        pid_path = "/tmp/dark-pattern.pid"
        db_path = "/tmp/dark-pattern.db"
    "#;

    #[test]
    fn valid_config_parses() {
        let config = Config::from_toml(VALID_TOML);
        assert!(config.is_ok());
    }

    #[test]
    fn invalid_toml_returns_parse_error() {
        let config = Config::from_toml("this is not valid toml {{{{");
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
            socket_path = "~/dark-pattern.sock"
            pid_path = "~/dark-pattern.pid"
            db_path = "~/dark-pattern.db"
        "#;
        let config = Config::from_toml(toml).unwrap();

        assert!(!config.hosts_file.to_string_lossy().contains('~'));
        assert!(config.hosts_file.to_string_lossy().len() > 7);
    }
}
