use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Configuration for the repository monitor.
#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct MonitorConfig {
    /// Path to the local repository to monitor.
    pub repository_path: String,
    /// Maximum number of retries for the sync command.
    pub max_retries: u32,
    /// Interval in seconds between git pull attempts.
    pub pull_interval_sec: u64,
    /// Server IP.
    pub server_ip: String,
    /// Server port.
    pub server_port: u16,
    /// Branches to monitor.
    pub branches: Vec<String>,
    /// Maximum history days.
    pub max_history_days: u32,
}

impl MonitorConfig {
    /// Loads the configuration from the specified JSON and TOML files.
    pub fn load(monitor_config_path: &str, settings_toml_path: &str) -> Result<Self> {
        let builder = config::Config::builder()
            .add_source(config::File::new(settings_toml_path, config::FileFormat::Toml).required(false))
            .add_source(config::File::new(monitor_config_path, config::FileFormat::Json).required(false));
            
        let cfg = builder.build()?;
        
        let repository_path = cfg.get_string("git.repository_path").unwrap_or_default();
        let max_retries = cfg.get_int("max_retries").unwrap_or(3) as u32;
        let pull_interval_sec = cfg.get_int("pull_interval_sec").unwrap_or(10) as u64; // Default to 10 seconds per spec
        let server_ip = cfg.get_string("server_ip").unwrap_or_else(|_| "127.0.0.1".to_string());
        let server_port = cfg.get_int("server_port").unwrap_or(8080) as u16;
        
        // Parse branches array
        let branches = if let Ok(val) = cfg.get_array("branches") {
            val.into_iter()
                .filter_map(|v| v.into_string().ok())
                .collect()
        } else {
            vec!["main".to_string()]
        };

        let max_history_days = cfg.get_int("max_history_days").unwrap_or(180) as u32;

        Ok(MonitorConfig {
            repository_path,
            max_retries,
            pull_interval_sec,
            server_ip,
            server_port,
            branches,
            max_history_days,
        })
    }

    /// Returns the absolute path to the repository.
    /// Assuming current_dir is the sashiko root.
    pub fn get_absolute_repo_path(&self) -> Result<PathBuf> {
        let path = Path::new(&self.repository_path);
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(std::env::current_dir()?.join(path))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_load_config() {
        let dir = tempdir().unwrap();
        let monitor_cfg_path = dir.path().join("monitor_config.json");
        let settings_path = dir.path().join("Settings.toml");

        fs::write(&monitor_cfg_path, r#"
        {
            "max_retries": 5,
            "pull_interval_sec": 3600,
            "max_history_days": 90,
            "branches": ["main", "feature-x"]
        }
        "#).unwrap();

        fs::write(&settings_path, r#"
        [git]
        repository_path = "my_kernel_repo"
        "#).unwrap();

        let cfg = MonitorConfig::load(
            monitor_cfg_path.to_str().unwrap(),
            settings_path.to_str().unwrap(),
        ).unwrap();

        assert_eq!(cfg.max_retries, 5);
        assert_eq!(cfg.pull_interval_sec, 3600);
        assert_eq!(cfg.repository_path, "my_kernel_repo");
        assert_eq!(cfg.max_history_days, 90);
        assert_eq!(cfg.branches, vec!["main".to_string(), "feature-x".to_string()]);
    }

    #[test]
    fn test_absolute_repo_path() {
        let cfg = MonitorConfig {
            repository_path: "my_kernel_repo".to_string(),
            max_retries: 3,
            pull_interval_sec: 3600,
            server_ip: "127.0.0.1".to_string(),
            server_port: 8080,
            branches: vec!["main".to_string()],
            max_history_days: 180,
        };

        let path = cfg.get_absolute_repo_path().unwrap();
        assert!(path.is_absolute());
        assert!(path.to_string_lossy().contains("my_kernel_repo"));
    }
}
