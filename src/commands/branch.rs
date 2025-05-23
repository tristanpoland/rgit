use anyhow::Result;
use colored::*;
use git2::{Branch, BranchType, Repository};

use crate::cli::BranchArgs;
use crate::config::Config;
use crate::core::RgitCore;
use crate::error::RgitError;
use crate::interactive::InteractivePrompt;

/// Execute the branch command
pub async fn execute(args: &BranchArgs, rgit: &RgitCore, config: &Config) -> Result<()> {
    let repo = &rgit.repo;

    if args.delete.is_some() {
        delete_branch(repo, args, config).await
    } else if args.rename.is_some() {
        move_branch(repo, args, config).await
    } else if args.copy.is_some() {
        copy_branch(repo, args, config).await
    } else if let Some(branch_name) = &args.name {
        create_branch(repo, branch_name, args, config).await
    } else {
        list_branches(repo, args, config).await
    }
}

/// List branches
async fn list_branches(repo: &Repository, args: &BranchArgs, config: &Config) -> Result<()> {
    println!("{} Repository branches:", "🌿".green().bold());

    let branch_type = if args.list {
        None // Show both local and remote when --list is true
    } else if args.remotes {
        Some(BranchType::Remote)
    } else {
        Some(BranchType::Local)
    };

    let current_branch = get_current_branch(repo)?;

    // Collect and sort branches
    let mut branches = Vec::new();

    if branch_type.is_none() || branch_type == Some(BranchType::Local) {
        collect_branches(repo, BranchType::Local, &mut branches, &current_branch)?;
    }

    if branch_type.is_none() || branch_type == Some(BranchType::Remote) {
        collect_branches(repo, BranchType::Remote, &mut branches, &current_branch)?;
    }

    // Sort branches by name
    branches.sort_by(|a, b| a.name.cmp(&b.name));

    // Display branches
    if branches.is_empty() {
        println!("  {} No branches found", "ℹ️".blue());
        return Ok(());
    }

    for branch_info in branches.clone() {
        display_branch_info(&branch_info, config)?;
    }

    // Show summary
    if config.ui.interactive {
        let local_count = branches.iter().filter(|b| !b.is_remote).count();
        let remote_count = branches.iter().filter(|b| b.is_remote).count();

        println!();
        if local_count > 0 {
            println!(
                "{} {} local branch{}",
                "📍".blue(),
                local_count,
                if local_count == 1 { "" } else { "es" }
            );
        }
        if remote_count > 0 {
            println!(
                "{} {} remote branch{}",
                "🌐".blue(),
                remote_count,
                if remote_count == 1 { "" } else { "es" }
            );
        }

        // Show next steps
        println!("\n{} Commands:", "💡".blue());
        println!("  • {} - Create new branch", "rgit branch <name>".cyan());
        println!("  • {} - Switch to branch", "rgit checkout <name>".cyan());
        println!("  • {} - Delete branch", "rgit branch -d <name>".cyan());
    }

    Ok(())
}

/// Branch information for display
#[derive(Debug, Clone)]
struct BranchInfo {
    name: String,
    is_current: bool,
    is_remote: bool,
    commit_id: String,
    commit_message: String,
    author: String,
    ahead_behind: Option<(usize, usize)>,
    upstream: Option<String>,
}

/// Collect branches of a specific type
fn collect_branches(
    repo: &Repository,
    branch_type: BranchType,
    branches: &mut Vec<BranchInfo>,
    current_branch: &Option<String>,
) -> Result<()> {
    let branch_iter = repo.branches(Some(branch_type))?;

    for branch_result in branch_iter {
        let (branch, branch_type) = branch_result?;
        let is_remote = branch_type == BranchType::Remote;

        if let Some(name) = branch.name()? {
            let is_current = match current_branch {
                Some(current) => name == current && !is_remote,
                None => false,
            };

            let commit = branch.get().peel_to_commit()?;
            let commit_message = commit.summary().unwrap_or("No commit message").to_string();
            let author = commit.author();
            let author_name = author.name().unwrap_or("Unknown").to_string();
            // Calculate ahead/behind for local branches with upstream
            let ahead_behind = if !is_remote {
                calculate_ahead_behind(repo, &branch)?
            } else {
                None
            };

            // Get upstream info for local branches
            let upstream = if !is_remote {
                get_upstream_branch(repo, name)?
            } else {
                None
            };
            branches.push(BranchInfo {
                name: name.to_string(),
                is_current,
                is_remote,
                commit_id: commit.id().to_string()[..8].to_string(),
                commit_message,
                author: author_name,
                ahead_behind,
                upstream,
            });
        }
    }

    Ok(())
}

/// Display information for a single branch
fn display_branch_info(branch: &BranchInfo, config: &Config) -> Result<()> {
    let prefix = if branch.is_current {
        "*".green().bold()
    } else {
        " ".normal()
    };

    let branch_color = if branch.is_current {
        branch.name.green().bold()
    } else if branch.is_remote {
        branch.name.red()
    } else {
        branch.name.cyan()
    };

    print!("{} {}", prefix, branch_color);

    // Show upstream tracking
    if let Some(upstream) = &branch.upstream {
        print!(" -> {}", upstream.yellow());
    }

    // Show ahead/behind status
    if let Some((ahead, behind)) = branch.ahead_behind {
        if ahead > 0 || behind > 0 {
            print!(" [");
            if ahead > 0 {
                print!("ahead {}", ahead.to_string().green());
            }
            if ahead > 0 && behind > 0 {
                print!(", ");
            }
            if behind > 0 {
                print!("behind {}", behind.to_string().red());
            }
            print!("]");
        }
    }

    if config.ui.interactive {
        println!();
        println!(
            "    {} {} by {}",
            branch.commit_id.yellow(),
            branch.commit_message.white(),
            branch.author.dimmed()
        );
    } else {
        println!(
            " {} {}",
            branch.commit_id.yellow(),
            branch.commit_message.white()
        );
    }

    Ok(())
}

/// Create a new branch
async fn create_branch(
    repo: &Repository,
    branch_name: &str,
    args: &BranchArgs,
    config: &Config,
) -> Result<()> {
    println!(
        "{} Creating branch '{}'",
        "🌱".green().bold(),
        branch_name.cyan()
    );

    // Validate branch name
    if !is_valid_branch_name(branch_name) {
        return Err(RgitError::InvalidBranchName(branch_name.to_string()).into());
    }

    // Check if branch already exists
    if repo.find_branch(branch_name, BranchType::Local).is_ok() {
        return Err(RgitError::BranchAlreadyExists(branch_name.to_string()).into());
    }
    
    // Determine starting point (use HEAD for now)
    let start_point = repo.head()?.peel_to_commit()?;

    // Create the branch
    let branch = repo.branch(branch_name, &start_point, false)?;

    println!(
        "{} Branch '{}' created successfully",
        "✅".green(),
        branch_name.cyan()
    );

    // Ask if user wants to switch to the new branch
    if config.is_interactive() {
        let switch = InteractivePrompt::new()
            .with_message(&format!("Switch to branch '{}'?", branch_name))
            .confirm()?;
        if switch {
            checkout_branch(repo, branch_name)?;
            println!(
                "{} Switched to branch '{}'",
                "🔄".green(),
                branch_name.cyan()
            );
        }
    } else {
        println!(
            "{} Use 'rgit checkout {}' to switch to the new branch",
            "ℹ️".blue(),
            branch_name.cyan()
        );
    }
    Ok(())
}

/// Delete a branch
async fn delete_branch(repo: &Repository, args: &BranchArgs, config: &Config) -> Result<()> {
    let branch_name = args.delete.as_ref().unwrap();

    println!(
        "{} Deleting branch '{}'",
        "🗑️".red().bold(),
        branch_name.red()
    );

    // Find the branch
    let branch = repo
        .find_branch(branch_name, BranchType::Local)
        .map_err(|_| RgitError::BranchNotFound(branch_name.to_string()))?;

    // Check if branch is merged (unless force delete)
    if args.force_delete.is_none() {
        if !is_branch_merged(repo, &branch)? {
            if config.is_interactive() {
                println!(
                    "{} Branch '{}' is not fully merged",
                    "⚠️".yellow(),
                    branch_name.yellow()
                );

                let force_delete = InteractivePrompt::new()
                    .with_message("Delete anyway? (This will lose commits)")
                    .confirm()?;

                if !force_delete {
                    println!("{} Branch deletion cancelled", "❌".red());
                    return Ok(());
                }
            } else {
                return Err(RgitError::OperationFailed(format!(
                    "Branch '{}' is not fully merged",
                    branch_name
                ))
                .into());
            }
        }
    }

    // Delete the branch
    let mut branch = branch;
    branch.delete()?;

    println!(
        "{} Branch '{}' deleted successfully",
        "✅".green(),
        branch_name.cyan()
    );

    Ok(())
}

async fn move_branch(repo: &Repository, args: &BranchArgs, _config: &Config) -> Result<()> {
    let new_name = args.rename.as_ref().unwrap();
    let current_branch = get_current_branch(repo)?;
    let old_name = if let Some(name) = args.name.as_ref() {
        name.as_str()
    } else {
        current_branch
            .as_deref()
            .ok_or_else(|| RgitError::OperationFailed("Branch name required".to_string()))?
    };

    // Validate new branch name
    if !is_valid_branch_name(new_name) {
        return Err(RgitError::InvalidBranchName(new_name.to_string()).into());
    }
    // Check if target name already exists
    if repo.find_branch(new_name, BranchType::Local).is_ok() {
        return Err(RgitError::BranchAlreadyExists(new_name.to_string()).into());
    }
    // Find and rename the branch
    let mut branch = repo
        .find_branch(old_name, BranchType::Local)
        .map_err(|_| RgitError::BranchNotFound(old_name.to_string()))?;
    branch.rename(new_name, false)?;

    println!(
        "{} Branch '{}' renamed to '{}'",
        "✅".green(),
        old_name.cyan(),
        new_name.cyan()
    );

    Ok(())
}

/// Copy a branch
async fn copy_branch(repo: &Repository, args: &BranchArgs, _config: &Config) -> Result<()> {
    let new_name = args.move_to.as_ref().unwrap();
    let current_branch = get_current_branch(repo)?;
    let current_ref: Option<&String> = current_branch.as_ref();
    let source_name = if let Some(name) = args.name.as_ref() {
        name.as_str()
    } else {
        current_ref
            .map(|s| s.as_str())
            .ok_or_else(|| RgitError::OperationFailed("Branch name required".to_string()))?
    };

    println!(
        "{} Copying branch '{}' to '{}'",
        "📋".blue().bold(),
        source_name.cyan(),
        new_name.cyan()
    );

    // Validate new branch name
    if !is_valid_branch_name(new_name) {
        return Err(RgitError::InvalidBranchName(new_name.to_string()).into());
    }

    // Check if target name already exists
    if repo.find_branch(new_name, BranchType::Local).is_ok() {
        return Err(RgitError::BranchAlreadyExists(new_name.to_string()).into());
    }

    // Find source branch and get its commit
    let source_branch = repo
        .find_branch(source_name, BranchType::Local)
        .map_err(|_| RgitError::BranchNotFound(source_name.to_string()))?;

    let commit = source_branch.get().peel_to_commit()?;

    // Create new branch at the same commit
    repo.branch(new_name, &commit, false)?;

    println!("{} Branch copied successfully", "✅".green());

    Ok(())
}

/// Helper functions

fn get_current_branch(repo: &Repository) -> Result<Option<String>> {
    match repo.head() {
        Ok(head) => Ok(head.shorthand().map(|s| s.to_string())),
        Err(_) => Ok(None),
    }
}

fn resolve_commit_reference<'a>(repo: &'a Repository, reference: &str) -> Result<git2::Commit<'a>> {
    let obj = repo.revparse_single(reference)?;
    Ok(obj.peel_to_commit()?)
}

fn is_valid_branch_name(name: &str) -> bool {
    // Basic validation - can be extended
    !name.is_empty()
        && !name.starts_with('-')
        && !name.contains("..")
        && !name.contains(' ')
        && !name.contains('\t')
        && !name.contains('\n')
        && name != "HEAD"
}

fn calculate_ahead_behind(repo: &Repository, branch: &Branch) -> Result<Option<(usize, usize)>> {
    if let Ok(upstream) = branch.upstream() {
        let local_oid = branch.get().target().unwrap();
        let upstream_oid = upstream.get().target().unwrap();

        let (ahead, behind) = repo.graph_ahead_behind(local_oid, upstream_oid)?;
        Ok(Some((ahead, behind)))
    } else {
        Ok(None)
    }
}

fn get_upstream_branch(repo: &Repository, branch_name: &str) -> Result<Option<String>> {
    let config = repo.config()?;
    let remote_key = format!("branch.{}.remote", branch_name);
    let merge_key = format!("branch.{}.merge", branch_name);

    if let (Ok(remote), Ok(merge_ref)) = (
        config.get_string(&remote_key),
        config.get_string(&merge_key),
    ) {
        if let Some(branch_name) = merge_ref.strip_prefix("refs/heads/") {
            Ok(Some(format!("{}/{}", remote, branch_name)))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn set_branch_upstream(repo: &Repository, branch: &Branch, upstream: &str) -> Result<()> {
    let branch_name = branch.name()?.unwrap();
    let mut config = repo.config()?;

    // Parse upstream (format: remote/branch)
    if let Some((remote, branch_ref)) = upstream.split_once('/') {
        let remote_key = format!("branch.{}.remote", branch_name);
        let merge_key = format!("branch.{}.merge", branch_name);
        let merge_ref = format!("refs/heads/{}", branch_ref);

        config.set_str(&remote_key, remote)?;
        config.set_str(&merge_key, &merge_ref)?;
    }

    Ok(())
}

fn checkout_branch(repo: &Repository, branch_name: &str) -> Result<()> {
    let branch = repo.find_branch(branch_name, BranchType::Local)?;
    let reference = branch.get();

    repo.set_head(reference.name().unwrap())?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().safe()))?;

    Ok(())
}

fn is_branch_merged(repo: &Repository, branch: &Branch) -> Result<bool> {
    let branch_commit = branch.get().peel_to_commit()?;
    let head_commit = repo.head()?.peel_to_commit()?;
    
    // Check if the branch commit is an ancestor of HEAD
    let is_ancestor = repo.graph_descendant_of(head_commit.id(), branch_commit.id())?;
    Ok(is_ancestor)
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
    fn test_is_valid_branch_name() {
        assert!(is_valid_branch_name("feature/new-feature"));
        assert!(is_valid_branch_name("main"));
        assert!(is_valid_branch_name("develop"));

        assert!(!is_valid_branch_name(""));
        assert!(!is_valid_branch_name("-invalid"));
        assert!(!is_valid_branch_name("branch..with..dots"));
        assert!(!is_valid_branch_name("HEAD"));
    }

    #[tokio::test]
    async fn test_create_branch() {
        let (_temp_dir, repo) = create_test_repo();
        let config = Config::minimal();

        // Create initial commit first
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
        )
        .unwrap();

        let args = BranchArgs {
            name: Some("test-branch".to_string()),
            delete: None,
            force_delete: None,
            list: false,
            remotes: false,
            rename: None,
            move_to: None,
            copy: None,
            merged: false,
            no_merged: false,
        };

        let result = create_branch(&repo, "test-branch", &args, &config).await;
        assert!(result.is_ok());

        // Verify branch was created
        assert!(repo.find_branch("test-branch", BranchType::Local).is_ok());
    }
}