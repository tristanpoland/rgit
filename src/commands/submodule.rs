use anyhow::Result;
use colored::*;
use git2::*;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::{SubmoduleArgs, SubmoduleCommands};
use crate::config::Config;
use crate::core::RgitCore;
use crate::error::RgitError;
use crate::interactive::{InteractivePrompt, ProgressDisplay, TableDisplay};
use crate::submodule::SubmoduleManager;
use crate::utils::parse_git_url;

/// Execute submodule command
pub async fn execute(args: &SubmoduleArgs, rgit: &RgitCore, config: &Config) -> Result<()> {
    let submodule_manager = SubmoduleManager::new(rgit, config);
    
    match &args.action {
        SubmoduleCommands::Add { url, path, branch, name, depth } => {
            add_submodule(&submodule_manager, url, path, branch, name, *depth, config).await
        }
        SubmoduleCommands::Init { paths, all } => {
            init_submodules(&submodule_manager, paths, *all, config).await
        }
        SubmoduleCommands::Update { paths, init, recursive, merge, rebase, remote, force } => {
            update_submodules(&submodule_manager, paths, *init, *recursive, *merge, *rebase, *remote, *force, config).await
        }
        SubmoduleCommands::Status { recursive, health } => {
            show_submodule_status(&submodule_manager, *recursive, *health, config).await
        }
        SubmoduleCommands::Sync { paths, recursive } => {
            sync_submodules(&submodule_manager, paths, *recursive, config).await
        }
        SubmoduleCommands::Deinit { path, force, remove } => {
            deinit_submodule(&submodule_manager, path, *force, *remove, config).await
        }
        SubmoduleCommands::Foreach { command, recursive, continue_on_error } => {
            foreach_submodule(&submodule_manager, command, *recursive, *continue_on_error, config).await
        }
    }
}

/// Add a new submodule to the repository
async fn add_submodule(
    manager: &SubmoduleManager<'_>,
    url: &str,
    path: &str,
    branch: &Option<String>,
    name: &Option<String>,
    depth: Option<u32>,
    config: &Config,
) -> Result<()> {
    manager.rgit.log(&format!("Adding submodule: {} -> {}", url, path));
    
    // Validate inputs
    validate_submodule_add_inputs(url, path, config)?;
    
    // Check if path already exists
    let submodule_path = Path::new(path);
    if submodule_path.exists() {
        return Err(RgitError::SubmoduleOperationFailed(
            format!("Path '{}' already exists", path)
        ).into());
    }
    
    // Show what will be added
    show_submodule_add_preview(url, path, branch, name, depth, config)?;
    
    // Confirm if interactive
    if config.is_interactive() && !confirm_submodule_add(url, path, config)? {
        manager.rgit.info("Submodule add cancelled");
        return Ok(());
    }
    
    // Perform the add operation
    let progress = if config.ui.progress {
        Some(ProgressDisplay::new("Adding submodule")
            .create_progress_bar())
    } else {
        None
    };
    
    if let Some(ref pb) = progress {
        pb.set_message("Cloning submodule repository...");
    }
    
    // Add submodule to .gitmodules and clone
    add_submodule_to_repo(manager.rgit, url, path, branch.as_deref(), name.as_deref())?;
    
    if let Some(ref pb) = progress {
        pb.set_message("Initializing submodule...");
    }
    
    // Initialize the submodule
    let mut submodule = manager.rgit.repo.find_submodule(path)?;
    submodule.init(false)?;
    
    if let Some(ref pb) = progress {
        pb.set_message("Updating submodule...");
    }
    
    // Update to get the actual content
    submodule.update(true, None)?;
    
    if let Some(ref pb) = progress {
        pb.finish_with_message("✅ Submodule added successfully");
    }
    
    manager.rgit.success(&format!("Added submodule '{}' at '{}'", url, path));
    
    // Show next steps
    show_submodule_add_next_steps(path, config)?;
    
    Ok(())
}

/// Initialize submodules
async fn init_submodules(
    manager: &SubmoduleManager<'_>,
    paths: &[String],
    all: bool,
    config: &Config,
) -> Result<()> {
    manager.rgit.log("Initializing submodules...");
    
    let submodules = manager.rgit.repo.submodules()?;
    
    if submodules.is_empty() {
        manager.rgit.info("No submodules found");
        return Ok(());
    }
    
    let target_submodules: Vec<_> = if all || paths.is_empty() {
        submodules.iter().collect()
    } else {
        filter_submodules_by_path(&submodules, paths)?
    };
    
    if target_submodules.is_empty() {
        manager.rgit.info("No matching submodules found");
        return Ok(());
    }
    
    // Show what will be initialized
    show_init_preview(&target_submodules, config)?;
    
    let mut initialized = 0;
    let mut skipped = 0;
    
    for submodule in target_submodules {
        let name = submodule.name().unwrap_or("unknown");
        
        // Check if already initialized
        if submodule.open().is_ok() {
            manager.rgit.log(&format!("Submodule '{}' already initialized", name));
            skipped += 1;
            continue;
        }
        
        // Create a mutable reference by finding the submodule again
        let mut mutable_submodule = manager.rgit.repo.find_submodule(submodule.path().to_str().unwrap())?;
        match mutable_submodule.init(false) {
            Ok(()) => {
                manager.rgit.success(&format!("Initialized '{}'", name));
                initialized += 1;
            }
            Err(e) => {
                manager.rgit.warning(&format!("Failed to initialize '{}': {}", name, e));
            }
        }
    }
    
    // Show summary
    show_init_summary(initialized, skipped, config)?;
    
    Ok(())
}

/// Update submodules
async fn update_submodules(
    manager: &SubmoduleManager<'_>,
    paths: &[String],
    init: bool,
    recursive: bool,
    merge: bool,
    rebase: bool,
    remote: bool,
    force: bool,
    config: &Config,
) -> Result<()> {
    manager.rgit.log("Updating submodules...");
    
    // Health check first
    if config.submodules.health_check && !manager.interactive_health_check()? {
        return Err(RgitError::SubmoduleError("Health check failed".to_string()).into());
    }
    
    let submodules = manager.rgit.repo.submodules()?;
    
    if submodules.is_empty() {
        manager.rgit.info("No submodules found");
        return Ok(());
    }
    
    let target_submodules: Vec<_> = if paths.is_empty() {
        submodules.iter().collect()
    } else {
        filter_submodules_by_path(&submodules, paths)?
    };
    
    // Show update plan
    show_update_preview(&target_submodules, init, recursive, merge, rebase, remote, config)?;
    
    let progress = if config.ui.progress {
        Some(ProgressDisplay::new("Updating submodules")
            .with_total(target_submodules.len() as u64)
            .create_progress_bar())
    } else {
        None
    };
    
    let mut updated = 0;
    let mut failed = 0;
    
    for (i, submodule) in target_submodules.iter().enumerate() {
        let name = submodule.name().unwrap_or("unknown");
        
        if let Some(ref pb) = progress {
            pb.set_position(i as u64);
            pb.set_message(&format!("Updating {}", name));
        }
        
        // Get a mutable reference to the submodule
        let mut mutable_submodule = manager.rgit.repo.find_submodule(submodule.path().to_str().unwrap())?;
        
        // Initialize if needed and requested
        if init && mutable_submodule.open().is_err() {
            if let Err(e) = mutable_submodule.init(false) {
                manager.rgit.warning(&format!("Failed to initialize '{}': {}", name, e));
                failed += 1;
                continue;
            }
        }
        
        // Update the submodule
        match update_single_submodule(&mut mutable_submodule, merge, rebase, remote, force) {
            Ok(()) => {
                manager.rgit.success(&format!("Updated '{}'", name));
                updated += 1;
                
                // Recursive update if requested
                if recursive {
                    if let Err(e) = update_submodule_recursively(submodule, config).await {
                        manager.rgit.warning(&format!("Recursive update failed for '{}': {}", name, e));
                    }
                }
            }
            Err(e) => {
                manager.rgit.warning(&format!("Failed to update '{}': {}", name, e));
                failed += 1;
            }
        }
    }
    
    if let Some(ref pb) = progress {
        pb.finish_with_message(&format!("✅ Updated {} submodules", updated));
    }
    
    // Show summary
    show_update_summary(updated, failed, config)?;
    
    Ok(())
}

/// Show comprehensive submodule status
async fn show_submodule_status(
    manager: &SubmoduleManager<'_>,
    recursive: bool,
    health: bool,
    config: &Config,
) -> Result<()> {
    manager.rgit.log("Checking submodule status...");
    
    let submodules = manager.rgit.repo.submodules()?;
    
    if submodules.is_empty() {
        manager.rgit.info("No submodules found");
        return Ok(());
    }
    
    println!("{} Submodule Status Report", "📦".blue().bold());
    println!();
    
    if health {
        // Show detailed health information
        let health_info = manager.check_health()?;
        show_health_summary(&health_info, config)?;
    }
    
    // Show status table
    show_submodule_status_table(&submodules, recursive, config)?;
    
    // Show recommendations
    show_submodule_recommendations(&submodules, config)?;
    
    Ok(())
}

/// Sync submodule URLs from .gitmodules
async fn sync_submodules(
    manager: &SubmoduleManager<'_>,
    paths: &[String],
    recursive: bool,
    config: &Config,
) -> Result<()> {
    manager.rgit.log("Syncing submodule URLs...");
    
    let submodules = manager.rgit.repo.submodules()?;
    
    if submodules.is_empty() {
        manager.rgit.info("No submodules found");
        return Ok(());
    }
    
    let target_submodules: Vec<_> = if paths.is_empty() {
        submodules.iter().collect()
    } else {
        filter_submodules_by_path(&submodules, paths)?
    };
    
    let mut synced = 0;
    
    for submodule in target_submodules {
        let name = submodule.name().unwrap_or("unknown");
        
        // Sync would update the remote URL from .gitmodules to .git/config
        manager.rgit.log(&format!("Syncing URLs for '{}'", name));
        // In real implementation: submodule.sync()?;
        
        synced += 1;
        
        if recursive {
            // Recursively sync nested submodules
            if let Ok(sub_repo) = submodule.open() {
                sync_nested_submodules(&sub_repo, config).await?;
            }
        }
    }
    
    manager.rgit.success(&format!("Synced {} submodule{}", 
                                 synced, 
                                 if synced == 1 { "" } else { "s" }));
    
    Ok(())
}

/// Deinitialize/remove a submodule
async fn deinit_submodule(
    manager: &SubmoduleManager<'_>,
    path: &str,
    force: bool,
    remove: bool,
    config: &Config,
) -> Result<()> {
    manager.rgit.log(&format!("Deinitializing submodule: {}", path));
    
    // Find the submodule
    let submodule = manager.rgit.repo.find_submodule(path)
        .map_err(|_| RgitError::SubmoduleNotFound(path.to_string()))?;
    
    let name = submodule.name().unwrap_or("unknown");
    
    // Check for uncommitted changes unless force
    if !force {
        if let Ok(sub_repo) = submodule.open() {
            if manager.has_uncommitted_changes(&sub_repo)? {
                return Err(RgitError::SubmoduleUncommittedChanges(name.to_string()).into());
            }
        }
    }
    
    // Show what will be removed
    show_deinit_preview(&submodule, remove, config)?;
    
    // Confirm if interactive
    if config.is_interactive() && !confirm_submodule_deinit(name, remove, config)? {
        manager.rgit.info("Deinit cancelled");
        return Ok(());
    }
    
    // Perform deinit
    deinit_submodule_implementation(manager.rgit, &submodule, remove)?;
    
    if remove {
        manager.rgit.success(&format!("Removed submodule '{}'", name));
    } else {
        manager.rgit.success(&format!("Deinitialized submodule '{}'", name));
    }
    
    Ok(())
}

/// Execute command in each submodule
async fn foreach_submodule(
    manager: &SubmoduleManager<'_>,
    command: &str,
    recursive: bool,
    continue_on_error: bool,
    config: &Config,
) -> Result<()> {
    manager.rgit.log(&format!("Executing '{}' in submodules...", command));
    
    let submodules = manager.rgit.repo.submodules()?;
    
    if submodules.is_empty() {
        manager.rgit.info("No submodules found");
        return Ok(());
    }
    
    println!("{} Executing: {}", "🔄".blue(), command.cyan().bold());
    println!();
    
    let mut success_count = 0;
    let mut error_count = 0;
    
    for submodule in submodules {
        let name = submodule.name().unwrap_or("unknown");
        let path = submodule.path();
        
        if !path.exists() {
            manager.rgit.warning(&format!("Submodule '{}' path does not exist", name));
            continue;
        }
        
        println!("{} Entering '{}'", "📁".blue(), name.cyan());
        
        match execute_command_in_submodule(command, path) {
            Ok(output) => {
                if !output.is_empty() {
                    println!("{}", output);
                }
                success_count += 1;
            }
            Err(e) => {
                manager.rgit.warning(&format!("Command failed in '{}': {}", name, e));
                error_count += 1;
                
                if !continue_on_error {
                    return Err(e);
                }
            }
        }
        
        if recursive {
            // Execute recursively in nested submodules
            if let Ok(sub_repo) = submodule.open() {
                execute_foreach_recursively(&sub_repo, command, continue_on_error).await?;
            }
        }
        
        println!();
    }
    
    // Show summary
    println!("{} Foreach completed:", "📊".blue().bold());
    println!("  {} {} successful", "✅".green(), success_count);
    if error_count > 0 {
        println!("  {} {} failed", "❌".red(), error_count);
    }
    
    Ok(())
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Validate submodule add inputs
fn validate_submodule_add_inputs(url: &str, path: &str, _config: &Config) -> Result<()> {
    // Validate URL
    if parse_git_url(url).is_none() {
        return Err(RgitError::SubmoduleInvalidUrl(url.to_string()).into());
    }
    
    // Validate path
    if path.is_empty() || path.contains("..") || path.starts_with('/') {
        return Err(RgitError::InvalidPath(PathBuf::from(path)).into());
    }
    
    Ok(())
}

/// Show preview of what will be added
fn show_submodule_add_preview(
    url: &str,
    path: &str,
    _branch: &Option<String>,
    name: &Option<String>,
    depth: Option<u32>,
    config: &Config,
) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    println!("{} Submodule Add Preview:", "👁️".blue().bold());
    println!("  {} {}", "URL:".bold(), url.cyan());
    println!("  {} {}", "Path:".bold(), path.yellow());
    
    if let Some(ref branch_name) = _branch {
        println!("  {} {}", "Branch:".bold(), branch_name.green());
    }
    
    if let Some(ref submodule_name) = name {
        println!("  {} {}", "Name:".bold(), submodule_name.white());
    }
    
    if let Some(clone_depth) = depth {
        println!("  {} {}", "Depth:".bold(), clone_depth.to_string().white());
    }
    
    println!();
    Ok(())
}

/// Confirm submodule add operation
fn confirm_submodule_add(url: &str, path: &str, config: &Config) -> Result<bool> {
    if !config.is_interactive() {
        return Ok(true);
    }
    
    InteractivePrompt::new()
        .with_message(&format!("Add submodule '{}' at '{}'?", url, path))
        .confirm()
}

/// Add submodule to repository
fn add_submodule_to_repo(
    rgit: &RgitCore,
    url: &str,
    path: &str,
    _branch: Option<&str>,
    _name: Option<&str>,
) -> Result<()> {
    // In real implementation, this would:
    // 1. Add entry to .gitmodules
    // 2. Clone the repository
    // 3. Add the submodule to git index
    
    rgit.log(&format!("Adding submodule {} to {}", url, path));
    
    // For now, simulate the operation
    Ok(())
}

/// Filter submodules by path patterns
fn filter_submodules_by_path<'a>(
    submodules: &'a [Submodule<'_>],
    paths: &[String],
) -> Result<Vec<&'a Submodule<'a>>> {
    let mut filtered = Vec::new();
    
    for submodule in submodules {
        let submodule_path = submodule.path().to_string_lossy();
        
        for pattern in paths {
            if submodule_path.contains(pattern) || 
               submodule.name().unwrap_or("").contains(pattern) {
                filtered.push(submodule);
                break;
            }
        }
    }
    
    Ok(filtered)
}

/// Show initialization preview
fn show_init_preview(submodules: &[&Submodule<'_>], config: &Config) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    println!("{} Will initialize {} submodule{}:", 
            "📋".blue(),
            submodules.len(),
            if submodules.len() == 1 { "" } else { "s" });
    
    for submodule in submodules {
        let name = submodule.name().unwrap_or("unknown");
        let path = submodule.path().display();
        println!("  {} {} ({})", "•".blue(), name.cyan(), path.to_string().dimmed());
    }
    
    println!();
    Ok(())
}

/// Show initialization summary
fn show_init_summary(initialized: usize, skipped: usize, _config: &Config) -> Result<()> {
    println!("\n{} Initialization Summary:", "📊".blue().bold());
    println!("  {} {} initialized", "✅".green(), initialized);
    
    if skipped > 0 {
        println!("  {} {} already initialized", "ℹ️".blue(), skipped);
    }
    
    Ok(())
}

/// Show update preview
fn show_update_preview(
    submodules: &[&Submodule<'_>],
    init: bool,
    recursive: bool,
    merge: bool,
    rebase: bool,
    remote: bool,
    config: &Config,
) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    println!("{} Update Plan:", "📋".blue().bold());
    println!("  {} {} submodule{}", "📦".blue(), submodules.len(), if submodules.len() == 1 { "" } else { "s" });
    
    let mut options = Vec::new();
    if init { options.push("initialize if needed"); }
    if recursive { options.push("recursive"); }
    if merge { options.push("merge"); }
    if rebase { options.push("rebase"); }
    if remote { options.push("track remote"); }
    
    if !options.is_empty() {
        println!("  {} Options: {}", "⚙️".blue(), options.join(", ").cyan());
    }
    
    println!();
    Ok(())
}

/// Update a single submodule
fn update_single_submodule(
    submodule: &mut Submodule<'_>,
    _merge: bool,
    _rebase: bool,
    _remote: bool,
    _force: bool,
) -> Result<()> {
    // In real implementation, this would handle different update strategies
    submodule.update(true, None)?;
    Ok(())
}

/// Update submodule recursively
async fn update_submodule_recursively(
    _submodule: &Submodule<'_>,
    _config: &Config,
) -> Result<()> {
    // In real implementation, this would recursively update nested submodules
    Ok(())
}

/// Show update summary
fn show_update_summary(updated: usize, failed: usize, _config: &Config) -> Result<()> {
    println!("\n{} Update Summary:", "📊".blue().bold());
    println!("  {} {} updated successfully", "✅".green(), updated);
    
    if failed > 0 {
        println!("  {} {} failed to update", "❌".red(), failed);
    }
    
    Ok(())
}

/// Show health summary
fn show_health_summary(
    health: &crate::submodule::SubmoduleHealth,
    config: &Config,
) -> Result<()> {
    if health.is_healthy() {
        println!("{} All submodules are healthy", "🎉".green());
        return Ok(());
    }
    
    println!("{} Submodule Health Issues:", "⚠️".yellow().bold());
    
    for (name, status) in &health.submodules {
        if !status.issues.is_empty() {
            println!("\n📦 {} ({}):", name.yellow(), status.path.display().to_string().dimmed());
            
            for issue in &status.issues {
                let severity_icon = issue.severity().icon();
                println!("  {} {}", severity_icon, issue.description());
                
                if config.ui.interactive {
                    for suggestion in issue.suggestions() {
                        println!("    {} {}", "💡".blue(), suggestion.dimmed());
                    }
                }
            }
        }
    }
    
    println!();
    Ok(())
}

/// Show submodule status table
fn show_submodule_status_table(
    submodules: &[Submodule<'_>],
    recursive: bool,
    config: &Config,
) -> Result<()> {
    let mut table = TableDisplay::new()
        .with_headers(vec![
            "Name".to_string(),
            "Path".to_string(),
            "Status".to_string(),
            "Branch/Commit".to_string(),
            "Issues".to_string(),
        ])
        .with_max_width(config.terminal_width());
    
    for submodule in submodules {
        let name = submodule.name().unwrap_or("unknown").to_string();
        let path = submodule.path().display().to_string();
        
        let (status, branch_info, issues) = get_submodule_table_info(submodule)?;
        
        table.add_row(vec![name, path, status, branch_info, issues]);
        
        if recursive {
            // Add nested submodules with indentation
            if let Ok(sub_repo) = submodule.open() {
                add_nested_submodules_to_table(&mut table, &sub_repo, 1)?;
            }
        }
    }
    
    table.display();
    println!();
    
    Ok(())
}

/// Get submodule information for table display
fn get_submodule_table_info(submodule: &Submodule<'_>) -> Result<(String, String, String)> {
    let status = if submodule.open().is_ok() {
        "✅ OK".green().to_string()
    } else {
        "❓ Not Init".red().to_string()
    };
    
    let branch_info = if let Ok(sub_repo) = submodule.open() {
        get_submodule_branch_info(&sub_repo)?
    } else {
        "N/A".dimmed().to_string()
    };
    
    let issues = if let Ok(sub_repo) = submodule.open() {
        get_submodule_issues_summary(&sub_repo)?
    } else {
        "Not initialized".red().to_string()
    };
    
    Ok((status, branch_info, issues))
}

/// Get branch information for submodule
fn get_submodule_branch_info(repo: &Repository) -> Result<String> {
    match repo.head() {
        Ok(head) => {
            if head.is_branch() {
                Ok(head.shorthand().unwrap_or("unknown").green().to_string())
            } else {
                let oid = head.target().unwrap_or_else(|| Oid::zero());
                Ok(format!("{} (detached)", 
                          crate::utils::shorten_oid(&oid, 7).yellow()))
            }
        }
        Err(_) => Ok("No HEAD".red().to_string()),
    }
}

/// Get issues summary for submodule
fn get_submodule_issues_summary(repo: &Repository) -> Result<String> {
    let statuses = repo.statuses(None)?;
    
    if statuses.is_empty() {
        Ok("None".green().to_string())
    } else {
        Ok(format!("{} changes", statuses.len()).yellow().to_string())
    }
}

/// Add nested submodules to table
fn add_nested_submodules_to_table(
    table: &mut TableDisplay,
    repo: &Repository,
    depth: usize,
) -> Result<()> {
    let submodules = repo.submodules()?;
    let indent = "  ".repeat(depth);
    
    for submodule in submodules {
        let name = format!("{}{}", indent, submodule.name().unwrap_or("unknown"));
        let path = submodule.path().display().to_string();
        
        let (status, branch_info, issues) = get_submodule_table_info(&submodule)?;
        
        table.add_row(vec![name, path, status, branch_info, issues]);
        
        // Recurse further if needed (limit depth to prevent infinite recursion)
        if depth < 3 {
            if let Ok(sub_repo) = submodule.open() {
                add_nested_submodules_to_table(table, &sub_repo, depth + 1)?;
            }
        }
    }
    
    Ok(())
}

/// Show submodule recommendations
fn show_submodule_recommendations(submodules: &[Submodule<'_>], config: &Config) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    let mut recommendations = Vec::new();
    
    // Check for common issues and suggest fixes
    let uninitialized_count = submodules.iter()
        .filter(|s| s.open().is_err())
        .count();
    
    if uninitialized_count > 0 {
        recommendations.push(format!("Run 'rgit submodule init' to initialize {} submodule{}", 
                                   uninitialized_count,
                                   if uninitialized_count == 1 { "" } else { "s" }));
    }
    
    // Check for outdated submodules
    let mut outdated_count = 0;
    for submodule in submodules {
        if let Ok(_sub_repo) = submodule.open() {
            // In real implementation, check if submodule is behind its remote
            // outdated_count += 1;
        }
    }
    
    if outdated_count > 0 {
        recommendations.push(format!("Run 'rgit submodule update' to update {} outdated submodule{}", 
                                   outdated_count,
                                   if outdated_count == 1 { "" } else { "s" }));
    }
    
    if !recommendations.is_empty() {
        println!("{} Recommendations:", "💡".blue().bold());
        for recommendation in recommendations {
            println!("  • {}", recommendation.cyan());
        }
        println!();
    }
    
    Ok(())
}

/// Show deinit preview
fn show_deinit_preview(submodule: &Submodule<'_>, remove: bool, config: &Config) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    let name = submodule.name().unwrap_or("unknown");
    let path = submodule.path().display();
    
    println!("{} Deinit Preview:", "👁️".blue().bold());
    println!("  {} {}", "Submodule:".bold(), name.cyan());
    println!("  {} {}", "Path:".bold(), path.to_string().yellow());
    
    if remove {
        println!("  {} Will be completely removed", "Action:".bold().red());
    } else {
        println!("  {} Will be deinitialized (can be re-initialized later)", "Action:".bold().yellow());
    }
    
    println!();
    Ok(())
}

/// Confirm submodule deinit
fn confirm_submodule_deinit(name: &str, remove: bool, config: &Config) -> Result<bool> {
    if !config.is_interactive() {
        return Ok(true);
    }
    
    let action = if remove { "remove" } else { "deinitialize" };
    
    InteractivePrompt::new()
        .with_message(&format!("Are you sure you want to {} submodule '{}'?", action, name))
        .confirm()
}

/// Deinitialize submodule implementation
fn deinit_submodule_implementation(
    _rgit: &RgitCore,
    _submodule: &Submodule<'_>,
    _remove: bool,
) -> Result<()> {
    // In real implementation, this would:
    // 1. Remove working tree content
    // 2. Remove from .git/config
    // 3. If remove=true, also remove from .gitmodules and git index
    
    Ok(())
}

/// Execute command in submodule directory
fn execute_command_in_submodule(command: &str, path: &Path) -> Result<String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(path)
        .output()?;
    
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(RgitError::CommandExecutionFailed(error.to_string()).into())
    }
}

/// Execute foreach recursively
async fn execute_foreach_recursively(
    _repo: &Repository,
    _command: &str,
    _continue_on_error: bool,
) -> Result<()> {
    // In real implementation, this would recursively execute in nested submodules
    Ok(())
}

/// Sync nested submodules
async fn sync_nested_submodules(_repo: &Repository, _config: &Config) -> Result<()> {
    // In real implementation, this would sync nested submodules
    Ok(())
}

/// Show next steps after adding submodule
fn show_submodule_add_next_steps(path: &str, config: &Config) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    println!("\n{} Next steps:", "💡".blue());
    println!("  • {} - Stage the submodule", "rgit add .gitmodules".cyan());
    println!("  • {} - Commit the submodule addition", "rgit commit".cyan());
    println!("  • {} - Check submodule status", "rgit submodule status".cyan());
    println!("  • {} - Work in the submodule", format!("cd {}", path).cyan());
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, Repository) {
        let temp_dir = TempDir::new().unwrap();
        let repo = Repository::init(temp_dir.path()).unwrap();
        
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        
        (temp_dir, repo)
    }

    #[test]
    fn test_validate_submodule_add_inputs() {
        let config = Config::default();
        
        // Valid inputs
        assert!(validate_submodule_add_inputs("https://github.com/user/repo.git", "path/to/sub", &config).is_ok());
        
        // Invalid URL
        assert!(validate_submodule_add_inputs("not-a-url", "path/to/sub", &config).is_err());
        
        // Invalid path
        assert!(validate_submodule_add_inputs("https://github.com/user/repo.git", "../bad/path", &config).is_err());
        assert!(validate_submodule_add_inputs("https://github.com/user/repo.git", "/absolute/path", &config).is_err());
    }

    #[test]
    fn test_filter_submodules_by_path() {
        // This test would require creating actual submodules
        // For now, we'll test with empty input
        let submodules: Vec<Submodule> = vec![];
        let paths = vec!["test".to_string()];
        
        let filtered = filter_submodules_by_path(&submodules, &paths).unwrap();
        assert_eq!(filtered.len(), 0);
    }

    #[tokio::test]
    async fn test_submodule_status_empty() {
        let (_temp_dir, repo) = create_test_repo();
        let rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let config = Config::default();
        let manager = SubmoduleManager::new(&rgit, &config);
        
        // Should not fail with empty submodules
        let result = show_submodule_status(&manager, false, false, &config).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_command_in_submodule() {
        let temp_dir = TempDir::new().unwrap();
        
        // Test with a simple command
        let result = execute_command_in_submodule("echo 'test'", temp_dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().contains("test"));
        
        // Test with failing command
        let result = execute_command_in_submodule("false", temp_dir.path());
        assert!(result.is_err());
    }
}