use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use crate::error::RgitError;

/// Main configuration structure for rgit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// UI and display preferences
    pub ui: UiConfig,
    /// Git operation defaults
    pub git: GitConfig,
    /// Submodule management settings
    pub submodules: SubmoduleConfig,
    /// Integration settings
    pub integrations: IntegrationConfig,
    /// User preferences
    pub user: UserConfig,
    /// Advanced settings
    pub advanced: AdvancedConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Use colored output
    pub colors: bool,
    /// Color theme (dark, light, auto)
    pub theme: String,
    /// Show progress bars
    pub progress: bool,
    /// Use emoji icons
    pub icons: bool,
    /// Interactive prompts enabled
    pub interactive: bool,
    /// Default editor for commit messages
    pub editor: Option<String>,
    /// Terminal width override
    pub width: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    /// Default remote name
    pub default_remote: String,
    /// Default branch name for new repositories
    pub default_branch: String,
    /// Auto-stage on commit
    pub auto_stage: bool,
    /// Sign commits by default
    pub sign_commits: bool,
    /// Push tags with branches
    pub push_tags: bool,
    /// Rebase instead of merge on pull
    pub pull_rebase: bool,
    /// Prune on fetch
    pub auto_prune: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmoduleConfig {
    /// Auto-initialize submodules on clone
    pub auto_init: bool,
    /// Update submodules recursively by default
    pub recursive: bool,
    /// Check submodule health before operations
    pub health_check: bool,
    /// Auto-stash submodule changes
    pub auto_stash: bool,
    /// Parallel submodule operations
    pub parallel: bool,
    /// Maximum parallel jobs
    pub max_jobs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationConfig {
    /// External diff tool
    pub diff_tool: Option<String>,
    /// External merge tool
    pub merge_tool: Option<String>,
    /// GPG signing configuration
    pub gpg: GpgConfig,
    /// Hooks configuration
    pub hooks: HooksConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpgConfig {
    /// Enable GPG signing
    pub enabled: bool,
    /// GPG key ID
    pub key_id: Option<String>,
    /// GPG program path
    pub program: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    /// Enable pre-commit hooks
    pub pre_commit: bool,
    /// Enable commit-msg hooks
    pub commit_msg: bool,
    /// Enable pre-push hooks
    pub pre_push: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    /// User's name
    pub name: Option<String>,
    /// User's email
    pub email: Option<String>,
    /// Preferred language
    pub language: String,
    /// Timezone
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConfig {
    /// Enable verbose logging
    pub verbose: bool,
    /// Log level (error, warn, info, debug, trace)
    pub log_level: String,
    /// Cache settings
    pub cache: CacheConfig,
    /// Performance tuning
    pub performance: PerformanceConfig,
    /// Safety settings
    pub safety: SafetyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable caching
    pub enabled: bool,
    /// Cache directory
    pub directory: Option<PathBuf>,
    /// Cache TTL in seconds
    pub ttl: u64,
    /// Maximum cache size in MB
    pub max_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Number of parallel threads
    pub threads: usize,
    /// Buffer size for I/O operations
    pub buffer_size: usize,
    /// Enable memory mapping for large files
    pub use_mmap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfig {
    /// Require confirmation for destructive operations
    pub confirm_destructive: bool,
    /// Auto-backup before major operations
    pub auto_backup: bool,
    /// Maximum backup retention days
    pub backup_retention: u32,
    /// Prevent force push without --force-with-lease
    pub safe_force_push: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ui: UiConfig::default(),
            git: GitConfig::default(),
            submodules: SubmoduleConfig::default(),
            integrations: IntegrationConfig::default(),
            user: UserConfig::default(),
            advanced: AdvancedConfig::default(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            colors: true,
            theme: "auto".to_string(),
            progress: true,
            icons: true,
            interactive: true,
            editor: std::env::var("EDITOR").ok(),
            width: None,
        }
    }
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            default_remote: "origin".to_string(),
            default_branch: "main".to_string(),
            auto_stage: false,
            sign_commits: false,
            push_tags: false,
            pull_rebase: false,
            auto_prune: true,
        }
    }
}

impl Default for SubmoduleConfig {
    fn default() -> Self {
        Self {
            auto_init: true,
            recursive: true,
            health_check: true,
            auto_stash: false,
            parallel: true,
            max_jobs: num_cpus::get().min(8),
        }
    }
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            diff_tool: None,
            merge_tool: None,
            gpg: GpgConfig::default(),
            hooks: HooksConfig::default(),
        }
    }
}

impl Default for GpgConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            key_id: None,
            program: which::which("gpg").ok().map(|p| p.display().to_string()),
        }
    }
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            pre_commit: true,
            commit_msg: true,
            pre_push: true,
        }
    }
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            name: None,
            email: None,
            language: "en".to_string(),
            timezone: None,
        }
    }
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            verbose: false,
            log_level: "info".to_string(),
            cache: CacheConfig::default(),
            performance: PerformanceConfig::default(),
            safety: SafetyConfig::default(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: None, // Will be set to platform-specific cache dir
            ttl: 3600, // 1 hour
            max_size: 100, // 100 MB
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            threads: num_cpus::get(),
            buffer_size: 8192,
            use_mmap: true,
        }
    }
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            confirm_destructive: true,
            auto_backup: true,
            backup_retention: 30,
            safe_force_push: true,
        }
    }
}

impl Config {
    /// Load configuration from default locations
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;
        
        if config_path.exists() {
            Self::load_from_file(&config_path)
        } else {
            debug!("No configuration file found, using defaults");
            let config = Self::default();
            config.ensure_directories()?;
            Ok(config)
        }
    }

    /// Load configuration from a specific file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;
        
        let mut config: Config = toml::from_str(&content)
            .with_context(|| "Failed to parse configuration file")?;
        
        config.apply_environment_overrides();
        config.ensure_directories()?;
        config.validate()?;
        
        debug!("Loaded configuration from {}", path.as_ref().display());
        Ok(config)
    }

    /// Save configuration to default location
    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;
        self.save_to_file(&config_path)
    }

    /// Save configuration to a specific file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)
            .context("Failed to serialize configuration")?;
        
        fs::write(path.as_ref(), content)
            .with_context(|| format!("Failed to write config file: {}", path.as_ref().display()))?;
        
        debug!("Saved configuration to {}", path.as_ref().display());
        Ok(())
    }

    /// Get the default configuration file path
    pub fn get_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| RgitError::ConfigurationError("Cannot determine config directory".to_string()))?;
        
        Ok(config_dir.join("rgit").join("config.toml"))
    }

    /// Get the cache directory path
    pub fn get_cache_dir(&self) -> Result<PathBuf> {
        if let Some(ref dir) = self.advanced.cache.directory {
            Ok(dir.clone())
        } else {
            let cache_dir = dirs::cache_dir()
                .ok_or_else(|| RgitError::ConfigurationError("Cannot determine cache directory".to_string()))?;
            Ok(cache_dir.join("rgit"))
        }
    }

    /// Get the data directory path
    pub fn get_data_dir() -> Result<PathBuf> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| RgitError::ConfigurationError("Cannot determine data directory".to_string()))?;
        Ok(data_dir.join("rgit"))
    }

    /// Apply environment variable overrides
    fn apply_environment_overrides(&mut self) {
        // UI overrides
        if let Ok(value) = std::env::var("RGIT_NO_COLOR") {
            if value == "1" || value.to_lowercase() == "true" {
                self.ui.colors = false;
            }
        }

        if let Ok(theme) = std::env::var("RGIT_THEME") {
            self.ui.theme = theme;
        }

        if let Ok(editor) = std::env::var("RGIT_EDITOR") {
            self.ui.editor = Some(editor);
        }

        // Git overrides
        if let Ok(remote) = std::env::var("RGIT_DEFAULT_REMOTE") {
            self.git.default_remote = remote;
        }

        if let Ok(branch) = std::env::var("RGIT_DEFAULT_BRANCH") {
            self.git.default_branch = branch;
        }

        // Advanced overrides
        if let Ok(value) = std::env::var("RGIT_VERBOSE") {
            if value == "1" || value.to_lowercase() == "true" {
                self.advanced.verbose = true;
            }
        }

        if let Ok(level) = std::env::var("RGIT_LOG_LEVEL") {
            self.advanced.log_level = level;
        }
    }

    /// Ensure required directories exist
    fn ensure_directories(&self) -> Result<()> {
        // Create config directory
        if let Ok(config_path) = Self::get_config_path() {
            if let Some(parent) = config_path.parent() {
                fs::create_dir_all(parent)?;
            }
        }

        // Create cache directory
        let cache_dir = self.get_cache_dir()?;
        fs::create_dir_all(&cache_dir)?;

        // Create data directory
        let data_dir = Self::get_data_dir()?;
        fs::create_dir_all(&data_dir)?;

        Ok(())
    }

    /// Validate configuration settings
    fn validate(&self) -> Result<()> {
        // Validate theme
        if !["auto", "dark", "light"].contains(&self.ui.theme.as_str()) {
            return Err(RgitError::InvalidConfigValue {
                key: "ui.theme".to_string(),
                value: self.ui.theme.clone(),
            }.into());
        }

        // Validate log level
        if !["error", "warn", "info", "debug", "trace"].contains(&self.advanced.log_level.as_str()) {
            return Err(RgitError::InvalidConfigValue {
                key: "advanced.log_level".to_string(),
                value: self.advanced.log_level.clone(),
            }.into());
        }

        // Validate performance settings
        if self.advanced.performance.threads == 0 {
            return Err(RgitError::InvalidConfigValue {
                key: "advanced.performance.threads".to_string(),
                value: "0".to_string(),
            }.into());
        }

        if self.submodules.max_jobs == 0 {
            return Err(RgitError::InvalidConfigValue {
                key: "submodules.max_jobs".to_string(),
                value: "0".to_string(),
            }.into());
        }

        Ok(())
    }

    /// Merge with another configuration (other takes precedence)
    pub fn merge(&mut self, other: &Config) {
        // UI settings
        if !other.ui.colors { self.ui.colors = false; }
        if other.ui.theme != "auto" { self.ui.theme = other.ui.theme.clone(); }
        if !other.ui.progress { self.ui.progress = false; }
        if !other.ui.icons { self.ui.icons = false; }
        if !other.ui.interactive { self.ui.interactive = false; }
        if other.ui.editor.is_some() { self.ui.editor = other.ui.editor.clone(); }
        if other.ui.width.is_some() { self.ui.width = other.ui.width; }

        // Git settings
        if other.git.default_remote != "origin" { self.git.default_remote = other.git.default_remote.clone(); }
        if other.git.default_branch != "main" { self.git.default_branch = other.git.default_branch.clone(); }
        if other.git.auto_stage { self.git.auto_stage = true; }
        if other.git.sign_commits { self.git.sign_commits = true; }
        if other.git.push_tags { self.git.push_tags = true; }
        if other.git.pull_rebase { self.git.pull_rebase = true; }
        if !other.git.auto_prune { self.git.auto_prune = false; }

        // Advanced settings
        if other.advanced.verbose { self.advanced.verbose = true; }
        if other.advanced.log_level != "info" { self.advanced.log_level = other.advanced.log_level.clone(); }
    }

    /// Get user identity from configuration and git config
    pub fn get_user_identity(&self) -> Result<(String, String)> {
        let name = self.user.name.clone()
            .or_else(|| std::env::var("GIT_AUTHOR_NAME").ok())
            .or_else(|| std::env::var("GIT_COMMITTER_NAME").ok())
            .ok_or_else(|| RgitError::UserIdentityNotConfigured)?;

        let email = self.user.email.clone()
            .or_else(|| std::env::var("GIT_AUTHOR_EMAIL").ok())
            .or_else(|| std::env::var("GIT_COMMITTER_EMAIL").ok())
            .ok_or_else(|| RgitError::UserIdentityNotConfigured)?;

        Ok((name, email))
    }

    /// Check if interactive mode is available
    pub fn is_interactive(&self) -> bool {
        self.ui.interactive && atty::is(atty::Stream::Stdin)
    }

    /// Get terminal width
    pub fn terminal_width(&self) -> usize {
        self.ui.width.unwrap_or_else(|| {
            terminal_size::terminal_size()
                .map(|(w, _)| w.0 as usize)
                .unwrap_or(80)
        })
    }

    /// Create a minimal configuration for testing
    #[cfg(test)]
    pub fn minimal() -> Self {
        Self {
            ui: UiConfig {
                colors: false,
                icons: false,
                interactive: false,
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

/// Configuration builder for easy configuration creation
pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: Config::default(),
        }
    }

    pub fn with_colors(mut self, enabled: bool) -> Self {
        self.config.ui.colors = enabled;
        self
    }

    pub fn with_theme(mut self, theme: impl Into<String>) -> Self {
        self.config.ui.theme = theme.into();
        self
    }

    pub fn with_editor(mut self, editor: impl Into<String>) -> Self {
        self.config.ui.editor = Some(editor.into());
        self
    }

    pub fn with_default_remote(mut self, remote: impl Into<String>) -> Self {
        self.config.git.default_remote = remote.into();
        self
    }

    pub fn with_verbose(mut self, enabled: bool) -> Self {
        self.config.advanced.verbose = enabled;
        self
    }

    pub fn build(self) -> Config {
        self.config
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.ui.colors);
        assert_eq!(config.git.default_remote, "origin");
        assert_eq!(config.git.default_branch, "main");
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        
        assert_eq!(config.ui.colors, deserialized.ui.colors);
        assert_eq!(config.git.default_remote, deserialized.git.default_remote);
    }

    #[test]
    fn test_config_builder() {
        let config = ConfigBuilder::new()
            .with_colors(false)
            .with_theme("dark")
            .with_default_remote("upstream")
            .with_verbose(true)
            .build();

        assert!(!config.ui.colors);
        assert_eq!(config.ui.theme, "dark");
        assert_eq!(config.git.default_remote, "upstream");
        assert!(config.advanced.verbose);
    }

    #[test]
    fn test_config_merge() {
        let mut base = Config::default();
        let override_config = ConfigBuilder::new()
            .with_colors(false)
            .with_verbose(true)
            .build();

        base.merge(&override_config);
        
        assert!(!base.ui.colors);
        assert!(base.advanced.verbose);
        assert_eq!(base.git.default_remote, "origin"); // Should remain unchanged
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        
        // Valid configuration should pass
        assert!(config.validate().is_ok());
        
        // Invalid theme should fail
        config.ui.theme = "invalid".to_string();
        assert!(config.validate().is_err());
        
        // Invalid thread count should fail
        config.ui.theme = "auto".to_string();
        config.advanced.performance.threads = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_environment_overrides() {
        std::env::set_var("RGIT_NO_COLOR", "1");
        std::env::set_var("RGIT_THEME", "dark");
        
        let mut config = Config::default();
        config.apply_environment_overrides();
        
        assert!(!config.ui.colors);
        assert_eq!(config.ui.theme, "dark");
        
        // Cleanup
        std::env::remove_var("RGIT_NO_COLOR");
        std::env::remove_var("RGIT_THEME");
    }

    #[test]
    fn test_config_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        
        let config = ConfigBuilder::new()
            .with_colors(false)
            .with_theme("light")
            .build();
        
        // Save configuration
        config.save_to_file(&config_path).unwrap();
        assert!(config_path.exists());
        
        // Load configuration
        let loaded_config = Config::load_from_file(&config_path).unwrap();
        assert!(!loaded_config.ui.colors);
        assert_eq!(loaded_config.ui.theme, "light");
    }
}