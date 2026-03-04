use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Default names that are allowed (common drop-guard patterns).
pub const DEFAULT_ALLOW_NAMES: &[&str] = &[
    "_guard",
    "_lock",
    "_handle",
    "_permit",
    "_subscription",
    "_span",
    "_enter",
    "_timer",
    "_tempdir",
    "_tempfile",
    "_dropper",
];

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    /// Paths to scan (default: ["src/"])
    pub paths: Vec<PathBuf>,
    /// Glob patterns for paths to exclude
    pub exclude_paths: Vec<String>,
    /// Exact variable names to allow
    pub allow_names: Vec<String>,
    /// Regex patterns to allow
    pub allow_patterns: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            paths: vec![PathBuf::from("src/")],
            exclude_paths: Vec::new(),
            allow_names: Vec::new(),
            allow_patterns: Vec::new(),
        }
    }
}

impl Config {
    /// Load configuration with fallback chain.
    pub fn load(config_path: Option<&PathBuf>) -> Result<Self> {
        // If explicit config path provided, try to load it
        if let Some(path) = config_path {
            return Self::load_from_file(path).context(format!("Failed to load config from {}", path.display()));
        }

        // Try primary location: ~/.config/lint-unused/lint-unused.yml
        if let Some(config_dir) = dirs::config_dir() {
            let project_name = env!("CARGO_PKG_NAME");
            let primary_config = config_dir.join(project_name).join(format!("{}.yml", project_name));
            if primary_config.exists() {
                match Self::load_from_file(&primary_config) {
                    Ok(config) => return Ok(config),
                    Err(e) => {
                        log::warn!("Failed to load config from {}: {}", primary_config.display(), e);
                    }
                }
            }
        }

        // Try fallback location: ./lint-unused.yml
        let project_name = env!("CARGO_PKG_NAME");
        let fallback_config = PathBuf::from(format!("{}.yml", project_name));
        if fallback_config.exists() {
            match Self::load_from_file(&fallback_config) {
                Ok(config) => return Ok(config),
                Err(e) => {
                    log::warn!("Failed to load config from {}: {}", fallback_config.display(), e);
                }
            }
        }

        // No config file found, use defaults
        log::info!("No config file found, using defaults");
        Ok(Self::default())
    }

    fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(&path).context("Failed to read config file")?;
        let config: Self = serde_yaml::from_str(&content).context("Failed to parse config file")?;
        log::info!("Loaded config from: {}", path.as_ref().display());
        Ok(config)
    }

    /// Get effective allow_names, including defaults unless disabled.
    pub fn effective_allow_names(&self, include_defaults: bool) -> Vec<String> {
        let mut names = self.allow_names.clone();
        if include_defaults {
            for name in DEFAULT_ALLOW_NAMES {
                let s = (*name).to_string();
                if !names.contains(&s) {
                    names.push(s);
                }
            }
        }
        names
    }
}
