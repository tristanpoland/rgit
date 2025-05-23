use anyhow::Result;
use colored::*;
use git2::{Repository, AnnotatedCommit, FetchOptions, RemoteCallbacks};
use std::io::{self, Write};

use crate::cli::PullArgs;
use crate::config::Config;
use crate::core::RgitCore;
use crate::error::RgitError;
use crate::interactive::InteractivePrompt;

/// Execute the pull command
pub async fn execute(args: &PullArgs, rgit: &RgitCore, config: &Config) -> Result<()> {
    println!("{} Pulling changes...", "🔄".blue().bold());
    
    let repo = &rgit.repo;
    
    // Check for uncommitted changes
    let status = rgit.status()?;
    if !status.is_clean() && !args.force {
        println!("{} You have uncommitted changes:", "⚠️".yellow().bold());
        
        if !status.staged.is_empty() {
            println!("  {} {} staged files", "📝".green(), status.staged.len());
        }
        if !status.unstaged.is_empty() {
            println!("  {} {} unstaged files", "📝".yellow(), status.unstaged.len());
        }
        if !status.untracked.is_empty() {
            println!("  {} {} untracked files", "❓".red(), status.untracked.len());
        }
        
        if config.is_interactive() {
            println!("\nOptions:");
            println!("  • {} - Stash changes and pull", "rgit stash && rgit pull".cyan());
            println!("  • {} - Commit changes and pull", "rgit commit && rgit pull".cyan());
            println!("  • {} - Force pull (may lose changes)", "rgit pull --force".red());
            
            let continue_anyway = InteractivePrompt::new()
                .with_message("Continue with pull anyway?")
                .confirm()?;
            
            if !continue_anyway {
                return Ok(());
            }
        } else {
            return Err(RgitError::UncommittedChanges.into());
        }
    }
    
    // Determine remote and branch
    let (remote_name, branch_name) = determine_pull_source(repo, args)?;
    
    println!("{} Remote: {}", "📡".blue(), remote_name.cyan());
    println!("{} Branch: {}", "🌿".green(), branch_name.yellow());
    
    // Fetch first
    let fetch_head = perform_fetch(repo, &remote_name, &branch_name, config).await?;
    
    // Determine merge strategy
    if args.rebase {
        perform_rebase(repo, &fetch_head, config).await?;
    } else {
        perform_merge(repo, &fetch_head, args, config).await?;
    }
    
    println!("{} Pull completed successfully", "✅".green().bold());
    
    // Show summary
    show_pull_summary(repo, &remote_name, &branch_name, config)?;
    
    Ok(())
}

/// Determine what remote and branch to pull from
fn determine_pull_source(repo: &Repository, args: &PullArgs) -> Result<(String, String)> {
    let remote_name = args.remote.clone()
        .or_else(|| get_upstream_remote(repo))
        .unwrap_or_else(|| "origin".to_string());
    
    let branch_name = args.branch.clone()
        .or_else(|| get_upstream_branch(repo))
        .or_else(|| get_current_branch_name(repo))
        .ok_or_else(|| RgitError::NoUpstreamBranch)?;
    
    Ok((remote_name, branch_name))
}

/// Get the upstream remote for the current branch
fn get_upstream_remote(repo: &Repository) -> Option<String> {
    if let Ok(head) = repo.head() {
        if let Some(branch_name) = head.shorthand() {
            let config = repo.config().ok()?;
            let remote_key = format!("branch.{}.remote", branch_name);
            config.get_string(&remote_key).ok()
        } else {
            None
        }
    } else {
        None
    }
}

/// Get the upstream branch for the current branch
fn get_upstream_branch(repo: &Repository) -> Option<String> {
    if let Ok(head) = repo.head() {
        if let Some(branch_name) = head.shorthand() {
            let config = repo.config().ok()?;
            let merge_key = format!("branch.{}.merge", branch_name);
            let merge_ref = config.get_string(&merge_key).ok()?;
            
            // Extract branch name from refs/heads/branch_name
            if merge_ref.starts_with("refs/heads/") {
                Some(merge_ref.strip_prefix("refs/heads/")?.to_string())
            } else {
                Some(merge_ref)
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// Get current branch name
fn get_current_branch_name(repo: &Repository) -> Option<String> {
    repo.head().ok()?.shorthand().map(|s| s.to_string())
}

/// Perform fetch operation
async fn perform_fetch<'a>(
    repo: &'a Repository,
    remote_name: &str,
    branch_name: &str,
    config: &Config,
) -> Result<AnnotatedCommit<'a>> {
    println!("{} Fetching from {}/{}", "📥".blue(), remote_name.cyan(), branch_name.yellow());
    
    let mut remote = repo.find_remote(remote_name)
        .map_err(|_| RgitError::RemoteNotFound(remote_name.to_string()))?;
    
    // Set up callbacks
    let mut callbacks = RemoteCallbacks::new();
    
    if config.ui.interactive {
        callbacks.pack_progress(|_stage, current, total| {
            if total > 0 {
                let percentage = (current * 100) / total;
                print!("\r{} Progress: {}%", "📦".blue(), percentage);
                let _ = io::stdout().flush();
            }
        });
    }
    
    // Set up authentication
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
    });
    
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);
    
    // Perform fetch
    let refspec = format!("refs/heads/{}:refs/remotes/{}/{}", 
                         branch_name, remote_name, branch_name);
    
    remote.fetch(&[&refspec], Some(&mut fetch_options), None)
        .map_err(|e| RgitError::FetchFailed(e.message().to_string()))?;
    
    if config.ui.interactive {
        println!(); // New line after progress
    }
    
    // Get the fetched commit
    let fetch_head_ref = format!("refs/remotes/{}/{}", remote_name, branch_name);
    let fetch_oid = repo.refname_to_id(&fetch_head_ref)
        .map_err(|_| RgitError::BranchNotFound(fetch_head_ref.clone()))?;
    
    let fetch_commit = repo.find_commit(fetch_oid)?;
    let fetch_head = repo.reference_to_annotated_commit(&repo.find_reference(&fetch_head_ref)?)?;
    
    println!("{} Fetched {} ({})", 
            "✅".green(), 
            fetch_commit.id().to_string()[..8].yellow(),
            fetch_commit.summary().unwrap_or("No message").white());
    
    Ok(fetch_head)
}

/// Perform merge operation
async fn perform_merge<'a>(
    repo: &'a Repository,
    fetch_head: &AnnotatedCommit<'a>,
    args: &PullArgs,
    config: &Config,
) -> Result<()> {
    let analysis = repo.merge_analysis(&[fetch_head])?;
    
    if analysis.0.is_fast_forward() {
        println!("{} Fast-forward merge", "⚡".yellow());
        perform_fast_forward_merge(repo, fetch_head)?;
    } else if analysis.0.is_normal() {
        // Check if ff_only is set
        if args.ff_only {
            return Err(RgitError::FastForwardNotPossible.into());
        }
        
        println!("{} Creating merge commit", "🔀".blue());
        perform_normal_merge(repo, fetch_head, config).await?;
    } else if analysis.0.is_up_to_date() {
        println!("{} Already up to date", "✅".green());
    } else {
        return Err(RgitError::MergeNotPossible.into());
    }
    
    Ok(())
}

/// Perform fast-forward merge
fn perform_fast_forward_merge(repo: &Repository, fetch_head: &AnnotatedCommit) -> Result<()> {
    let target_oid = fetch_head.id();
    
    // Update HEAD to point to the new commit
    let mut head_ref = repo.head()?;
    head_ref.set_target(target_oid, "Fast-forward merge")?;
    
    // Update working directory
    repo.set_head(head_ref.name().unwrap())?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
    
    Ok(())
}

/// Perform normal merge (creates merge commit)
async fn perform_normal_merge<'a>(
    repo: &'a Repository,
    fetch_head: &AnnotatedCommit<'a>,
    config: &Config,
) -> Result<()> {
    // Check for merge conflicts first
    let mut index = repo.index()?;
    repo.merge(&[fetch_head], None, None)?;
    
    if index.has_conflicts() {
        handle_merge_conflicts(repo, config).await?;
    }
    
    // Create merge commit
    let signature = get_signature(repo)?;
    let head_commit = repo.head()?.peel_to_commit()?;
    let fetch_commit = repo.find_commit(fetch_head.id())?;
    
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    
    let message = format!("Merge branch '{}' into {}", 
                         fetch_commit.summary().unwrap_or("unknown"),
                         head_commit.summary().unwrap_or("HEAD"));
    
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &tree,
        &[&head_commit, &fetch_commit],
    )?;
    
    // Clean up merge state
    repo.cleanup_state()?;
    
    Ok(())
}

/// Handle merge conflicts
async fn handle_merge_conflicts<'a>(repo: &'a Repository, config: &Config) -> Result<()> {
    println!("{} Merge conflicts detected!", "⚠️".red().bold());
    
    let index = repo.index()?;
    let conflicts: Vec<_> = index.conflicts()?.collect();
    
    println!("{} Conflicted files:", "📝".yellow());
    let mut conflict_files = Vec::new();
    for conflict in &conflicts {
        if let Ok(index_conflict) = conflict {
            if let Some(our_entry) = &index_conflict.our {
                if let Ok(path) = std::str::from_utf8(&our_entry.path) {
                    println!("  {} {}", "⚡".red(), path.yellow());
                    conflict_files.push(path.to_string());
                }
            }
        }
    }
    
    if config.is_interactive() {
        println!("\n{} Resolution options:", "💡".blue());
        println!("  • Manually resolve conflicts in your editor");
        println!("  • {} - Mark files as resolved", "rgit add <file>".cyan());
        println!("  • {} - Complete the merge", "rgit commit".cyan());
        println!("  • {} - Abort the merge", "rgit merge --abort".red());
        
        InteractivePrompt::new()
            .with_message("Resolve conflicts manually, then continue")
            .confirm()?;
    } else {
        // Return error with list of conflicted files
        return Err(RgitError::MergeConflict(conflict_files).into());
    }
    
    Ok(())
}

/// Perform rebase operation
async fn perform_rebase<'a>(
    repo: &'a Repository,
    fetch_head: &AnnotatedCommit<'a>,
    _config: &Config,
) -> Result<()> {
    println!("{} Rebasing current branch", "🔄".blue());
    
    let signature = get_signature(repo)?;
    
    // Get the current branch
    let head = repo.head()?;
    let head_annotated = repo.reference_to_annotated_commit(&head)?;
    
    // Start rebase
    let mut rebase = repo.rebase(
        Some(&head_annotated),
        None,  // Use None for upstream base
        Some(fetch_head),
        None,
    )?;
    
    // Process rebase operations
    while let Some(operation) = rebase.next() {
        match operation {
            Ok(op) => {
                println!("  {} Applying: {}", "✅".green(), 
                        repo.find_commit(op.id())?.summary().unwrap_or("No message"));
                rebase.commit(None, &signature, None)?;
            }
            Err(e) => {
                println!("{} Rebase conflict: {}", "⚠️".red(), e.message());
                return Err(RgitError::RebaseConflict(e.message().to_string()).into());
            }
        }
    }
    
    // Finish rebase
    rebase.finish(Some(&signature))?;
    
    println!("{} Rebase completed", "✅".green());
    
    Ok(())
}

/// Get git signature for commits
fn get_signature(repo: &Repository) -> Result<git2::Signature<'_>> {
    let config = repo.config()?;
    let name = config.get_string("user.name")
        .unwrap_or_else(|_| "Unknown User".to_string());
    let email = config.get_string("user.email")
        .unwrap_or_else(|_| "unknown@example.com".to_string());
    
    Ok(git2::Signature::now(&name, &email)?)
}

/// Show summary after pull
fn show_pull_summary(
    repo: &Repository,
    remote_name: &str,
    branch_name: &str,
    config: &Config,
) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    println!("\n{} Pull Summary:", "📊".blue().bold());
    
    // Show current HEAD
    if let Ok(head) = repo.head() {
        if let Ok(commit) = head.peel_to_commit() {
            println!("  {} Current commit: {}", "📝".yellow(), 
                    commit.id().to_string()[..8].yellow());
            
            if let Some(summary) = commit.summary() {
                println!("    {} {}", "💬".blue(), summary.white());
            }
        }
    }
    
    // Show remote tracking
    println!("  {} Tracking: {}/{}", "🔗".green(), remote_name.cyan(), branch_name.cyan());
    
    // Show next steps
    println!("\n{} Next steps:", "💡".blue());
    println!("  • {} - View recent changes", "rgit log".cyan());
    println!("  • {} - Check repository status", "rgit status".cyan());
    println!("  • {} - Push changes if any", "rgit push".cyan());
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
    fn test_get_current_branch_name() {
        let (_temp_dir, repo) = create_test_repo();
        
        // Create initial commit
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        ).unwrap();
        
        let branch = get_current_branch_name(&repo);
        assert!(branch.is_some());
        let name = branch.unwrap();
        assert!(name == "master" || name == "main");
    }

    #[test]
    fn test_get_signature() {
        let (_temp_dir, repo) = create_test_repo();
        
        let signature = get_signature(&repo).unwrap();
        assert_eq!(signature.name().unwrap(), "Test User");
        assert_eq!(signature.email().unwrap(), "test@example.com");
    }

    #[test]
    fn test_determine_pull_source() {
        let (_temp_dir, repo) = create_test_repo();
        
        let args = PullArgs {
            remote: Some("origin".to_string()),
            branch: Some("main".to_string()),
            rebase: false,
            no_edit: false,
            no_commit: false,
            force: false,
            ff_only: false,
        };
        
        let (remote, branch) = determine_pull_source(&repo, &args).unwrap();
        assert_eq!(remote, "origin");
        assert_eq!(branch, "main");
    }
}