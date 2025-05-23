use anyhow::Result;
use colored::*;
use git2::{FetchOptions, RemoteCallbacks, Repository};
use std::io::{self, Write};

use crate::cli::FetchArgs;
use crate::config::Config;
use crate::core::RgitCore;
use crate::error::RgitError;

/// Execute the fetch command
pub async fn execute(args: &FetchArgs, rgit: &RgitCore, config: &Config) -> Result<()> {
    println!("{} Fetching from remote repositories...", "📥".blue().bold());
    
    let repo = &rgit.repo;
    
    if args.all {
        fetch_all_remotes(repo, config).await?;
    } else {
        let remote_name = args.remote.as_deref().unwrap_or("origin");
        fetch_single_remote(repo, remote_name, args, config).await?;
    }
    
    // Show fetch results
    show_fetch_summary(repo, args, config)?;
    
    println!("{} Fetch completed successfully", "✅".green().bold());
    
    Ok(())
}

/// Fetch from all configured remotes
async fn fetch_all_remotes(repo: &Repository, config: &Config) -> Result<()> {
    let remotes = repo.remotes()?;
    
    if remotes.is_empty() {
        println!("{} No remotes configured", "ℹ️".blue());
        return Ok(());
    }
    
    println!("{} Fetching from {} remote{}", 
            "🌐".blue(), 
            remotes.len(), 
            if remotes.len() == 1 { "" } else { "s" });
    
    for remote_name in remotes.iter() {
        if let Some(name) = remote_name {
            println!("\n{} Fetching from {}", "📡".blue(), name.cyan());
            
            match fetch_remote_by_name(repo, name, config).await {
                Ok(_) => println!("  {} {}", "✅".green(), "Success".green()),
                Err(e) => {
                    println!("  {} Failed: {}", "❌".red(), e);
                    // Continue with other remotes even if one fails
                }
            }
        }
    }
    
    Ok(())
}

/// Fetch from a single remote
async fn fetch_single_remote(
    repo: &Repository,
    remote_name: &str,
    args: &FetchArgs,
    config: &Config,
) -> Result<()> {
    println!("{} Fetching from {}", "📡".blue(), remote_name.cyan());
    
    // Check if remote exists
    if repo.find_remote(remote_name).is_err() {
        return Err(RgitError::RemoteNotFound(remote_name.to_string()).into());
    }
    
    // Show remote URL
    if let Ok(remote) = repo.find_remote(remote_name) {
        if let Some(url) = remote.url() {
            println!("{} URL: {}", "🌐".blue(), url.dimmed());
        }
    }
    
    // Perform fetch with specific options
    fetch_remote_with_options(repo, remote_name, args, config).await?;
    
    Ok(())
}

/// Fetch from a remote by name
async fn fetch_remote_by_name(repo: &Repository, remote_name: &str, config: &Config) -> Result<()> {
    let args = FetchArgs {
        remote: Some(remote_name.to_string()),
        all: false,
        prune: false,
        tags: false,
        depth: None,
        unshallow: false,
    };
    
    fetch_remote_with_options(repo, remote_name, &args, config).await
}

/// Fetch from remote with specific options
async fn fetch_remote_with_options(
    repo: &Repository,
    remote_name: &str,
    args: &FetchArgs,
    config: &Config,
) -> Result<()> {
    let mut remote = repo.find_remote(remote_name)
        .map_err(|_| RgitError::RemoteNotFound(remote_name.to_string()))?;
    
    // Set up callbacks
    let mut callbacks = RemoteCallbacks::new();
    
    // Progress callback
    if config.ui.interactive {
        callbacks.progress(|progress| {
            if let Some(msg) = std::str::from_utf8(progress).ok() {
                let msg = msg.trim();
                if !msg.is_empty() {
                    print!("\r{} {}", "📦".blue(), msg);
                    io::stdout().flush().unwrap();
                }
            }
            true
        });
    }
    
    // Authentication callback
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
    });
    
    // Update tips callback
    callbacks.update_tips(|refname, old_oid, new_oid| {
        if config.ui.interactive {
            let old_short = old_oid.to_string()[..8].to_string();
            let new_short = new_oid.to_string()[..8].to_string();
            println!("\r{} {}: {} -> {}", 
                    "🔄".yellow(), 
                    refname.cyan(),
                    if old_oid.is_zero() { "new".green() } else { old_short.yellow() },
                    new_short.green());
        }
        true
    });
    
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);
    
    // Configure fetch options
    if let Some(depth) = args.depth {
        fetch_options.depth(depth as i32);
    }
    
    if args.unshallow {
        fetch_options.depth(i32::MAX); // Effectively unshallow
    }
    
    // Determine what to fetch
    let refspecs = if args.tags {
        vec!["refs/tags/*:refs/tags/*"]
    } else {
        // Use default refspecs from remote configuration
        let refspecs = remote.fetch_refspecs()?;
        refspecs.iter().collect::<Option<Vec<&str>>>()
            .ok_or_else(|| RgitError::InvalidRefspec("Failed to get refspecs".to_string()))?
    };
    
    // Perform the fetch
    remote.fetch(&refspecs, Some(&mut fetch_options), None)
        .map_err(|e| RgitError::FetchFailed(e.message().to_string()))?;
    
    // Handle pruning
    if args.prune {
        prune_remote_refs(repo, remote_name, config)?;
    }
    
    if config.ui.interactive {
        println!(); // New line after progress
    }
    
    Ok(())
}

/// Prune remote tracking branches that no longer exist on remote
fn prune_remote_refs(repo: &Repository, remote_name: &str, config: &Config) -> Result<()> {
    println!("{} Pruning remote tracking branches", "✂️".yellow());
    
    let remote_prefix = format!("refs/remotes/{}/", remote_name);
    let mut pruned_count = 0;
    
    // Get list of remote refs
    let remote = repo.find_remote(remote_name)?;
    let mut connection = remote.connect(git2::Direction::Fetch)?;
    let remote_refs = connection.list()?;
    
    // Build set of existing remote branch names
    let mut remote_branches = std::collections::HashSet::new();
    for remote_ref in remote_refs {
        if let Some(name) = remote_ref.name().strip_prefix("refs/heads/") {
            remote_branches.insert(name.to_string());
        }
    }
    
    // Check local remote tracking branches
    let references = repo.references_glob(&format!("{}*", remote_prefix))?;
    
    for reference in references {
        let reference = reference?;
        if let Some(name) = reference.name() {
            if let Some(branch_name) = name.strip_prefix(&remote_prefix) {
                if !remote_branches.contains(branch_name) {
                    // This remote tracking branch no longer exists on remote
                    if config.ui.interactive {
                        println!("  {} Pruning {}", "✂️".red(), name.red());
                    }
                    reference.delete()?;
                    pruned_count += 1;
                }
            }
        }
    }
    
    if pruned_count > 0 {
        println!("{} Pruned {} remote tracking branch{}", 
                "✅".green(), 
                pruned_count, 
                if pruned_count == 1 { "" } else { "es" });
    } else if config.ui.interactive {
        println!("  {} No branches to prune", "ℹ️".blue());
    }
    
    Ok(())
}

/// Show fetch summary
fn show_fetch_summary(repo: &Repository, args: &FetchArgs, config: &Config) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    println!("\n{} Fetch Summary:", "📊".blue().bold());
    
    // Show what was fetched
    if args.all {
        let remotes = repo.remotes()?;
        println!("  {} Fetched from {} remote{}", 
                "📡".blue(), 
                remotes.len(),
                if remotes.len() == 1 { "" } else { "s" });
    } else {
        let remote_name = args.remote.as_deref().unwrap_or("origin");
        println!("  {} Fetched from {}", "📡".blue(), remote_name.cyan());
    }
    
    // Show remote tracking branch status
    show_tracking_status(repo)?;
    
    // Show next steps
    println!("\n{} Next steps:", "💡".blue());
    println!("  • {} - Check for new commits", "rgit log --oneline".cyan());
    println!("  • {} - View all branches", "rgit branch -a".cyan());
    println!("  • {} - Merge or rebase changes", "rgit pull".cyan());
    println!("  • {} - Check status", "rgit status".cyan());
    
    Ok(())
}

/// Show status of remote tracking branches
fn show_tracking_status(repo: &Repository) -> Result<()> {
    let head = match repo.head() {
        Ok(head) => head,
        Err(_) => return Ok(()), // No HEAD, probably empty repo
    };
    
    let current_branch = head.shorthand().unwrap_or("HEAD");
    
    // Check if current branch has upstream
    if let Ok(upstream) = head.upstream() {
        let upstream_name = upstream.shorthand().unwrap_or("unknown");
        
        // Compare HEAD with upstream
        let head_oid = head.target().unwrap();
        let upstream_oid = upstream.target().unwrap();
        
        if head_oid == upstream_oid {
            println!("  {} {} is up to date with {}", 
                    "✅".green(), 
                    current_branch.cyan(), 
                    upstream_name.yellow());
        } else {
            // Calculate ahead/behind counts
            let (ahead, behind) = repo.graph_ahead_behind(head_oid, upstream_oid)?;
            
            if ahead > 0 && behind > 0 {
                println!("  {} {} is {} ahead, {} behind {}", 
                        "↕️".yellow(), 
                        current_branch.cyan(),
                        ahead.to_string().green(),
                        behind.to_string().red(),
                        upstream_name.yellow());
            } else if ahead > 0 {
                println!("  {} {} is {} ahead of {}", 
                        "⬆️".green(), 
                        current_branch.cyan(),
                        ahead.to_string().green(),
                        upstream_name.yellow());
            } else if behind > 0 {
                println!("  {} {} is {} behind {}", 
                        "⬇️".red(), 
                        current_branch.cyan(),
                        behind.to_string().red(),
                        upstream_name.yellow());
            }
        }
    } else {
        println!("  {} {} has no upstream branch", 
                "⚠️".yellow(), 
                current_branch.cyan());
    }
    
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
    fn test_show_tracking_status() {
        let (_temp_dir, repo) = create_test_repo();
        
        // Should handle empty repo gracefully
        let result = show_tracking_status(&repo);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_fetch_all_remotes_empty() {
        let (_temp_dir, repo) = create_test_repo();
        let config = Config::minimal();
        
        // Should handle repo with no remotes
        let result = fetch_all_remotes(&repo, &config).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_prune_remote_refs() {
        let (_temp_dir, repo) = create_test_repo();
        let config = Config::minimal();
        
        // Should handle repo with no remotes gracefully
        let result = prune_remote_refs(&repo, "origin", &config);
        assert!(result.is_err()); // Expected since no remote exists
    }
}