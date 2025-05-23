use anyhow::Result;
use colored::*;

use crate::cli::StatusArgs;
use crate::config::Config;
use crate::core::RgitCore;
use crate::status::StatusDisplay;
use crate::submodule::SubmoduleManager;

/// Execute the status command
pub async fn execute(args: &StatusArgs, rgit: &RgitCore, config: &Config) -> Result<()> {
    // Create status display with options from arguments
    let display = StatusDisplay::from_args(
        args.short,
        args.ignored,
        args.submodules,
        args.ahead_behind,
        args.timestamps,
    );

    // Show enhanced status
    display.display(rgit)?;

    // Show submodule status if requested or if submodules have issues
    if args.submodules || config.submodules.health_check {
        show_submodule_status(rgit, config, args.submodules).await?;
    }

    // Show helpful hints if not in short mode
    if !args.short && config.ui.interactive {
        show_status_hints(rgit, config).await?;
    }

    Ok(())
}

/// Show submodule status information
async fn show_submodule_status(rgit: &RgitCore, config: &Config, detailed: bool) -> Result<()> {
    let submodule_manager = SubmoduleManager::new(rgit, config);
    let health = submodule_manager.check_health()?;

    if health.submodules.is_empty() {
        if detailed {
            println!("{} No submodules found", "ℹ️".blue());
        }
        return Ok(());
    }

    if detailed {
        // Show detailed submodule information
        submodule_manager.display_detailed_status(&health)?;
    } else if !health.is_healthy() {
        // Show summary of issues if there are any
        println!("\n{} {} submodule{} with issues:", 
                "⚠️".yellow(), 
                health.total_issues(),
                if health.total_issues() == 1 { "" } else { "s" });
        
        for name in health.unhealthy_submodules() {
            println!("  {} {}", "📦".red(), name.yellow());
        }
        
        println!("  💡 Use \"{}\" for details", "rgit submodule status".cyan());
    }

    Ok(())
}

/// Show helpful hints based on current repository state
async fn show_status_hints(rgit: &RgitCore, config: &Config) -> Result<()> {
    let status = rgit.status()?;
    
    if status.is_clean() {
        show_clean_repository_hints(rgit, config).await?;
    } else {
        show_dirty_repository_hints(&status, config).await?;
    }

    Ok(())
}

/// Show hints for clean repositories
async fn show_clean_repository_hints(rgit: &RgitCore, config: &Config) -> Result<()> {
    let branch_info = rgit.get_branch_info()?;
    
    println!("\n{} {} Repository is clean!", "✨".green(), "Tip:".bold());
    
    // Suggest next actions based on branch state
    if branch_info.behind > 0 {
        println!("  • {} - Get latest changes", "rgit pull".cyan());
    }
    
    if branch_info.ahead > 0 {
        println!("  • {} - Share your changes", "rgit push".cyan());
    }
    
    if branch_info.upstream.is_none() && branch_info.name != "main" && branch_info.name != "master" {
        println!("  • {} - Set up tracking", 
                format!("rgit push --set-upstream origin {}", branch_info.name).cyan());
    }
    
    // Suggest common development actions
    println!("  • {} - Quick workflow for changes", "rgit quick-commit".cyan());
    println!("  • {} - Sync with remote", "rgit sync".cyan());
    
    // Check if there are stashes
    if let Ok(stash_count) = count_stash_entries(rgit) {
        if stash_count > 0 {
            println!("  • {} - Review {} stashed change{}", 
                    "rgit stash list".cyan(),
                    stash_count,
                    if stash_count == 1 { "" } else { "s" });
        }
    }

    Ok(())
}

/// Show hints for repositories with changes
async fn show_dirty_repository_hints(
    status: &crate::core::RepositoryStatus, 
    _config: &Config
) -> Result<()> {
    println!("\n{} {} Next steps:", "💡".blue(), "Tip:".bold());
    
    if !status.untracked.is_empty() || !status.unstaged.is_empty() {
        println!("  • {} - Select files to stage", "rgit add".cyan());
        if status.untracked.len() + status.unstaged.len() > 3 {
            println!("  • {} - Stage all changes", "rgit add --all".cyan());
        }
    }
    
    if !status.staged.is_empty() {
        println!("  • {} - Commit staged changes", "rgit commit".cyan());
        println!("  • {} - Quick commit workflow", "rgit quick-commit".cyan());
    }
    
    if !status.is_clean() {
        println!("  • {} - Sync when ready", "rgit sync".cyan());
        println!("  • {} - Temporarily save changes", "rgit stash save".cyan());
    }

    Ok(())
}

/// Count stash entries in the repository
fn count_stash_entries(rgit: &RgitCore) -> Result<usize> {
    // Try to get stash reference
    match rgit.repo.reflog("refs/stash") {
        Ok(reflog) => Ok(reflog.len()),
        Err(_) => Ok(0), // No stash exists
    }
}

/// Enhanced status command that can be called from other commands
pub async fn show_status_summary(rgit: &RgitCore, config: &Config) -> Result<()> {
    let status = rgit.status()?;
    
    if status.is_clean() {
        println!("{} Working tree clean", "✅".green());
    } else {
        let total = status.total_changes();
        println!("{} {} change{} ({} staged, {} unstaged, {} untracked)", 
                "📊".blue(),
                total,
                if total == 1 { "" } else { "s" },
                status.staged.len(),
                status.unstaged.len(),
                status.untracked.len());
    }
    
    // Show branch status
    let branch_info = status.branch_info;
    if branch_info.ahead > 0 || branch_info.behind > 0 {
        println!("   {}", branch_info.format_tracking_info());
    }
    
    Ok(())
}

/// Quick status check for use in other commands
pub fn quick_status_check(rgit: &RgitCore) -> Result<StatusSummary> {
    let status = rgit.status()?;
    let branch_info = rgit.get_branch_info()?;
    
    Ok(StatusSummary {
        is_clean: status.is_clean(),
        staged_count: status.staged.len(),
        unstaged_count: status.unstaged.len(),
        untracked_count: status.untracked.len(),
        ahead: branch_info.ahead,
        behind: branch_info.behind,
        has_upstream: branch_info.upstream.is_some(),
        branch_name: branch_info.name,
    })
}

/// Summary of repository status for quick checks
#[derive(Debug, Clone)]
pub struct StatusSummary {
    pub is_clean: bool,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub untracked_count: usize,
    pub ahead: usize,
    pub behind: usize,
    pub has_upstream: bool,
    pub branch_name: String,
}

impl StatusSummary {
    pub fn total_changes(&self) -> usize {
        self.staged_count + self.unstaged_count + self.untracked_count
    }
    
    pub fn has_changes(&self) -> bool {
        self.total_changes() > 0
    }
    
    pub fn needs_push(&self) -> bool {
        self.ahead > 0
    }
    
    pub fn needs_pull(&self) -> bool {
        self.behind > 0
    }
    
    pub fn is_in_sync(&self) -> bool {
        self.ahead == 0 && self.behind == 0
    }
    
    pub fn format_summary(&self) -> String {
        if self.is_clean && self.is_in_sync() {
            "Clean and up to date".green().to_string()
        } else if self.is_clean {
            match (self.ahead, self.behind) {
                (0, behind) if behind > 0 => format!("Clean, {} behind", behind.to_string().red()),
                (ahead, 0) if ahead > 0 => format!("Clean, {} ahead", ahead.to_string().green()),
                (ahead, behind) if ahead > 0 && behind > 0 => {
                    format!("Clean, {} ahead, {} behind", 
                           ahead.to_string().green(), 
                           behind.to_string().red())
                }
                _ => "Clean".green().to_string(),
            }
        } else {
            let changes = self.total_changes();
            format!("{} change{}", 
                   changes.to_string().yellow(), 
                   if changes == 1 { "" } else { "s" })
        }
    }
}

/// Status check that can be used as a pre-condition for other commands
pub fn require_clean_working_tree(rgit: &RgitCore, operation: &str) -> Result<()> {
    let status = rgit.status()?;
    
    if !status.is_clean() {
        return Err(crate::error::RgitError::BranchHasUncommittedChanges.into());
    }
    
    Ok(())
}

/// Status check with user interaction for potentially destructive operations
pub async fn confirm_with_status(
    rgit: &RgitCore, 
    config: &Config, 
    operation: &str
) -> Result<bool> {
    let status = rgit.status()?;
    
    if status.is_clean() {
        return Ok(true);
    }
    
    // Show current status
    println!("{} Current repository status:", "📋".blue());
    show_status_summary(rgit, config).await?;
    
    // Ask for confirmation
    if !config.is_interactive() {
        return Err(crate::error::RgitError::NonInteractiveEnvironment.into());
    }
    
    let message = format!("Continue with {} despite uncommitted changes?", operation);
    crate::interactive::InteractivePrompt::new()
        .with_message(&message)
        .confirm()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_repo() -> (TempDir, git2::Repository) {
        let temp_dir = TempDir::new().unwrap();
        let repo = git2::Repository::init(temp_dir.path()).unwrap();
        
        // Set up user identity
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        
        (temp_dir, repo)
    }

    #[tokio::test]
    async fn test_status_clean_repo() {
        let (_temp_dir, repo) = create_test_repo();
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let summary = quick_status_check(&rgit).unwrap();
        
        assert!(summary.is_clean);
        assert_eq!(summary.total_changes(), 0);
    }

    #[tokio::test]
    async fn test_status_with_changes() {
        let (temp_dir, repo) = create_test_repo();
        
        // Create a file
        fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();
        
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let summary = quick_status_check(&rgit).unwrap();
        
        assert!(!summary.is_clean);
        assert_eq!(summary.untracked_count, 1);
    }

    #[test]
    fn test_status_summary_formatting() {
        let clean_summary = StatusSummary {
            is_clean: true,
            staged_count: 0,
            unstaged_count: 0,
            untracked_count: 0,
            ahead: 0,
            behind: 0,
            has_upstream: true,
            branch_name: "main".to_string(),
        };
        
        assert!(clean_summary.format_summary().contains("Clean"));
        
        let dirty_summary = StatusSummary {
            is_clean: false,
            staged_count: 1,
            unstaged_count: 2,
            untracked_count: 1,
            ahead: 0,
            behind: 0,
            has_upstream: true,
            branch_name: "main".to_string(),
        };
        
        assert_eq!(dirty_summary.total_changes(), 4);
        assert!(dirty_summary.format_summary().contains("4 changes"));
    }

    #[test]
    fn test_status_summary_properties() {
        let summary = StatusSummary {
            is_clean: false,
            staged_count: 1,
            unstaged_count: 1,
            untracked_count: 1,
            ahead: 2,
            behind: 1,
            has_upstream: true,
            branch_name: "feature".to_string(),
        };
        
        assert!(summary.has_changes());
        assert!(summary.needs_push());
        assert!(summary.needs_pull());
        assert!(!summary.is_in_sync());
    }

    #[tokio::test]
    async fn test_require_clean_working_tree() {
        let (_temp_dir, repo) = create_test_repo();
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        
        // Should pass with clean repo
        assert!(require_clean_working_tree(&rgit, "test operation").is_ok());
    }
}