use anyhow::Result;
use colored::*;
use git2::{PushOptions, RemoteCallbacks, Repository};
use std::io::{self, Write};

use crate::cli::PushArgs;
use crate::config::Config;
use crate::core::RgitCore;
use crate::error::RgitError;
use crate::interactive::InteractivePrompt;

/// Execute the push command
pub async fn execute(args: &PushArgs, rgit: &RgitCore, config: &Config) -> Result<()> {
    println!("{} Pushing changes...", "🚀".blue().bold());
    
    // Check if we have any commits to push
    let status = rgit.status()?;
    if !status.staged.is_empty() {
        println!("{} You have staged changes that haven't been committed:", "⚠️".yellow());
        println!("  Run {} first", "rgit commit".cyan());
        
        if config.is_interactive() {
            let continue_anyway = InteractivePrompt::new()
                .with_message("Continue with push anyway?")
                .confirm()?;
            
            if !continue_anyway {
                return Ok(());
            }
        }
    }
    
    let repo = &rgit.repo;
    
    // Determine what to push
    let (remote_name, branch_specs) = determine_push_target(repo, args, config)?;
    
    // Get the remote
    let mut remote = repo.find_remote(&remote_name)
        .map_err(|_| RgitError::RemoteNotFound(remote_name.clone()))?;
    
    // Show push details
    println!("{} Remote: {}", "📡".blue(), remote_name.cyan());
    if let Some(url) = remote.url() {
        println!("{} URL: {}", "🌐".blue(), url.dimmed());
    }
    
    for spec in &branch_specs {
        println!("{} Pushing: {}", "🌿".green(), spec.yellow());
    }
    
    // Check if we need to set upstream
    let current_branch = get_current_branch(repo)?;
    let needs_upstream = should_set_upstream(repo, &current_branch, &remote_name)?;
    
    if needs_upstream && !args.set_upstream {
        if config.is_interactive() {
            let set_upstream = InteractivePrompt::new()
                .with_message(&format!("Set '{}' as upstream for '{}'?", remote_name, current_branch))
                .confirm()?;
            
            if set_upstream {
                println!("{} Setting upstream branch", "🔗".blue());
            }
        }
    }
    
    // Perform the push
    perform_push(&mut remote, &branch_specs, args, config).await?;
    
    println!("{} Successfully pushed to {}", "✅".green().bold(), remote_name.cyan());
    
    // Show post-push information
    show_push_summary(repo, &remote_name, &current_branch, config)?;
    
    Ok(())
}

/// Determine what remote and branches to push
fn determine_push_target(
    repo: &Repository,
    args: &PushArgs,
    _config: &Config,
) -> Result<(String, Vec<String>)> {
    let remote_name = args.remote.clone()
        .or_else(|| get_default_remote(repo))
        .unwrap_or_else(|| "origin".to_string());
    
    let branch_specs = if let Some(ref branch) = args.branch {
        // Push specific branch
        vec![format!("refs/heads/{}:refs/heads/{}", branch, branch)]
    } else {
        // Push current branch
        let current_branch = get_current_branch(repo)?;
        vec![format!("refs/heads/{}:refs/heads/{}", current_branch, current_branch)]
    };
    
    Ok((remote_name, branch_specs))
}

/// Get the current branch name
fn get_current_branch(repo: &Repository) -> Result<String> {
    let head = repo.head()?;
    if let Some(name) = head.shorthand() {
        Ok(name.to_string())
    } else {
        Err(anyhow::anyhow!("Not on a branch").into())
    }
}

/// Get the default remote for the current branch
fn get_default_remote(repo: &Repository) -> Option<String> {
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

/// Check if we should set upstream for the current branch
fn should_set_upstream(repo: &Repository, branch: &str, remote: &str) -> Result<bool> {
    let config = repo.config()?;
    let upstream_key = format!("branch.{}.remote", branch);
    
    match config.get_string(&upstream_key) {
        Ok(existing_remote) => Ok(existing_remote != remote),
        Err(_) => Ok(true), // No upstream set
    }
}

/// Get all local branches for pushing
fn get_all_local_branches(repo: &Repository) -> Result<Vec<String>> {
    let mut branches = Vec::new();
    let branch_iter = repo.branches(Some(git2::BranchType::Local))?;
    
    for branch_result in branch_iter {
        let (branch, _) = branch_result?;
        if let Some(name) = branch.name()? {
            branches.push(format!("refs/heads/{}:refs/heads/{}", name, name));
        }
    }
    
    Ok(branches)
}

/// Get all tags for pushing
fn get_all_tags(repo: &Repository) -> Result<Vec<String>> {
    let mut tags = Vec::new();
    
    repo.tag_foreach(|_oid, name| {
        if let Some(tag_name) = std::str::from_utf8(name).ok() {
            if tag_name.starts_with("refs/tags/") {
                tags.push(format!("{}:{}", tag_name, tag_name));
            }
        }
        true
    })?;
    
    Ok(tags)
}

/// Perform the actual push operation
async fn perform_push(
    remote: &mut git2::Remote<'_>,
    refspecs: &[String],
    args: &PushArgs,
    config: &Config,
) -> Result<()> {
    let mut callbacks = RemoteCallbacks::new();
    
    // Set up progress callback
    if config.ui.interactive {
        callbacks.pack_progress(|_stage, current, total| {
            if total > 0 {
                let percentage = (current * 100) / total;
                print!("\r{} Progress: {}% ({}/{})", "📤".blue(), percentage, current, total);
                io::stdout().flush().unwrap();
            }
            ()
        });
    }
    
    // Set up push progress callback
    callbacks.push_update_reference(|refname, status| {
        if let Some(msg) = status {
            println!("\r{} Failed to push {}: {}", "❌".red(), refname, msg);
            return Err(git2::Error::from_str("Push rejected"));
        }
        
        if config.ui.interactive {
            println!("\r{} Updated {}", "✅".green(), refname);
        }
        
        Ok(())
    });
    
    // Set up authentication callback if needed
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
    });
    
    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);
    
    // Convert refspecs to the format git2 expects
    let refspec_refs: Vec<&str> = refspecs.iter().map(|s| s.as_str()).collect();
    
    // Perform the push
    match remote.push(&refspec_refs, Some(&mut push_options)) {
        Ok(_) => {
            if config.ui.interactive {
                println!(); // New line after progress
            }
        }
        Err(e) => {
            if e.message().contains("non-fast-forward") {
                println!("\n{} Push rejected (non-fast-forward)", "❌".red().bold());
                println!("{} The remote contains work that you do not have locally.", "💡".blue());
                
                if args.force {
                    println!("{} Force pushing...", "⚠️".yellow().bold());
                    force_push(remote, refspecs)?;
                } else {
                    println!("Suggestions:");
                    println!("  • {} - Fetch and merge remote changes", "rgit pull".cyan());
                    println!("  • {} - Force push (destructive!)", "rgit push --force".red());
                    return Err(anyhow::anyhow!("Push rejected: {}", e.message()).into());
                }
            } else {
                return Err(anyhow::anyhow!("Push failed: {}", e.message()).into());
            }
        }
    }
    
    Ok(())
}

/// Force push (dangerous operation)
fn force_push(remote: &mut git2::Remote, refspecs: &[String]) -> Result<()> {
    let mut callbacks = RemoteCallbacks::new();
    
    // Set up authentication
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
    });
    
    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);
    
    // Force push by adding + prefix to refspecs
    let force_refspecs: Vec<String> = refspecs.iter()
        .map(|spec| format!("+{}", spec))
        .collect();
    
    let refspec_refs: Vec<&str> = force_refspecs.iter().map(|s| s.as_str()).collect();
    
    remote.push(&refspec_refs, Some(&mut push_options))
        .map_err(|e| anyhow::anyhow!("Force push failed: {}", e.message()))?;
    
    Ok(())
}

/// Show summary after successful push
fn show_push_summary(
    repo: &Repository,
    remote_name: &str,
    branch_name: &str,
    config: &Config,
) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    println!("\n{} Push Summary:", "📊".blue().bold());
    
    // Show what was pushed
    if let Ok(head) = repo.head() {
        if let Ok(commit) = head.peel_to_commit() {
            println!("  {} Latest commit: {}", "📝".yellow(), 
                    commit.id().to_string()[..8].yellow());
            
            if let Some(summary) = commit.summary() {
                println!("    {} {}", "💬".blue(), summary.white());
            }
        }
    }
    
    // Show remote tracking information
    println!("  {} Remote branch: {}/{}", "🌿".green(), remote_name.cyan(), branch_name.cyan());
    
    // Show next steps
    println!("\n{} Next steps:", "💡".blue());
    println!("  • {} - View remote repository", "Open in browser".cyan());
    println!("  • {} - Check for new activity", "rgit fetch".cyan());
    println!("  • {} - View commit history", "rgit log".cyan());
    
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
    fn test_get_current_branch() {
        let (_temp_dir, repo) = create_test_repo();
        
        // Create and checkout a branch
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        
        let _commit_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        ).unwrap();
        
        // Should be on master/main branch
        let branch = get_current_branch(&repo).unwrap();
        assert!(branch == "master" || branch == "main");
    }

    #[test]
    fn test_should_set_upstream() {
        let (_temp_dir, repo) = create_test_repo();
        
        // Initially should need upstream
        let needs_upstream = should_set_upstream(&repo, "main", "origin");
        assert!(needs_upstream.is_ok());
    }

    #[test]
    fn test_get_all_tags() {
        let (_temp_dir, repo) = create_test_repo();
        
        // Should return empty vector for new repo
        let tags = get_all_tags(&repo).unwrap();
        assert!(tags.is_empty());
    }
}