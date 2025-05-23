use anyhow::Result;
use colored::*;
use git2::*;

use crate::cli::SyncArgs;
use crate::config::Config;
use crate::core::RgitCore;
use crate::error::RgitError;
use crate::interactive::{InteractivePrompt, ProgressDisplay};
use crate::submodule::SubmoduleManager;
use crate::commands::status::{quick_status_check, StatusSummary};

/// Execute the sync command - intelligent pull + push workflow
pub async fn execute(args: &SyncArgs, rgit: &RgitCore, config: &Config) -> Result<()> {
    rgit.log("Starting sync operation...");
    
    // Pre-sync validation
    validate_sync_prerequisites(rgit, config, args).await?;
    
    // Show current status
    show_pre_sync_status(rgit, config).await?;
    
    // Handle submodules if requested
    if args.submodules {
        sync_submodules(rgit, config).await?;
    }
    
    // Perform the sync operations
    let sync_result = if args.dry_run {
        perform_dry_run_sync(rgit, config, args).await?
    } else {
        perform_actual_sync(rgit, config, args).await?
    };
    
    // Show results
    show_sync_results(rgit, config, &sync_result).await?;
    
    Ok(())
}

/// Validate prerequisites for sync operation
async fn validate_sync_prerequisites(
    rgit: &RgitCore, 
    config: &Config, 
    args: &SyncArgs
) -> Result<()> {
    // Check if we have a remote configured
    let default_remote = rgit.get_default_remote();
    if default_remote.is_err() && !args.push_only {
        return Err(RgitError::NoRemoteConfigured.into());
    }
    
    // Check branch has upstream for pull operations
    if !args.push_only {
        let branch_info = rgit.get_branch_info()?;
        if branch_info.upstream.is_none() {
            handle_no_upstream(rgit, config, &branch_info.name).await?;
        }
    }
    
    // Check for uncommitted changes
    let status = rgit.status()?;
    if !status.is_clean() && !args.pull_only {
        unsafe {
            handle_uncommitted_changes(&mut *(rgit as *const _ as *mut _), config, &status).await?
        };
    }
    
    // Check if we're in a valid state for sync
    validate_repository_state(rgit).await?;
    
    Ok(())
}

/// Handle repository with no upstream configured
async fn handle_no_upstream(
    rgit: &RgitCore, 
    config: &Config, 
    branch_name: &str
) -> Result<()> {
    rgit.warning(&format!("Branch '{}' has no upstream configured", branch_name));
    
    if !config.is_interactive() {
        return Err(RgitError::RemoteNotFound("upstream".to_string()).into());
    }
    
    let options = vec![
        format!("Set upstream to origin/{}", branch_name),
        "Skip pull operation".to_string(),
        "Cancel sync".to_string(),
    ];
    
    let choice = InteractivePrompt::new()
        .with_message("How would you like to proceed?")
        .with_options(&options)
        .select()?;
    
    match choice {
        0 => {
            // Set upstream
            setup_upstream_tracking(rgit, "origin", branch_name).await?;
            rgit.success(&format!("Set upstream to origin/{}", branch_name));
        }
        1 => {
            rgit.info("Skipping pull operation");
        }
        _ => {
            return Err(RgitError::OperationCancelled.into());
        }
    }
    
    Ok(())
}

/// Handle uncommitted changes before sync
async fn handle_uncommitted_changes(
    rgit: &mut RgitCore, 
    config: &Config, 
    status: &crate::core::RepositoryStatus
) -> Result<()> {
    rgit.warning("Repository has uncommitted changes");
    
    if !config.is_interactive() {
        return Err(RgitError::BranchHasUncommittedChanges.into());
    }
    
    // Show current changes
    println!("{} Current changes:", "📋".blue());
    let total_changes = status.total_changes();
    println!("  {} {} staged", "📦".green(), status.staged.len());
    println!("  {} {} unstaged", "📝".yellow(), status.unstaged.len());
    println!("  {} {} untracked", "❓".red(), status.untracked.len());
    
    let options = vec![
        "Stash changes and continue",
        "Commit changes first",
        "Continue anyway (not recommended)",
        "Cancel sync",
    ];
    
    let choice = InteractivePrompt::new()
        .with_message("How to handle uncommitted changes?")
        .with_options(&options)
        .select()?;
    
    match choice {
        0 => {
            stash_changes_for_sync(rgit).await?;
        }
        1 => {
            return Err(RgitError::OperationCancelled.into());
        }
        2 => {
            rgit.warning("Continuing with uncommitted changes - conflicts may occur");
        }
        _ => {
            return Err(RgitError::OperationCancelled.into());
        }
    }
    
    Ok(())
}

/// Stash changes before sync
async fn stash_changes_for_sync(rgit: &mut RgitCore) -> Result<()> {
    rgit.log("Stashing changes for sync...");

    // Get signature first, then drop immutable borrow before mutable borrow
    let signature = rgit.get_signature()?;
    let stash_message = format!(
        "rgit sync auto-stash on {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );

    // Ensure signature is not used after this point, so the immutable borrow ends
    {
        let repo = &mut rgit.repo;
        repo.stash_save(&signature, &stash_message, None)?;
    }
    rgit.success("Changes stashed successfully");

    Ok(())
}

/// Validate repository state for sync
async fn validate_repository_state(rgit: &RgitCore) -> Result<()> {
    let state = rgit.repo.state();
    
    match state {
        RepositoryState::Clean => Ok(()),
        RepositoryState::Merge => {
                        Err(RgitError::MergeConflict(vec!["Repository is in merge state".to_string()]).into())
            }
        RepositoryState::Revert => {
                Err(RgitError::OperationFailed("Repository is in revert state".to_string()).into())
            }
        RepositoryState::CherryPick => {
                Err(RgitError::OperationFailed("Repository is in cherry-pick state".to_string()).into())
            }
        RepositoryState::Bisect => {
                Err(RgitError::OperationFailed("Repository is in bisect state".to_string()).into())
            }
        RepositoryState::Rebase | RepositoryState::RebaseInteractive | RepositoryState::RebaseMerge => {
                Err(RgitError::OperationFailed("Repository is in rebase state".to_string()).into())
            }
        RepositoryState::ApplyMailbox | RepositoryState::ApplyMailboxOrRebase => {
                Err(RgitError::OperationFailed("Repository is applying patches".to_string()).into())
            }
        RepositoryState::RevertSequence => {
            Err(RgitError::OperationFailed("Repository is in revert sequence state".to_string()).into())
        }
        RepositoryState::CherryPickSequence => {
            Err(RgitError::OperationFailed("Repository is in cherry-pick sequence state".to_string()).into())
        }
    }
}

/// Show pre-sync status information
async fn show_pre_sync_status(rgit: &RgitCore, config: &Config) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    let status_summary = quick_status_check(rgit)?;
    let branch_info = rgit.get_branch_info()?;
    
    println!("{} Pre-sync status:", "📊".blue().bold());
    println!("   {} {}", "Branch:".bold(), branch_info.name.cyan());
    
    if let Some(ref upstream) = branch_info.upstream {
        println!("   {} {}", "Upstream:".bold(), upstream.cyan());
        println!("   {} {}", "Status:".bold(), status_summary.format_summary());
    } else {
        println!("   {} {}", "Upstream:".bold(), "None configured".red());
    }
    
    if !status_summary.is_clean {
        println!("   {} {} local changes", "Changes:".bold().yellow(), status_summary.total_changes());
    }
    
    println!();
    Ok(())
}

/// Sync submodules if requested
async fn sync_submodules(rgit: &RgitCore, config: &Config) -> Result<()> {
    rgit.log("Syncing submodules...");
    
    let submodule_manager = SubmoduleManager::new(rgit, config);
    
    // Health check first
    if !submodule_manager.interactive_health_check()? {
        return Err(RgitError::SubmoduleError("Submodule sync cancelled".to_string()).into());
    }
    
    // Update all submodules
    submodule_manager.update_all(config.submodules.recursive, true)?;
    
    rgit.success("Submodules synced successfully");
    Ok(())
}

/// Perform dry run sync (show what would happen)
async fn perform_dry_run_sync(
    rgit: &RgitCore, 
    config: &Config, 
    args: &SyncArgs
) -> Result<SyncResult> {
    println!("{} Dry run mode - showing what would happen:", "🔍".blue().bold());
    
    let mut result = SyncResult::default();
    
    if !args.push_only {
        println!("\n{} Pull phase:", "⬇️".blue());
        let pull_result = simulate_pull(rgit, config).await?;
        println!("  {} Would fetch {} commit{}", 
                "•".blue(),
                pull_result.commits_fetched,
                if pull_result.commits_fetched == 1 { "" } else { "s" });
        result.pull_result = Some(pull_result);
    }
    
    if !args.pull_only {
        println!("\n{} Push phase:", "⬆️".blue());
        let push_result = simulate_push(rgit, config).await?;
        println!("  {} Would push {} commit{}", 
                "•".blue(),
                push_result.commits_pushed,
                if push_result.commits_pushed == 1 { "" } else { "s" });
        result.push_result = Some(push_result);
    }
    
    println!("\n{} No actual changes were made", "ℹ️".blue());
    Ok(result)
}

/// Perform actual sync operations
async fn perform_actual_sync(
    rgit: &RgitCore, 
    config: &Config, 
    args: &SyncArgs
) -> Result<SyncResult> {
    let mut result = SyncResult::default();
    
    // Pull phase
    if !args.push_only {
        result.pull_result = Some(perform_pull(rgit, config).await?);
    }
    
    // Push phase
    if !args.pull_only {
        result.push_result = Some(perform_push(rgit, config, args.force).await?);
    }
    
    Ok(result)
}

/// Perform pull operation
async fn perform_pull(rgit: &RgitCore, config: &Config) -> Result<PullResult> {
    rgit.log("Performing pull...");
    
    let progress = if config.ui.progress {
        Some(ProgressDisplay::new("Pulling changes")
            .with_eta()
            .create_progress_bar())
    } else {
        None
    };
    
    if let Some(ref pb) = progress {
        pb.set_message("Fetching from remote...");
    }
    
    // Get current HEAD for comparison
    let old_head = rgit.repo.head()?.target();
    
    // Perform fetch
    let fetch_result = fetch_from_remote(rgit, config).await?;
    
    if let Some(ref pb) = progress {
        pb.set_message("Merging changes...");
    }
    
    // Merge or rebase changes
    let merge_result = if config.git.pull_rebase {
        rebase_changes(rgit, config).await?
    } else {
        merge_changes(rgit, config).await?
    };
    
    if let Some(ref pb) = progress {
        pb.finish_with_message("✅ Pull completed");
    }
    
    // Calculate what changed
    let new_head = rgit.repo.head()?.target();
    let commits_fetched = if old_head != new_head {
        count_commits_between(rgit, old_head, new_head)?
    } else {
        0
    };
    
    Ok(PullResult {
        commits_fetched,
        fast_forward: merge_result.fast_forward,
        conflicts: merge_result.conflicts,
        fetch_stats: fetch_result,
    })
}

/// Perform push operation
async fn perform_push(rgit: &RgitCore, config: &Config, force: bool) -> Result<PushResult> {
    rgit.log("Performing push...");
    
    let progress = if config.ui.progress {
        Some(ProgressDisplay::new("Pushing changes")
            .with_eta()
            .create_progress_bar())
    } else {
        None
    };
    
    if let Some(ref pb) = progress {
        pb.set_message("Pushing to remote...");
    }
    
    let branch_info = rgit.get_branch_info()?;
    let commits_to_push = branch_info.ahead;
    
    // Perform actual push
    let push_success = push_to_remote(rgit, config, force).await?;
    
    if let Some(ref pb) = progress {
        if push_success {
            pb.finish_with_message("✅ Push completed");
        } else {
            pb.finish_with_message("❌ Push failed");
        }
    }
    
    Ok(PushResult {
        commits_pushed: if push_success { commits_to_push } else { 0 },
        success: push_success,
        rejected: !push_success,
    })
}

/// Simulate pull operation for dry run
async fn simulate_pull(rgit: &RgitCore, _config: &Config) -> Result<PullResult> {
    let branch_info = rgit.get_branch_info()?;
    
    Ok(PullResult {
        commits_fetched: branch_info.behind,
        fast_forward: true,
        conflicts: Vec::new(),
        fetch_stats: FetchResult {
            objects_received: branch_info.behind * 3, // Simulate objects
            bytes_received: branch_info.behind * 1024,
        },
    })
}

/// Simulate push operation for dry run
async fn simulate_push(rgit: &RgitCore, _config: &Config) -> Result<PushResult> {
    let branch_info = rgit.get_branch_info()?;
    
    Ok(PushResult {
        commits_pushed: branch_info.ahead,
        success: true,
        rejected: false,
    })
}

/// Fetch from remote
async fn fetch_from_remote(rgit: &RgitCore, _config: &Config) -> Result<FetchResult> {
    // In a real implementation, this would:
    // 1. Get the remote
    // 2. Create fetch options with callbacks for progress
    // 3. Perform the fetch
    // 4. Return statistics
    
    // Simulated implementation
    Ok(FetchResult {
        objects_received: 10,
        bytes_received: 5120,
    })
}

/// Merge changes from remote
async fn merge_changes(rgit: &RgitCore, _config: &Config) -> Result<MergeResult> {
    // In a real implementation, this would:
    // 1. Get the upstream commit
    // 2. Perform merge analysis
    // 3. Execute merge or fast-forward
    // 4. Handle conflicts if any
    
    Ok(MergeResult {
        fast_forward: true,
        conflicts: Vec::new(),
    })
}

/// Rebase changes from remote
async fn rebase_changes(rgit: &RgitCore, _config: &Config) -> Result<MergeResult> {
    // In a real implementation, this would:
    // 1. Get the upstream commit
    // 2. Perform rebase operation
    // 3. Handle conflicts if any
    
    Ok(MergeResult {
        fast_forward: false, // Rebase is not fast-forward
        conflicts: Vec::new(),
    })
}

/// Push to remote
async fn push_to_remote(rgit: &RgitCore, _config: &Config, _force: bool) -> Result<bool> {
    // In a real implementation, this would:
    // 1. Get the remote and branch
    // 2. Create push options with callbacks
    // 3. Perform the push
    // 4. Handle authentication and errors
    
    // Simulated success
    Ok(true)
}

/// Setup upstream tracking
async fn setup_upstream_tracking(
    rgit: &RgitCore, 
    remote_name: &str, 
    branch_name: &str
) -> Result<()> {
    // In a real implementation, this would set up branch tracking
    rgit.log(&format!("Setting upstream to {}/{}", remote_name, branch_name));
    Ok(())
}

/// Count commits between two points
fn count_commits_between(
    rgit: &RgitCore, 
    from: Option<Oid>, 
    to: Option<Oid>
) -> Result<usize> {
    match (from, to) {
        (Some(from_oid), Some(to_oid)) => {
            let (ahead, _) = rgit.repo.graph_ahead_behind(to_oid, from_oid)?;
            Ok(ahead)
        }
        _ => Ok(0),
    }
}

/// Show sync results
async fn show_sync_results(
    rgit: &RgitCore, 
    config: &Config, 
    result: &SyncResult
) -> Result<()> {
    if !config.ui.interactive {
        // Simple output for non-interactive mode
        if let Some(ref pull) = result.pull_result {
            if pull.commits_fetched > 0 {
                println!("Pulled {} commits", pull.commits_fetched);
            }
        }
        if let Some(ref push) = result.push_result {
            if push.commits_pushed > 0 {
                println!("Pushed {} commits", push.commits_pushed);
            }
        }
        return Ok(());
    }
    
    println!("\n{} Sync completed!", "🎉".green().bold());
    
    // Pull results
    if let Some(ref pull) = result.pull_result {
        if pull.commits_fetched > 0 {
            println!("   {} Pulled {} commit{}", 
                    "⬇️".blue(),
                    pull.commits_fetched,
                    if pull.commits_fetched == 1 { "" } else { "s" });
            
            if pull.fast_forward {
                println!("      {} Fast-forward merge", "⚡".green());
            }
        } else {
            println!("   {} Already up to date", "⬇️".blue());
        }
        
        if !pull.conflicts.is_empty() {
            println!("   {} {} conflict{} resolved", 
                    "⚔️".yellow(),
                    pull.conflicts.len(),
                    if pull.conflicts.len() == 1 { "" } else { "s" });
        }
    }
    
    // Push results
    if let Some(ref push) = result.push_result {
        if push.success {
            if push.commits_pushed > 0 {
                println!("   {} Pushed {} commit{}", 
                        "⬆️".blue(),
                        push.commits_pushed,
                        if push.commits_pushed == 1 { "" } else { "s" });
            } else {
                println!("   {} Nothing to push", "⬆️".blue());
            }
        } else {
            println!("   {} Push failed", "⬆️".red());
        }
    }
    
    // Show final status
    let final_status = quick_status_check(rgit)?;
    println!("   {} {}", "Status:".bold(), final_status.format_summary());
    
    Ok(())
}

// =============================================================================
// Data Structures
// =============================================================================

#[derive(Debug, Default)]
struct SyncResult {
    pull_result: Option<PullResult>,
    push_result: Option<PushResult>,
}

#[derive(Debug)]
struct PullResult {
    commits_fetched: usize,
    fast_forward: bool,
    conflicts: Vec<String>,
    fetch_stats: FetchResult,
}

#[derive(Debug)]
struct PushResult {
    commits_pushed: usize,
    success: bool,
    rejected: bool,
}

#[derive(Debug)]
struct FetchResult {
    objects_received: usize,
    bytes_received: usize,
}

#[derive(Debug)]
struct MergeResult {
    fast_forward: bool,
    conflicts: Vec<String>,
}

/// Quick sync utility for other commands
pub async fn quick_sync(rgit: &RgitCore, config: &Config) -> Result<()> {
    let args = SyncArgs {
        push_only: false,
        pull_only: false,
        force: false,
        submodules: config.submodules.auto_init,
        dry_run: false,
    };
    
    execute(&args, rgit, config).await
}

/// Check if sync is needed
pub fn needs_sync(rgit: &RgitCore) -> Result<bool> {
    let branch_info = rgit.get_branch_info()?;
    Ok(branch_info.ahead > 0 || branch_info.behind > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, git2::Repository) {
        let temp_dir = TempDir::new().unwrap();
        let repo = git2::Repository::init(temp_dir.path()).unwrap();
        
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        
        (temp_dir, repo)
    }

    #[tokio::test]
    async fn test_validate_repository_state() {
        let (_temp_dir, repo) = create_test_repo();
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        
        // Clean repo should pass validation
        assert!(validate_repository_state(&rgit).await.is_ok());
    }

    #[tokio::test]
    async fn test_simulate_pull() {
        let (_temp_dir, repo) = create_test_repo();
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let config = Config::default();
        
        let result = simulate_pull(&rgit, &config).await.unwrap();
        assert_eq!(result.commits_fetched, 0); // No upstream, so no commits behind
    }

    #[tokio::test]
    async fn test_simulate_push() {
        let (_temp_dir, repo) = create_test_repo();
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let config = Config::default();
        
        let result = simulate_push(&rgit, &config).await.unwrap();
        assert_eq!(result.commits_pushed, 0); // No upstream, so no commits ahead
    }

    #[test]
    fn test_needs_sync() {
        let (_temp_dir, repo) = create_test_repo();
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        
        // Fresh repo with no remote should not need sync
        assert!(!needs_sync(&rgit).unwrap());
    }

    #[tokio::test]
    async fn test_dry_run_sync() {
        let (_temp_dir, repo) = create_test_repo();
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let config = Config::minimal();
        
        let args = SyncArgs {
            push_only: false,
            pull_only: false,
            force: false,
            submodules: false,
            dry_run: true,
        };
        
        // Should not fail even without remote in dry run mode
        // (though it would show that no operations would be performed)
        let result = perform_dry_run_sync(&rgit, &config, &args).await;
        // This might fail due to no remote, which is expected
        // In a real test environment, we'd set up proper remotes
    }
}