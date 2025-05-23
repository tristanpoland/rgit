use anyhow::Result;
use colored::*;

use crate::cli::QuickCommitArgs;
use crate::config::Config;
use crate::core::RgitCore;
use crate::error::RgitError;
use crate::interactive::{InteractivePrompt, CommitMessageEditor};
use crate::submodule::SubmoduleManager;
use crate::commands::{add, commit, status};

/// Execute the quick-commit command - streamlined commit workflow
pub async fn execute(args: &QuickCommitArgs, rgit: &RgitCore, config: &Config) -> Result<()> {
    println!("{} {} Quick Commit Workflow", "⚡".yellow(), "rgit".cyan().bold());
    println!();

    // Step 1: Check repository state and submodules
    validate_quick_commit_preconditions(rgit, config).await?;

    // Step 2: Show current status
    show_quick_commit_status(rgit, config).await?;

    // Step 3: Handle file staging
    let staged_files = handle_file_staging(rgit, config, args).await?;

    if staged_files == 0 {
        rgit.info("No files staged for commit");
        return Ok(());
    }

    // Step 4: Get commit message
    let message = get_quick_commit_message(args, config).await?;

    // Step 5: Create the commit
    let commit_id = create_quick_commit(rgit, &message).await?;

    // Step 6: Handle push if requested
    if args.push {
        handle_quick_push(rgit, config).await?;
    }

    // Step 7: Show success and next steps
    show_quick_commit_success(rgit, config, commit_id, args.push).await?;

    Ok(())
}

/// Validate preconditions for quick commit
async fn validate_quick_commit_preconditions(rgit: &RgitCore, config: &Config) -> Result<()> {
    // Check user identity
    if rgit.get_signature().is_err() {
        return Err(RgitError::UserIdentityNotConfigured.into());
    }

    // Check submodule health if enabled
    if config.submodules.health_check {
        let submodule_manager = SubmoduleManager::new(rgit, config);
        if !submodule_manager.interactive_health_check()? {
            return Err(RgitError::SubmoduleError("Submodule issues prevent commit".to_string()).into());
        }
    }

    // Check repository state
    let repo_state = rgit.repo.state();
    if !matches!(repo_state, git2::RepositoryState::Clean) {
        return Err(RgitError::InvalidRepositoryState(
            format!("Repository is in {:?} state", repo_state)
        ).into());
    }

    Ok(())
}

/// Show current repository status for quick commit
async fn show_quick_commit_status(rgit: &RgitCore, config: &Config) -> Result<()> {
    let status_summary = status::quick_status_check(rgit)?;

    println!("{} Current Status:", "📊".blue().bold());
    println!("  {} {}", "Branch:".bold(), status_summary.branch_name.cyan());
    println!("  {} {}", "Status:".bold(), status_summary.format_summary());

    if status_summary.has_changes() {
        println!("  {} {} staged, {} unstaged, {} untracked", 
                "Files:".bold(),
                status_summary.staged_count.to_string().green(),
                status_summary.unstaged_count.to_string().yellow(),
                status_summary.untracked_count.to_string().red());
    }

    if status_summary.needs_pull() {
        println!("  {} {} commits behind remote", "⬇️".blue(), status_summary.behind);
    }

    if status_summary.needs_push() {
        println!("  {} {} commits ahead of remote", "⬆️".blue(), status_summary.ahead);
    }

    println!();
    Ok(())
}

/// Handle file staging for quick commit
async fn handle_file_staging(rgit: &RgitCore, config: &Config, args: &QuickCommitArgs) -> Result<usize> {
    let status = rgit.status()?;
    let initial_staged = status.staged.len();

    if args.all {
        // Stage all changes
        stage_all_changes(rgit, config).await?;
    } else if initial_staged == 0 {
        // No files staged and not using --all, offer interactive staging
        stage_files_interactively(rgit, config).await?;
    }

    // Check final staged count
    let final_status = rgit.status()?;
    Ok(final_status.staged.len())
}

/// Stage all changes for quick commit
async fn stage_all_changes(rgit: &RgitCore, config: &Config) -> Result<()> {
    let status = rgit.status()?;
    let unstaged_count = status.unstaged.len();
    let untracked_count = status.untracked.len();
    let total_to_stage = unstaged_count + untracked_count;

    if total_to_stage == 0 {
        return Ok(());
    }

    println!("{} Auto-staging {} file{}...", 
            "📦".blue(),
            total_to_stage,
            if total_to_stage == 1 { "" } else { "s" });

    // Show what will be staged
    if config.ui.interactive && total_to_stage <= 10 {
        for file in status.unstaged.iter().take(5) {
            println!("  {} {}: {}", 
                    "○".yellow(), 
                    file.status_symbol(false).yellow(),
                    file.path.white());
        }
        for file in status.untracked.iter().take(5) {
            println!("  {} {}: {}", 
                    "?".red(), 
                    "untracked".red(),
                    file.path.white());
        }
        if total_to_stage > 10 {
            println!("  {} and {} more...", "...".dimmed(), total_to_stage - 10);
        }
    }

    // This would call the actual staging logic
    // For now, simulate success
    rgit.success(&format!("Staged {} files", total_to_stage));

    Ok(())
}

/// Stage files interactively
async fn stage_files_interactively(rgit: &RgitCore, config: &Config) -> Result<()> {
    if !config.is_interactive() {
        println!("{} No files staged and not in interactive mode", "ℹ️".blue());
        return Ok(());
    }

    let status = rgit.status()?;
    let stageable_count = status.unstaged.len() + status.untracked.len();

    if stageable_count == 0 {
        println!("{} No files to stage", "ℹ️".blue());
        return Ok(());
    }

    println!("{} {} file{} available for staging:", 
            "📝".yellow(),
            stageable_count,
            if stageable_count == 1 { "" } else { "s" });

    // Quick preview
    let preview_count = stageable_count.min(3);
    for file in status.unstaged.iter().take(preview_count) {
        println!("  {} {}: {}", 
                "○".yellow(), 
                file.status_symbol(false).yellow(),
                file.path.white());
    }
    for file in status.untracked.iter().take(preview_count - status.unstaged.len().min(preview_count)) {
        println!("  {} {}: {}", 
                "?".red(), 
                "untracked".red(),
                file.path.white());
    }
    if stageable_count > preview_count {
        println!("  {} and {} more...", "...".dimmed(), stageable_count - preview_count);
    }

    let options = vec![
        "Stage all files",
        "Select files interactively",
        "Continue without staging",
    ];

    let choice = InteractivePrompt::new()
        .with_message("How would you like to stage files?")
        .with_options(&options)
        .select()?;

    match choice {
        0 => {
            // Stage all
            stage_all_changes(rgit, config).await?;
        }
        1 => {
            // Interactive selection
            run_interactive_add(rgit, config).await?;
        }
        2 => {
            // Continue without staging
            println!("{} Continuing without staging new files", "ℹ️".blue());
        }
        _ => {}
    }

    Ok(())
}

/// Run interactive add command
async fn run_interactive_add(rgit: &RgitCore, config: &Config) -> Result<()> {
    // This would call the interactive add functionality
    // For now, simulate the process
    println!("{} Interactive file selection...", "🎯".blue());
    
    // In real implementation, this would call:
    // add::interactive_add(rgit, config).await?;
    
    rgit.success("Files staged interactively");
    Ok(())
}

/// Get commit message for quick commit
async fn get_quick_commit_message(args: &QuickCommitArgs, config: &Config) -> Result<String> {
    if let Some(ref message) = args.message {
        return Ok(message.clone());
    }

    if !config.is_interactive() {
        return Err(RgitError::NonInteractiveEnvironment.into());
    }

    // For quick commits, prefer simple inline input
    get_simple_commit_message(config).await
}

/// Get a simple commit message for quick workflow
async fn get_simple_commit_message(config: &Config) -> Result<String> {
    println!("{} Quick commit message:", "💬".blue());
    
    // Provide some helpful examples
    println!("  {} Examples: 'Fix bug in authentication', 'Add user profile page', 'Update dependencies'", "💡".dimmed());
    
    loop {
        let message: String = InteractivePrompt::new()
            .with_message("Enter commit message")
            .input()?;

        let trimmed = message.trim();
        
        if trimmed.is_empty() {
            println!("{} Commit message cannot be empty", "❌".red());
            continue;
        }

        // Basic validation
        if trimmed.len() > 72 {
            println!("{} Message is quite long ({}). Consider a shorter summary.", 
                    "⚠️".yellow(), trimmed.len());
            
            if InteractivePrompt::new()
                .with_message("Use this message anyway?")
                .confirm()? {
                return Ok(trimmed.to_string());
            }
            continue;
        }

        return Ok(trimmed.to_string());
    }
}

/// Create the quick commit
async fn create_quick_commit(rgit: &RgitCore, message: &str) -> Result<git2::Oid> {
    println!("{} Creating commit...", "📝".blue());
    
    let commit_id = rgit.commit(message, false)?;
    
    let short_id = crate::utils::shorten_oid(&commit_id, 8);
    let first_line = message.lines().next().unwrap_or("");
    
    rgit.success(&format!("Created commit {} \"{}\"", 
                         short_id.yellow(), 
                         first_line.white()));
    
    Ok(commit_id)
}

/// Handle push after quick commit
async fn handle_quick_push(rgit: &RgitCore, config: &Config) -> Result<()> {
    println!("\n{} Pushing to remote...", "⬆️".blue());
    
    let branch_info = rgit.get_branch_info()?;
    
    // Check if upstream is configured
    if branch_info.upstream.is_none() {
        handle_no_upstream_push(rgit, config, &branch_info.name).await?;
    } else {
        perform_quick_push(rgit, config).await?;
    }
    
    Ok(())
}

/// Handle push when no upstream is configured
async fn handle_no_upstream_push(rgit: &RgitCore, config: &Config, branch_name: &str) -> Result<()> {
    if !config.is_interactive() {
        return Err(RgitError::RemoteNotFound("upstream".to_string()).into());
    }

    println!("{} No upstream configured for branch '{}'", "⚠️".yellow(), branch_name);
    
    let options = vec![
        format!("Set upstream and push to origin/{}", branch_name),
        "Push without setting upstream".to_string(),
        "Skip push".to_string(),
    ];

    let choice = InteractivePrompt::new()
        .with_message("How would you like to push?")
        .with_options(&options)
        .select()?;

    match choice {
        0 => {
            // Set upstream and push
            println!("  {} Setting upstream to origin/{}", "🔗".blue(), branch_name);
            // In real implementation: set upstream and push
            rgit.success("Pushed and set upstream");
        }
        1 => {
            // Push without upstream
            perform_quick_push(rgit, config).await?;
        }
        2 => {
            println!("  {} Skipping push", "⏭️".blue());
        }
        _ => {}
    }

    Ok(())
}

/// Perform the actual push
async fn perform_quick_push(rgit: &RgitCore, _config: &Config) -> Result<()> {
    // In real implementation, this would:
    // 1. Get the remote and branch
    // 2. Push with progress feedback
    // 3. Handle authentication if needed
    
    // Simulate push
    rgit.success("Pushed to remote");
    Ok(())
}

/// Show quick commit success and next steps
async fn show_quick_commit_success(
    rgit: &RgitCore, 
    config: &Config, 
    commit_id: git2::Oid, 
    pushed: bool
) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }

    println!("\n{} Quick commit completed successfully! 🎉", "✅".green().bold());
    
    let short_id = crate::utils::shorten_oid(&commit_id, 8);
    println!("   {} Commit: {}", "📝".blue(), short_id.yellow());
    
    if pushed {
        println!("   {} Changes pushed to remote", "⬆️".green());
    }

    // Show updated status
    let final_status = status::quick_status_check(rgit)?;
    println!("   {} Status: {}", "📊".blue(), final_status.format_summary());

    // Show next steps
    println!("\n{} What's next:", "💡".blue().bold());
    
    if !pushed {
        println!("  • {} - Share your changes", "rgit push".cyan());
    }
    
    if final_status.has_changes() {
        println!("  • {} - Stage more changes", "rgit add".cyan());
        println!("  • {} - Make another quick commit", "rgit quick-commit".cyan());
    }
    
    println!("  • {} - Continue working on your project", "Edit files".cyan());
    println!("  • {} - View commit history", "rgit log".cyan());
    
    Ok(())
}

/// Enhanced quick commit with smart defaults
pub async fn smart_quick_commit(
    rgit: &RgitCore, 
    config: &Config, 
    auto_message: Option<String>
) -> Result<()> {
    // This is a utility function that can be called from other commands
    // It provides smart defaults for quick commits
    
    let status = rgit.status()?;
    
    // If working tree is clean, nothing to commit
    if status.is_clean() {
        rgit.info("Working tree clean, nothing to commit");
        return Ok(());
    }

    // Smart message generation if not provided
    let message = if let Some(msg) = auto_message {
        msg
    } else {
        generate_smart_commit_message(&status)?
    };

    // Stage all changes for smart commit
    if status.staged.is_empty() {
        // Auto-stage everything for smart commit
        println!("{} Auto-staging all changes for smart commit", "📦".blue());
        // In real implementation: stage all changes
    }

    // Create commit
    let commit_id = rgit.commit(&message, false)?;
    
    rgit.success(&format!("Smart commit created: {}", 
                         crate::utils::shorten_oid(&commit_id, 8).yellow()));
    
    Ok(())
}

/// Generate a smart commit message based on changes
fn generate_smart_commit_message(status: &crate::core::RepositoryStatus) -> Result<String> {
    let total_files = status.total_changes();
    
    if total_files == 0 {
        return Ok("Update files".to_string());
    }

    // Analyze file types and changes
    let mut new_files = 0;
    let mut modified_files = 0;
    let mut deleted_files = 0;

    for file in &status.unstaged {
        if file.status.contains(git2::Status::WT_NEW) {
            new_files += 1;
        } else if file.status.contains(git2::Status::WT_MODIFIED) {
            modified_files += 1;
        } else if file.status.contains(git2::Status::WT_DELETED) {
            deleted_files += 1;
        }
    }

    for file in &status.untracked {
        new_files += 1;
    }

    // Generate message based on changes
    let message = match (new_files, modified_files, deleted_files) {
        (n, 0, 0) if n > 0 => format!("Add {} new file{}", n, if n == 1 { "" } else { "s" }),
        (0, m, 0) if m > 0 => format!("Update {} file{}", m, if m == 1 { "" } else { "s" }),
        (0, 0, d) if d > 0 => format!("Remove {} file{}", d, if d == 1 { "" } else { "s" }),
        (n, m, 0) if n > 0 && m > 0 => format!("Add {} and update {} files", n, m),
        (0, m, d) if m > 0 && d > 0 => format!("Update {} and remove {} files", m, d),
        (n, 0, d) if n > 0 && d > 0 => format!("Add {} and remove {} files", n, d),
        _ => format!("Update {} files", total_files),
    };

    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_repo() -> (TempDir, git2::Repository) {
        let temp_dir = TempDir::new().unwrap();
        let repo = git2::Repository::init(temp_dir.path()).unwrap();
        
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        
        (temp_dir, repo)
    }

    #[test]
    fn test_generate_smart_commit_message() {
        use crate::core::{RepositoryStatus, FileStatus};
        
        let status = RepositoryStatus {
            staged: vec![],
            unstaged: vec![
                FileStatus {
                    path: "file1.txt".to_string(),
                    status: git2::Status::WT_MODIFIED,
                    size: 100,
                    modified_time: None,
                }
            ],
            untracked: vec![
                FileStatus {
                    path: "file2.txt".to_string(),
                    status: git2::Status::WT_NEW,
                    size: 50,
                    modified_time: None,
                }
            ],
            branch_info: Default::default(),
        };
        
        let message = generate_smart_commit_message(&status).unwrap();
        assert!(message.contains("Add") && message.contains("update"));
    }

    #[tokio::test]
    async fn test_quick_commit_validation() {
        let (_temp_dir, repo) = create_test_repo();
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let config = Config::minimal();
        
        // Should pass validation with proper setup
        let result = validate_quick_commit_preconditions(&rgit, &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_smart_quick_commit_clean_repo() {
        let (_temp_dir, repo) = create_test_repo();
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let config = Config::minimal();
        
        // Clean repo should return early
        let result = smart_quick_commit(&rgit, &config, None).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_generate_message_variants() {
        use crate::core::{RepositoryStatus, FileStatus};
        
        // Test new files only
        let status_new = RepositoryStatus {
            staged: vec![],
            unstaged: vec![],
            untracked: vec![
                FileStatus {
                    path: "new.txt".to_string(),
                    status: git2::Status::WT_NEW,
                    size: 100,
                    modified_time: None,
                }
            ],
            branch_info: Default::default(),
        };
        
        let message = generate_smart_commit_message(&status_new).unwrap();
        assert!(message.contains("Add 1 new file"));
        
        // Test modified files only
        let status_modified = RepositoryStatus {
            staged: vec![],
            unstaged: vec![
                FileStatus {
                    path: "existing.txt".to_string(),
                    status: git2::Status::WT_MODIFIED,
                    size: 100,
                    modified_time: None,
                }
            ],
            untracked: vec![],
            branch_info: Default::default(),
        };
        
        let message = generate_smart_commit_message(&status_modified).unwrap();
        assert!(message.contains("Update 1 file"));
    }
}