use anyhow::Result;
use clap::Parser;
use colored::*;
use std::process;
use tracing::{debug, error, info};

mod cli;
mod config;
mod core;
mod error;
mod interactive;
mod status;
mod submodule;
mod utils;
mod commands;

use cli::{Cli, Commands};
use config::Config;
use core::RgitCore;
use error::RgitError;

#[tokio::main]
async fn main() {
    // Initialize tracing for debugging
    init_tracing();

    // Parse command line arguments
    let cli = Cli::parse();

    // Initialize global configuration
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("{} Failed to load configuration: {}", "❌".red(), e);
            process::exit(1);
        }
    };

    // Handle global flags
    if cli.no_color {
        colored::control::set_override(false);
    }

    // Show welcome message for interactive commands
    if cli.verbose {
        print_banner();
    }

    // Execute the command
    let result = execute_command(cli, config).await;

    // Handle results with proper error formatting
    match result {
        Ok(()) => {
            debug!("Command executed successfully");
        }
        Err(e) => {
            error!("Command failed: {}", e);
            print_error(&e);
            process::exit(1);
        }
    }
}

/// Initialize tracing for debugging and logging
fn init_tracing() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("rgit=info".parse().unwrap())
        )
        .with_target(false)
        .compact()
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");
}

/// Print the application banner for verbose mode
fn print_banner() {
    println!("{}", format!("
╭─────────────────────────────────────────╮
│  🦀 {}  - A Superior Git CLI in Rust    │
│     Version {}                          │
│     Making Git operations delightful    │
╰─────────────────────────────────────────╯
", "rgit".cyan().bold(), env!("CARGO_PKG_VERSION")).cyan());
}

/// Execute the parsed command with proper error handling
async fn execute_command(cli: Cli, config: Config) -> Result<()> {
    debug!("Executing command: {:?}", cli.command);

    match &cli.command {
        // Repository initialization commands
        Commands::Init(args) => {
            commands::init::execute(args, &config).await
        }
        Commands::Clone(args) => {
            commands::clone::execute(args, &config).await
        }

        // Core Git operations
        Commands::Status(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::status::execute(args, &rgit, &config).await
        }
        Commands::Add(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::add::execute(args, &rgit, &config).await
        }
        Commands::Commit(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::commit::execute(args, &rgit, &config).await
        }
        Commands::Push(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::push::execute(args, &rgit, &config).await
        }
        Commands::Pull(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::pull::execute(args, &rgit, &config).await
        }
        Commands::Fetch(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::fetch::execute(args, &rgit, &config).await
        }

        // Branch management
        Commands::Branch(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::branch::execute(args, &rgit, &config).await
        }
        Commands::Checkout(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::checkout::execute(args, &rgit, &config).await
        }
        Commands::Merge(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::merge::execute(args, &rgit, &config).await
        }
        Commands::Rebase(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::rebase::execute(args, &rgit, &config).await
        }

        // History and information
        Commands::Log(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::log::execute(args, &rgit, &config).await
        }
        Commands::Diff(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::diff::execute(args, &rgit, &config).await
        }
        Commands::Show(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::show::execute(args, &rgit, &config).await
        }
        Commands::Blame(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::blame::execute(args, &rgit, &config).await
        }

        // Submodule operations
        Commands::Submodule(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::submodule::execute(args, &rgit, &config).await
        }

        // Advanced operations
        Commands::Stash(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::stash::execute(args, &rgit, &config).await
        }
        Commands::Tag(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::tag::execute(args, &rgit, &config).await
        }
        Commands::Remote(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::remote::execute(args, &rgit, &config).await
        }

        // Ease-of-use commands
        Commands::Sync(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::sync::execute(args, &rgit, &config).await
        }
        Commands::QuickCommit(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::quick_commit::execute(args, &rgit, &config).await
        }
        Commands::Undo(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::undo::execute(args, &rgit, &config).await
        }
        Commands::Clean(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::clean::execute(args, &rgit, &config).await
        }

        // Utility commands
        Commands::Doctor => {
            commands::doctor::execute(&config).await
        }
        Commands::Learn(args) => {
            commands::learn::execute(args, &config).await
        }
        Commands::Resolve => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::resolve::execute(&rgit, &config).await
        }
        Commands::Backup(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::backup::execute(args, &rgit, &config).await
        }
        Commands::Restore(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::restore::execute(args, &rgit, &config).await
        }

        // Advanced Git operations
        Commands::Bisect(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::bisect::execute(args, &rgit, &config).await
        }
        Commands::Reflog(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::reflog::execute(args, &rgit, &config).await
        }
        Commands::Gc(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::gc::execute(args, &rgit, &config).await
        }
        Commands::Fsck(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::fsck::execute(args, &rgit, &config).await
        }
        Commands::CherryPick(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::cherry_pick::execute(args, &rgit, &config).await
        }
        Commands::Grep(args) => {
            let rgit = RgitCore::new(cli.verbose)?;
            commands::grep::execute(args, &rgit, &config).await
        }
    }
}

/// Print formatted error messages with helpful suggestions
fn print_error(error: &anyhow::Error) {
    eprintln!("{} {}", "❌".red().bold(), "Error:".red().bold());
    
    // Print the main error
    eprintln!("   {}", error.to_string().white());
    
    // Print the error chain
    let mut current = error.source();
    while let Some(err) = current {
        eprintln!("   {} {}", "└─".dimmed(), err.to_string().dimmed());
        current = err.source();
    }
    
    // Print helpful suggestions based on error type
    if let Some(rgit_error) = error.downcast_ref::<RgitError>() {
        print_error_suggestions(rgit_error);
    }
    
    eprintln!();
    eprintln!("{} Use {} for help or {} for tutorials", 
             "💡".yellow(), 
             "rgit --help".cyan(), 
             "rgit learn".cyan());
}

/// Print context-specific suggestions for different error types
fn print_error_suggestions(error: &RgitError) {
    let suggestion = match error {
        RgitError::NotInRepository => {
            "Try running 'rgit init' to create a new repository or navigate to an existing one"
        }
        RgitError::SubmoduleError(_) => {
            "Use 'rgit submodule status' to check submodule health or 'rgit doctor' for diagnostics"
        }
        RgitError::MergeConflict(_) => {
            "Use 'rgit resolve' for interactive conflict resolution or 'rgit status' to see conflicts"
        }
        RgitError::AuthenticationError(_) => {
            "Check your Git credentials or use 'rgit doctor' to verify remote connections"
        }
        RgitError::NetworkError(_) => {
            "Check your internet connection and remote URL configuration"
        }
        _ => return,
    };
    
    eprintln!("   {} {}", "💡".yellow(), suggestion.yellow());
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_application_startup() {
        // Test that the application can start without panicking
        // This is a basic smoke test
        let _config = Config::default();
        // Add more specific tests as needed
    }
    
    #[test]
    fn test_error_formatting() {
        let error = RgitError::NotInRepository;
        let formatted = format!("{}", error);
        assert!(!formatted.is_empty());
    }
}