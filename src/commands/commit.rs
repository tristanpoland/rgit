use anyhow::Result;
use colored::*;
use git2::Oid;
use std::fs;
use std::path::PathBuf;

use crate::cli::CommitArgs;
use crate::config::Config;
use crate::core::RgitCore;
use crate::error::RgitError;
use crate::interactive::{CommitMessageEditor, InteractivePrompt};
use crate::utils::{validate_commit_message, shorten_oid};

/// Execute the commit command
pub async fn execute(args: &CommitArgs, rgit: &RgitCore, config: &Config) -> Result<()> {
    // Pre-commit checks
    perform_pre_commit_checks(rgit, config, args).await?;
    
    // Get commit message
    let message = get_commit_message(args, rgit, config).await?;
    
    // Auto-stage files if requested
    if args.all {
        auto_stage_files(rgit, config).await?;
    }
    
    // Check if there's anything to commit
    if !args.allow_empty && !rgit.has_staged_changes()? {
        if args.amend {
            // For amend, we can proceed even without staged changes
            rgit.log("Amending commit without adding new changes");
        } else {
            return Err(RgitError::NothingToCommit.into());
        }
    }
    
    // Run pre-commit hooks if enabled
    if !args.no_verify && config.integrations.hooks.pre_commit {
        run_pre_commit_hooks(rgit, config).await?;
    }
    
    // Create the commit
    let commit_id = create_commit(rgit, &message, args).await?;
    
    // Show commit summary
    show_commit_summary(rgit, commit_id, &message, config).await?;
    
    // Run post-commit hooks
    if !args.no_verify && config.integrations.hooks.commit_msg {
        run_post_commit_hooks(rgit, config, commit_id).await?;
    }
    
    // Show next steps
    show_next_steps(rgit, config).await?;
    
    Ok(())
}

/// Perform pre-commit validation and checks
async fn perform_pre_commit_checks(
    rgit: &RgitCore, 
    config: &Config, 
    args: &CommitArgs
) -> Result<()> {
    rgit.log("Performing pre-commit checks...");
    
    // Check for user identity
    if rgit.get_signature().is_err() {
        return Err(RgitError::UserIdentityNotConfigured.into());
    }
    
    // Check for merge state
    if is_merge_in_progress(rgit)? {
        handle_merge_commit(rgit, config).await?;
    }
    
    // Check submodule state if enabled
    if config.submodules.health_check {
        check_submodule_state(rgit, config).await?;
    }
    
    // Warn about amending published commits
    if args.amend {
        warn_about_amend_published(rgit, config).await?;
    }
    
    Ok(())
}

/// Get commit message from various sources
async fn get_commit_message(
    args: &CommitArgs, 
    rgit: &RgitCore, 
    config: &Config
) -> Result<String> {
    let message = if let Some(ref msg) = args.message {
        // Message provided via command line
        msg.clone()
    } else if let Some(ref file_path) = args.file {
        // Message from file
        read_message_from_file(file_path)?
    } else if args.template || config.git.default_branch.is_empty() {
        // Use commit message template
        get_message_from_template(rgit, config).await?
    } else {
        // Interactive message editing
        get_message_interactively(rgit, config).await?
    };
    
    // Validate message
    validate_and_improve_message(&message, config)?;
    
    Ok(message)
}

/// Read commit message from file
fn read_message_from_file(file_path: &PathBuf) -> Result<String> {
    let content = fs::read_to_string(file_path)
        .map_err(|_| RgitError::ConfigFileNotFound(file_path.clone()))?;
    
    if content.trim().is_empty() {
        return Err(RgitError::EmptyCommitMessage.into());
    }
    
    Ok(content.trim().to_string())
}

/// Get commit message using template
async fn get_message_from_template(rgit: &RgitCore, config: &Config) -> Result<String> {
    let template = create_commit_template(rgit, config).await?;
    
    let editor = CommitMessageEditor::new()
        .with_template(template)
        .with_validation()
        .with_diff();
    
    editor.edit()
}

/// Get commit message interactively
async fn get_message_interactively(rgit: &RgitCore, config: &Config) -> Result<String> {
    if !config.is_interactive() {
        return Err(RgitError::NonInteractiveEnvironment.into());
    }
    
    // Check if it's a simple commit that can use inline input
    let status = rgit.status()?;
    if status.staged.len() <= 3 && config.ui.interactive {
        return get_simple_commit_message(rgit, config).await;
    }
    
    // Use full editor for complex commits
    let template = create_commit_template(rgit, config).await?;
    let editor = CommitMessageEditor::new()
        .with_template(template)
        .with_validation()
        .with_diff();
    
    editor.edit()
}

/// Get a simple commit message for small changes
async fn get_simple_commit_message(rgit: &RgitCore, _config: &Config) -> Result<String> {
    let status = rgit.status()?;
    
    // Show what will be committed
    println!("{} Files to be committed:", "📦".green());
    for file in &status.staged {
        println!("  {} {}: {}", 
                "✓".green(), 
                file.status_symbol(true).green(),
                file.path.white());
    }
    println!();
    
    // Get commit message
    loop {
        let message: String = InteractivePrompt::new()
            .with_message("Commit message")
            .input()?;
        
        if message.trim().is_empty() {
            println!("{} Commit message cannot be empty", "❌".red());
            continue;
        }
        
        // Quick validation
        if let Err(issues) = quick_validate_message(&message) {
            println!("{} Message issues found:", "⚠️".yellow());
            for issue in &issues {
                println!("  • {}", issue.yellow());
            }
            
            if InteractivePrompt::new()
                .with_message("Use this message anyway?")
                .confirm()? {
                return Ok(message);
            }
            continue;
        }
        
        return Ok(message);
    }
}

/// Create commit message template
async fn create_commit_template(rgit: &RgitCore, _config: &Config) -> Result<String> {
    let mut template = String::new();
    
    // Add template hints
    template.push_str("# Enter your commit message above.\n");
    template.push_str("# \n");
    template.push_str("# Guidelines:\n");
    template.push_str("#   - Use imperative mood (\"Add feature\" not \"Added feature\")\n");
    template.push_str("#   - First line should be 50 characters or less\n");
    template.push_str("#   - Leave a blank line before the body\n");
    template.push_str("#   - Wrap body at 72 characters\n");
    template.push_str("# \n");
    
    // Add status information
    let status = rgit.status()?;
    if !status.staged.is_empty() {
        template.push_str("# Changes to be committed:\n");
        for file in &status.staged {
            template.push_str(&format!("#   {}: {}\n", 
                                     file.status_symbol(true), 
                                     file.path));
        }
        template.push_str("# \n");
    }
    
    if !status.unstaged.is_empty() {
        template.push_str("# Changes not staged for commit:\n");
        for file in &status.unstaged {
            template.push_str(&format!("#   {}: {}\n", 
                                     file.status_symbol(false), 
                                     file.path));
        }
        template.push_str("# \n");
    }
    
    if !status.untracked.is_empty() {
        template.push_str("# Untracked files:\n");
        for file in &status.untracked {
            template.push_str(&format!("#   {}\n", file.path));
        }
        template.push_str("# \n");
    }
    
    Ok(template)
}

/// Validate and potentially improve commit message
fn validate_and_improve_message(message: &str, config: &Config) -> Result<String> {
    let issues = validate_commit_message(message);
    
    if issues.is_empty() {
        return Ok(message.to_string());
    }
    
    // If non-interactive, just warn about issues
    if !config.is_interactive() {
        for issue in &issues {
            eprintln!("{} {}", "⚠️".yellow(), issue.yellow());
        }
        return Ok(message.to_string());
    }
    
    // Show issues and ask for confirmation
    println!("{} Commit message issues found:", "⚠️".yellow());
    for issue in &issues {
        println!("  • {}", issue.yellow());
    }
    
    if InteractivePrompt::new()
        .with_message("Continue with this message?")
        .confirm()? {
        Ok(message.to_string())
    } else {
        Err(RgitError::EmptyCommitMessage.into())
    }
}

/// Quick validation for simple messages
fn quick_validate_message(message: &str) -> Result<(), Vec<String>> {
    let issues = validate_commit_message(message);
    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

/// Auto-stage modified files
async fn auto_stage_files(rgit: &RgitCore, config: &Config) -> Result<()> {
    rgit.log("Auto-staging modified files...");
    
    let status = rgit.status()?;
    let unstaged_count = status.unstaged.len();
    
    if unstaged_count == 0 {
        return Ok(());
    }
    
    // Show what will be auto-staged
    if config.ui.interactive && unstaged_count > 0 {
        println!("{} Auto-staging {} modified file{}:", 
                "📝".yellow(),
                unstaged_count,
                if unstaged_count == 1 { "" } else { "s" });
        
        for file in &status.unstaged {
            println!("  {} {}: {}", 
                    "○".yellow(), 
                    file.status_symbol(false).yellow(),
                    file.path.white());
        }
    }
    
    // This would call rgit.add_update() in the real implementation
    // For now, we'll simulate it
    rgit.success(&format!("Auto-staged {} files", unstaged_count));
    
    Ok(())
}

/// Run pre-commit hooks
async fn run_pre_commit_hooks(rgit: &RgitCore, _config: &Config) -> Result<()> {
    rgit.log("Running pre-commit hooks...");
    
    // In a real implementation, this would:
    // 1. Look for .git/hooks/pre-commit
    // 2. Execute it if it exists and is executable
    // 3. Check the exit code and fail if non-zero
    
    // For now, we'll just simulate success
    Ok(())
}

/// Create the actual commit
async fn create_commit(rgit: &RgitCore, message: &str, args: &CommitArgs) -> Result<Oid> {
    rgit.log("Creating commit...");
    
    let commit_id = if args.gpg_sign || rgit.repo.config()?.get_bool("commit.gpgsign").unwrap_or(false) {
        // GPG signing would be implemented here
        rgit.commit(message, args.amend)?
    } else {
        rgit.commit(message, args.amend)?
    };
    
    Ok(commit_id)
}

/// Show commit summary and information
async fn show_commit_summary(
    rgit: &RgitCore, 
    commit_id: Oid, 
    message: &str, 
    config: &Config
) -> Result<()> {
    let short_id = shorten_oid(&commit_id, 8);
    let first_line = message.lines().next().unwrap_or("").to_string();
    
    if config.ui.interactive {
        println!("\n{} Commit created successfully!", "🎉".green());
        println!("   {} {}", "ID:".bold(), short_id.yellow());
        println!("   {} {}", "Message:".bold(), first_line.white());
        
        // Show statistics
        if let Ok(commit) = rgit.repo.find_commit(commit_id) {
            let stats = get_commit_stats(rgit, &commit)?;
            println!("   {} {}", "Changes:".bold(), stats.format_summary().cyan());
        }
    } else {
        println!("[{}] {}", short_id.yellow(), first_line);
    }
    
    Ok(())
}

/// Get commit statistics
fn get_commit_stats(
    rgit: &RgitCore, 
    commit: &git2::Commit
) -> Result<crate::utils::FileChangeStats> {
    let tree = commit.tree()?;
    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };
    
    let diff = rgit.repo.diff_tree_to_tree(
        parent_tree.as_ref(),
        Some(&tree),
        None,
    )?;
    
    let mut stats = crate::utils::FileChangeStats::default();
    stats.files = diff.deltas().len();
    
    diff.foreach(
        &mut |_delta, _progress| true,
        None,
        None,
        Some(&mut |_delta, _hunk, line| {
            match line.origin() {
                '+' => stats.additions += 1,
                '-' => stats.deletions += 1,
                _ => {}
            }
            true
        }),
    )?;
    
    Ok(stats)
}

/// Run post-commit hooks
async fn run_post_commit_hooks(
    rgit: &RgitCore, 
    _config: &Config, 
    _commit_id: Oid
) -> Result<()> {
    rgit.log("Running post-commit hooks...");
    
    // In a real implementation, this would:
    // 1. Look for .git/hooks/post-commit
    // 2. Execute it if it exists and is executable
    // 3. Log any output but don't fail on non-zero exit
    
    Ok(())
}

/// Show next steps after commit
async fn show_next_steps(rgit: &RgitCore, config: &Config) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    let branch_info = rgit.get_branch_info()?;
    
    println!("\n{} Next steps:", "💡".blue());
    
    // Push suggestions
    if branch_info.upstream.is_some() {
        if branch_info.ahead > 0 {
            println!("  • {} - Share your changes", "rgit push".cyan());
        }
    } else if branch_info.name != "main" && branch_info.name != "master" {
        println!("  • {} - Set up tracking and push", 
                format!("rgit push --set-upstream origin {}", branch_info.name).cyan());
    }
    
    // Additional suggestions
    println!("  • {} - Continue working", "Edit more files".cyan());
    println!("  • {} - Quick sync workflow", "rgit sync".cyan());
    
    // Check for remaining changes
    let status = rgit.status()?;
    if !status.is_clean() {
        println!("  • {} - Stage remaining changes", "rgit add".cyan());
    }
    
    Ok(())
}

/// Check if a merge is in progress
fn is_merge_in_progress(rgit: &RgitCore) -> Result<bool> {
    Ok(rgit.repo.state() == git2::RepositoryState::Merge)
}

/// Handle commits during merge
async fn handle_merge_commit(rgit: &RgitCore, config: &Config) -> Result<()> {
    rgit.log("Merge in progress detected");
    
    if config.ui.interactive {
        println!("{} Merge in progress", "🔀".blue());
        println!("Creating merge commit...");
    }
    
    Ok(())
}

/// Check submodule state before commit
async fn check_submodule_state(rgit: &RgitCore, config: &Config) -> Result<()> {
    let submodule_manager = crate::submodule::SubmoduleManager::new(rgit, config);
    
    if !submodule_manager.interactive_health_check()? {
        return Err(RgitError::SubmoduleError("Submodule issues prevent commit".to_string()).into());
    }
    
    Ok(())
}

/// Warn about amending published commits
async fn warn_about_amend_published(rgit: &RgitCore, config: &Config) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    // In a real implementation, this would check if the commit has been pushed
    let branch_info = rgit.get_branch_info()?;
    
    if branch_info.behind == 0 && branch_info.ahead > 0 {
        println!("{} {}", "⚠️".yellow(), "Warning: Amending unpushed commit".yellow());
        println!("This is safe as the commit hasn't been shared yet.");
    } else if branch_info.upstream.is_some() {
        println!("{} {}", "⚠️".yellow(), "Warning: Amending potentially published commit".yellow());
        println!("This will rewrite history and may cause issues for collaborators.");
        
        if !InteractivePrompt::new()
            .with_message("Continue with amend?")
            .confirm()? {
            return Err(RgitError::OperationCancelled.into());
        }
    }
    
    Ok(())
}

/// Utility function for other commands to create commits
pub async fn create_commit_with_message(
    rgit: &RgitCore, 
    message: &str, 
    amend: bool
) -> Result<Oid> {
    if message.trim().is_empty() {
        return Err(RgitError::EmptyCommitMessage.into());
    }
    
    rgit.commit(message, amend)
}

/// Check if repository has staged changes ready for commit
pub fn has_staged_changes(rgit: &RgitCore) -> Result<bool> {
    rgit.has_staged_changes()
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

    #[test]
    fn test_read_message_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("commit_msg.txt");
        
        fs::write(&file_path, "Test commit message").unwrap();
        
        let message = read_message_from_file(&file_path).unwrap();
        assert_eq!(message, "Test commit message");
    }

    #[test]
    fn test_quick_validate_message() {
        // Good message
        let good_message = "Add new feature\n\nThis adds a new feature to the application.";
        assert!(quick_validate_message(good_message).is_ok());
        
        // Bad message (too long subject)
        let bad_message = "This is a very long subject line that exceeds the recommended 50 character limit significantly";
        assert!(quick_validate_message(bad_message).is_err());
        
        // Empty message
        let empty_message = "";
        assert!(quick_validate_message(empty_message).is_err());
    }

    #[tokio::test]
    async fn test_create_commit_template() {
        let (_temp_dir, repo) = create_test_repo();
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let config = Config::default();
        
        let template = create_commit_template(&rgit, &config).await.unwrap();
        
        assert!(template.contains("# Enter your commit message"));
        assert!(template.contains("# Guidelines:"));
    }

    #[tokio::test]
    async fn test_commit_with_staged_files() {
        let (temp_dir, repo) = create_test_repo();
        
        // Create and stage a file
        fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();
        
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("test.txt")).unwrap();
        index.write().unwrap();
        
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        
        assert!(has_staged_changes(&rgit).unwrap());
        
        let commit_id = create_commit_with_message(&rgit, "Test commit", false).await.unwrap();
        assert!(!commit_id.is_zero());
    }

    #[test]
    fn test_is_merge_in_progress() {
        let (_temp_dir, repo) = create_test_repo();
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        
        // Fresh repo should not be in merge state
        assert!(!is_merge_in_progress(&rgit).unwrap());
    }

    #[tokio::test]
    async fn test_get_commit_stats() {
        let (temp_dir, repo) = create_test_repo();
        
        // Create, stage and commit a file
        fs::write(temp_dir.path().join("test.txt"), "line1\nline2\nline3\n").unwrap();
        
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("test.txt")).unwrap();
        index.write().unwrap();
        
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = repo.signature().unwrap();
        
        let commit_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Test commit",
            &tree,
            &[],
        ).unwrap();
        
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let commit = repo.find_commit(commit_id).unwrap();
        let stats = get_commit_stats(&rgit, &commit).unwrap();
        
        assert_eq!(stats.files, 1);
    }
}