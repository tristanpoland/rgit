use anyhow::Result;

use crate::config::Config;
use crate::core::RgitCore;

// Core commands
pub mod init;
pub mod clone;
pub mod status;
pub mod add;
pub mod commit;
pub mod push;
pub mod pull;
pub mod fetch;

// Branch management
pub mod branch;
pub mod checkout;
pub mod merge;
pub mod rebase;
pub mod cherry_pick;

// History and information
pub mod log;
pub mod diff;
pub mod show;
pub mod blame;
pub mod grep;

// Remote management
pub mod remote;

// Tag management
pub mod tag;

// Stash operations
pub mod stash;

// Submodule operations
pub mod submodule;

// Advanced operations
pub mod bisect;
pub mod reflog;
pub mod gc;
pub mod fsck;

// Ease-of-use commands
pub mod sync;
pub mod quick_commit;
pub mod undo;
pub mod clean;
pub mod resolve;
pub mod backup;
pub mod restore;

// Utility commands
pub mod doctor;
pub mod learn;

/// Trait for command implementations
pub trait Command {
    /// Execute the command with the given arguments
    fn execute(&self, rgit: &RgitCore, config: &Config) -> Result<()>;
    
    /// Get command name for logging and error reporting
    fn name(&self) -> &'static str;
    
    /// Get command description
    fn description(&self) -> &'static str;
    
    /// Check if command requires a git repository
    fn requires_repo(&self) -> bool {
        true
    }
    
    /// Check if command modifies the repository
    fn is_write_operation(&self) -> bool {
        false
    }
    
    /// Get command aliases
    fn aliases(&self) -> Vec<&'static str> {
        vec![]
    }
}

/// Async command trait for commands that perform async operations
#[async_trait::async_trait]
pub trait AsyncCommand {
    /// Execute the command asynchronously
    async fn execute_async(&self, rgit: &RgitCore, config: &Config) -> Result<()>;
    
    /// Get command name
    fn name(&self) -> &'static str;
    
    /// Get command description
    fn description(&self) -> &'static str;
    
    /// Check if command requires a git repository
    fn requires_repo(&self) -> bool {
        true
    }
    
    /// Check if command modifies the repository
    fn is_write_operation(&self) -> bool {
        false
    }
}

/// Command execution context
#[derive(Debug, Clone)]
pub struct CommandContext {
    /// Whether to show verbose output
    pub verbose: bool,
    /// Whether colors are enabled
    pub colors: bool,
    /// Working directory
    pub working_dir: Option<std::path::PathBuf>,
    /// Additional environment variables
    pub env_vars: std::collections::HashMap<String, String>,
}

impl CommandContext {
    pub fn new() -> Self {
        Self {
            verbose: false,
            colors: true,
            working_dir: None,
            env_vars: std::collections::HashMap::new(),
        }
    }
    
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
    
    pub fn with_colors(mut self, colors: bool) -> Self {
        self.colors = colors;
        self
    }
    
    pub fn with_working_dir(mut self, dir: std::path::PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }
    
    pub fn with_env_var(mut self, key: String, value: String) -> Self {
        self.env_vars.insert(key, value);
        self
    }
}

impl Default for CommandContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Command execution result with additional metadata
#[derive(Debug)]
pub struct CommandResult {
    /// Whether the command succeeded
    pub success: bool,
    /// Exit code
    pub exit_code: i32,
    /// Execution time in milliseconds
    pub execution_time: u64,
    /// Additional result data
    pub data: std::collections::HashMap<String, serde_json::Value>,
}

impl CommandResult {
    pub fn success() -> Self {
        Self {
            success: true,
            exit_code: 0,
            execution_time: 0,
            data: std::collections::HashMap::new(),
        }
    }
    
    pub fn failure(exit_code: i32) -> Self {
        Self {
            success: false,
            exit_code,
            execution_time: 0,
            data: std::collections::HashMap::new(),
        }
    }
    
    pub fn with_execution_time(mut self, time_ms: u64) -> Self {
        self.execution_time = time_ms;
        self
    }
    
    pub fn with_data(mut self, key: String, value: serde_json::Value) -> Self {
        self.data.insert(key, value);
        self
    }
}

/// Utility functions for command implementations
pub mod utils {
    use super::*;
    use crate::error::RgitError;
    use crate::interactive::InteractivePrompt;
    use colored::*;
    use std::time::Instant;

    /// Execute a command with timing and error handling
    pub async fn execute_with_timing<F, Fut>(
        command_name: &str,
        operation: F,
    ) -> Result<CommandResult>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let start = Instant::now();
        
        match operation().await {
            Ok(()) => {
                let duration = start.elapsed().as_millis() as u64;
                Ok(CommandResult::success().with_execution_time(duration))
            }
            Err(e) => {
                let duration = start.elapsed().as_millis() as u64;
                eprintln!("{} Command '{}' failed: {}", 
                         "❌".red(), 
                         command_name.cyan(), 
                         e);
                Ok(CommandResult::failure(1).with_execution_time(duration))
            }
        }
    }
    
    /// Confirm destructive operation
    pub fn confirm_destructive_operation(
        operation: &str,
        details: Option<&str>,
        config: &Config,
    ) -> Result<bool> {
        if !config.advanced.safety.confirm_destructive {
            return Ok(true);
        }
        
        if !config.is_interactive() {
            return Err(RgitError::NonInteractiveEnvironment.into());
        }
        
        let mut message = format!("Are you sure you want to {}?", operation);
        if let Some(details) = details {
            message.push_str(&format!("\n{}", details));
        }
        
        InteractivePrompt::new()
            .with_message(&message)
            .confirm()
    }
    
    /// Check if repository is in a clean state for operations that require it
    pub fn ensure_clean_working_tree(rgit: &RgitCore) -> Result<()> {
        if !rgit.is_clean()? {
            return Err(RgitError::BranchHasUncommittedChanges.into());
        }
        Ok(())
    }
    
    /// Show operation summary
    pub fn show_operation_summary(
        operation: &str,
        changes: &[String],
        config: &Config,
    ) {
        if config.ui.interactive && !changes.is_empty() {
            println!("\n{} {} summary:", "📋".blue(), operation.cyan().bold());
            for change in changes {
                println!("  {} {}", "•".green(), change);
            }
            println!();
        }
    }
    
    /// Format command execution time
    pub fn format_execution_time(ms: u64) -> String {
        if ms < 1000 {
            format!("{}ms", ms)
        } else if ms < 60000 {
            format!("{:.1}s", ms as f64 / 1000.0)
        } else {
            let seconds = ms / 1000;
            let minutes = seconds / 60;
            let remaining_seconds = seconds % 60;
            format!("{}m{}s", minutes, remaining_seconds)
        }
    }
    
    /// Check command prerequisites
    pub fn check_prerequisites(
        command: &dyn Command,
        rgit: Option<&RgitCore>,
        config: &Config,
    ) -> Result<()> {
        // Check if repository is required
        if command.requires_repo() && rgit.is_none() {
            return Err(RgitError::NotInRepository.into());
        }
        
        // Check if interactive mode is available for interactive commands
        if command.name() == "resolve" || command.name() == "learn" {
            if !config.is_interactive() {
                return Err(RgitError::NonInteractiveEnvironment.into());
            }
        }
        
        // Additional checks can be added here
        Ok(())
    }
    
    /// Show command help
    pub fn show_command_help(command: &dyn Command) {
        println!("{} {}", command.name().cyan().bold(), command.description());
        
        if !command.aliases().is_empty() {
            println!("Aliases: {}", 
                    command.aliases().join(", ").dimmed());
        }
        
        println!("Requires repository: {}", 
                if command.requires_repo() { "Yes".green() } else { "No".red() });
        
        println!("Modifies repository: {}", 
                if command.is_write_operation() { "Yes".yellow() } else { "No".green() });
    }
}

/// Macro to create a simple command implementation
#[macro_export]
macro_rules! impl_simple_command {
    ($struct_name:ident, $name:expr, $description:expr, $requires_repo:expr, $is_write:expr) => {
        impl Command for $struct_name {
            fn name(&self) -> &'static str {
                $name
            }
            
            fn description(&self) -> &'static str {
                $description
            }
            
            fn requires_repo(&self) -> bool {
                $requires_repo
            }
            
            fn is_write_operation(&self) -> bool {
                $is_write
            }
        }
    };
}

/// Macro to create an async command implementation
#[macro_export]
macro_rules! impl_async_command {
    ($struct_name:ident, $name:expr, $description:expr, $requires_repo:expr, $is_write:expr) => {
        #[async_trait::async_trait]
        impl AsyncCommand for $struct_name {
            fn name(&self) -> &'static str {
                $name
            }
            
            fn description(&self) -> &'static str {
                $description
            }
            
            fn requires_repo(&self) -> bool {
                $requires_repo
            }
            
            fn is_write_operation(&self) -> bool {
                $is_write
            }
        }
    };
}

/// Command registry for dynamic command discovery
pub struct CommandRegistry {
    commands: std::collections::HashMap<String, Box<dyn Command>>,
    aliases: std::collections::HashMap<String, String>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: std::collections::HashMap::new(),
            aliases: std::collections::HashMap::new(),
        }
    }
    
    pub fn register<C: Command + 'static>(&mut self, command: C) {
        let name = command.name().to_string();
        
        // Register aliases
        for alias in command.aliases() {
            self.aliases.insert(alias.to_string(), name.clone());
        }
        
        self.commands.insert(name, Box::new(command));
    }
    
    pub fn get(&self, name: &str) -> Option<&dyn Command> {
        // Try direct lookup first
        if let Some(command) = self.commands.get(name) {
            return Some(command.as_ref());
        }
        
        // Try alias lookup
        if let Some(real_name) = self.aliases.get(name) {
            return self.commands.get(real_name).map(|c| c.as_ref());
        }
        
        None
    }
    
    pub fn list_commands(&self) -> Vec<&str> {
        self.commands.keys().map(|k| k.as_str()).collect()
    }
    
    pub fn list_aliases(&self) -> Vec<(&str, &str)> {
        self.aliases
            .iter()
            .map(|(alias, command)| (alias.as_str(), command.as_str()))
            .collect()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCommand;
    
    impl Command for TestCommand {
        fn execute(&self, _rgit: &RgitCore, _config: &Config) -> Result<()> {
            Ok(())
        }
        
        fn name(&self) -> &'static str {
            "test"
        }
        
        fn description(&self) -> &'static str {
            "Test command"
        }
        
        fn aliases(&self) -> Vec<&'static str> {
            vec!["t"]
        }
    }

    #[test]
    fn test_command_registry() {
        let mut registry = CommandRegistry::new();
        registry.register(TestCommand);
        
        assert!(registry.get("test").is_some());
        assert!(registry.get("t").is_some());
        assert!(registry.get("nonexistent").is_none());
        
        let commands = registry.list_commands();
        assert!(commands.contains(&"test"));
        
        let aliases = registry.list_aliases();
        assert!(aliases.contains(&("t", "test")));
    }

    #[test]
    fn test_command_context() {
        let context = CommandContext::new()
            .with_verbose(true)
            .with_colors(false);
        
        assert!(context.verbose);
        assert!(!context.colors);
    }

    #[test]
    fn test_command_result() {
        let result = CommandResult::success()
            .with_execution_time(1000)
            .with_data("files_changed".to_string(), serde_json::Value::Number(serde_json::Number::from(5)));
        
        assert!(result.success);
        assert_eq!(result.execution_time, 1000);
        assert!(result.data.contains_key("files_changed"));
    }
}