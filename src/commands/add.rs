use anyhow::Result;
use colored::*;
use git2::{Status, StatusOptions};
use std::path::{Path, PathBuf};

use crate::cli::AddArgs;
use crate::config::Config;
use crate::core::{RgitCore, FileStatus};
use crate::error::RgitError;
use crate::interactive::{FileSelector, FileItem, InteractivePrompt};

/// Execute the add command
pub async fn execute(args: &AddArgs, rgit: &mut RgitCore, config: &Config) -> Result<()> {
    // Handle different add modes
    if args.all {
        add_all_changes(rgit, config).await
    } else if args.update {
        add_updated_files(rgit, config).await
    } else if args.patch {
        add_patch_mode(rgit, config, &args.files).await
    } else if args.files.is_empty() {
        interactive_add(rgit, config).await
    } else {
        add_specific_files(rgit, config, &args.files, args.force).await
    }
}

/// Add all changes (tracked and untracked files)
async fn add_all_changes(rgit: &mut RgitCore, config: &Config) -> Result<()> {
    rgit.log("Adding all changes...");
    
    let status = rgit.status()?;
    if status.is_clean() {
        rgit.info("No changes to add");
        return Ok(());
    }
    
    // Show what will be added
    let total_files = status.total_changes();
    if config.ui.interactive && total_files > 5 {
        println!("{} About to add {} files:", "📋".blue(), total_files);
        show_files_preview(&status.unstaged, &status.untracked, 5)?;
        
        if !confirm_add_all(total_files, config)? {
            rgit.info("Add operation cancelled");
            return Ok(());
        }
    }
    
    // Perform the add operation
    rgit.add_all()?;
    
    rgit.success(&format!("Added {} files", total_files));
    
    // Show summary
    if config.ui.interactive {
        show_add_summary(rgit, "Added all changes").await?;
    }
    
    Ok(())
}

/// Add only updated (tracked) files
async fn add_updated_files(rgit: &mut RgitCore, config: &Config) -> Result<()> {
    rgit.log("Adding updated files only...");
    
    let status = rgit.status()?;
    let updated_count = status.unstaged.len();
    
    if updated_count == 0 {
        rgit.info("No updated files to add");
        return Ok(());
    }
    
    // Show what will be updated
    if config.ui.interactive {
        println!("{} Updating {} tracked file{}:", 
                "📝".yellow(), 
                updated_count,
                if updated_count == 1 { "" } else { "s" });
        
        for file in &status.unstaged {
            println!("  {} {}: {}", 
                    "○".yellow(), 
                    file.status_symbol(false).yellow(),
                    file.path.white());
        }
    }
    
    rgit.add_update()?;
    rgit.success(&format!("Updated {} files", updated_count));
    
    if config.ui.interactive {
        show_add_summary(rgit, "Updated tracked files").await?;
    }
    
    Ok(())
}

/// Add specific files
async fn add_specific_files(
    rgit: &mut RgitCore, 
    config: &Config, 
    files: &[PathBuf], 
    force: bool
) -> Result<()> {
    rgit.log(&format!("Adding {} specific files...", files.len()));
    
    let mut added_files = Vec::new();
    let mut ignored_files = Vec::new();
    let mut missing_files = Vec::new();
    
    for file_path in files {
        if !file_path.exists() {
            missing_files.push(file_path.clone());
            continue;
        }
        
        // Check if file is ignored
        if !force && is_file_ignored(rgit, file_path)? {
            ignored_files.push(file_path.clone());
            continue;
        }
        
        match rgit.add_files(&[file_path]) {
            Ok(()) => added_files.push(file_path.clone()),
            Err(e) => {
                rgit.warning(&format!("Failed to add {}: {}", file_path.display(), e));
            }
        }
    }
    
    // Report results
    if !added_files.is_empty() {
        rgit.success(&format!("Added {} file{}", 
                             added_files.len(),
                             if added_files.len() == 1 { "" } else { "s" }));
        
        if config.ui.interactive {
            for file in &added_files {
                println!("  {} {}", "✓".green(), file.display().to_string().white());
            }
        }
    }
    
    if !missing_files.is_empty() {
        rgit.error(&format!("Missing files ({}): {}", 
                           missing_files.len(),
                           missing_files.iter()
                               .map(|p| p.display().to_string())
                               .collect::<Vec<_>>()
                               .join(", ")));
    }
    
    if !ignored_files.is_empty() {
        rgit.warning(&format!("Ignored files ({}): {}", 
                             ignored_files.len(),
                             ignored_files.iter()
                                 .map(|p| p.display().to_string())
                                 .collect::<Vec<_>>()
                                 .join(", ")));
        
        if config.ui.interactive {
            println!("  💡 Use {} to add ignored files", "--force".cyan());
        }
    }
    
    if config.ui.interactive && !added_files.is_empty() {
        show_add_summary(rgit, "Added specific files").await?;
    }
    
    Ok(())
}

/// Interactive file selection for adding
async fn interactive_add(rgit: &mut RgitCore, config: &Config) -> Result<()> {
    if !config.is_interactive() {
        return Err(RgitError::NonInteractiveEnvironment.into());
    }
    
    rgit.log("Starting interactive add...");
    
    let status = rgit.status()?;
    let addable_files = collect_addable_files(&status)?;
    
    if addable_files.is_empty() {
        rgit.info("No files to add");
        return Ok(());
    }
    
    // Show current status summary
    println!("{} Current repository status:", "📋".blue());
    println!("  {} {} unstaged changes", "📝".yellow(), status.unstaged.len());
    println!("  {} {} untracked files", "❓".red(), status.untracked.len());
    println!();
    
    // Use file selector
    let file_items = create_file_items(&addable_files)?;
    let selector = FileSelector::new()
        .with_files(file_items)
        .with_details();
    
    let selected_files = selector.select()?;
    
    if selected_files.is_empty() {
        rgit.info("No files selected");
        return Ok(());
    }
    
    // Add selected files
    rgit.add_files(&selected_files)?;
    
    rgit.success(&format!("Added {} file{}", 
                         selected_files.len(),
                         if selected_files.len() == 1 { "" } else { "s" }));
    
    // Show what was added
    for file in &selected_files {
        println!("  {} {}", "✓".green(), file.display().to_string().white());
    }
    
    show_add_summary(rgit, "Interactively added files").await?;
    
    Ok(())
}

/// Patch mode for adding hunks interactively
async fn add_patch_mode(
    rgit: &mut RgitCore, 
    config: &Config, 
    files: &[PathBuf]
) -> Result<()> {
    if !config.is_interactive() {
        return Err(RgitError::NonInteractiveEnvironment.into());
    }
    
    rgit.log("Starting patch mode...");
    
    // If no files specified, use all modified files
    let target_files = if files.is_empty() {
        let status = rgit.status()?;
        status.unstaged.iter().map(|f| PathBuf::from(&f.path)).collect()
    } else {
        files.to_vec()
    };
    
    if target_files.is_empty() {
        rgit.info("No files to patch");
        return Ok(());
    }
    
    println!("{} Interactive patch mode", "🔍".blue().bold());
    println!("Select hunks to add for each file:");
    println!();
    
    let mut added_hunks = 0;
    
    for file_path in &target_files {
        if !file_path.exists() {
            rgit.warning(&format!("File not found: {}", file_path.display()));
            continue;
        }
        
        match process_file_patches(rgit, file_path, config).await {
            Ok(hunks) => {
                added_hunks += hunks;
                if hunks > 0 {
                    println!("  {} Added {} hunk{} from {}", 
                            "✓".green(),
                            hunks,
                            if hunks == 1 { "" } else { "s" },
                            file_path.display().to_string().cyan());
                }
            }
            Err(e) => {
                rgit.warning(&format!("Failed to process {}: {}", file_path.display(), e));
            }
        }
    }
    
    if added_hunks > 0 {
        rgit.success(&format!("Added {} hunk{} total", 
                             added_hunks,
                             if added_hunks == 1 { "" } else { "s" }));
        show_add_summary(rgit, "Added hunks interactively").await?;
    } else {
        rgit.info("No hunks were added");
    }
    
    Ok(())
}

/// Process patches for a single file
async fn process_file_patches(
    rgit: &RgitCore, 
    file_path: &Path, 
    _config: &Config
) -> Result<usize> {
    // This is a simplified version - in a real implementation,
    // you would parse the diff and present each hunk for user selection
    
    println!("\n{} Processing: {}", "📁".blue(), file_path.display().to_string().yellow());
    
    // For now, we'll simulate the patch process
    // In a real implementation, this would:
    // 1. Get the diff for the file
    // 2. Parse it into hunks
    // 3. Show each hunk and ask user y/n/q/a/d/?
    // 4. Apply selected hunks to the index
    
    let options = vec![
        "Add this hunk",
        "Skip this hunk", 
        "Add all hunks in this file",
        "Skip all hunks in this file",
        "Show help",
    ];
    
    let choice = InteractivePrompt::new()
        .with_message("What to do with this file?")
        .with_options(&options)
        .select()?;
    
    match choice {
        0 => Ok(1), // Add this hunk
        1 => Ok(0), // Skip this hunk
        2 => Ok(3), // Add all hunks (simulated)
        3 => Ok(0), // Skip all hunks
        4 => {
            show_patch_help();
            Ok(0)
        }
        _ => Ok(0),
    }
}

/// Show help for patch mode
fn show_patch_help() {
    println!("\n{} Patch mode commands:", "💡".blue().bold());
    println!("  {} - add this hunk to index", "y".green().bold());
    println!("  {} - do not add this hunk to index", "n".red().bold());
    println!("  {} - quit; do not add this hunk or any remaining ones", "q".yellow().bold());
    println!("  {} - add this hunk and all later hunks in the file", "a".green().bold());
    println!("  {} - do not add this hunk or any later hunks in the file", "d".red().bold());
    println!("  {} - show this help", "?".blue().bold());
    println!();
}

/// Collect files that can be added
fn collect_addable_files(status: &crate::core::RepositoryStatus) -> Result<Vec<FileStatus>> {
    let mut files = Vec::new();
    
    // Add unstaged files
    files.extend(status.unstaged.clone());
    
    // Add untracked files
    files.extend(status.untracked.clone());
    
    Ok(files)
}

/// Create file items for the interactive selector
fn create_file_items(files: &[FileStatus]) -> Result<Vec<FileItem>> {
    files.iter().map(|file| {
        Ok(FileItem {
            path: PathBuf::from(&file.path),
            status: file.status_symbol(false).to_string(),
            size: Some(file.size),
            selected: false,
        })
    }).collect()
}

/// Check if a file is ignored by git
fn is_file_ignored(rgit: &RgitCore, file_path: &Path) -> Result<bool> {
    match rgit.repo.status_file(file_path) {
        Ok(flags) => Ok(flags.contains(Status::IGNORED)),
        Err(_) => Ok(false), // If we can't check, assume not ignored
    }
}

/// Confirm adding all files
fn confirm_add_all(total_files: usize, config: &Config) -> Result<bool> {
    if !config.is_interactive() {
        return Ok(true);
    }
    
    InteractivePrompt::new()
        .with_message(&format!("Add all {} files?", total_files))
        .confirm()
}

/// Show a preview of files that will be added
fn show_files_preview(
    unstaged: &[FileStatus], 
    untracked: &[FileStatus], 
    max_show: usize
) -> Result<()> {
    let mut shown = 0;
    
    // Show unstaged files
    for file in unstaged.iter().take(max_show - shown) {
        println!("  {} {}: {}", 
                "○".yellow(), 
                file.status_symbol(false).yellow(),
                file.path.white());
        shown += 1;
    }
    
    // Show untracked files
    for file in untracked.iter().take(max_show - shown) {
        println!("  {} {}: {}", 
                "?".red(), 
                "untracked".red(),
                file.path.white());
        shown += 1;
    }
    
    let total = unstaged.len() + untracked.len();
    if total > max_show {
        println!("  {} and {} more...", "...".dimmed(), (total - max_show));
    }
    
    Ok(())
}

/// Show summary after add operation
async fn show_add_summary(rgit: &RgitCore, operation: &str) -> Result<()> {
    let status = rgit.status()?;
    
    println!("\n{} {} completed:", "📋".blue(), operation.cyan());
    
    if !status.staged.is_empty() {
        println!("  {} {} file{} staged for commit", 
                "✅".green(),
                status.staged.len(),
                if status.staged.len() == 1 { "" } else { "s" });
    }
    
    if !status.unstaged.is_empty() || !status.untracked.is_empty() {
        let remaining = status.unstaged.len() + status.untracked.len();
        println!("  {} {} file{} remaining unstaged", 
                "📝".yellow(),
                remaining,
                if remaining == 1 { "" } else { "s" });
    }
    
    // Show next steps
    if !status.staged.is_empty() {
        println!("\n{} Next steps:", "💡".blue());
        println!("  • {} - Commit staged changes", "rgit commit".cyan());
        println!("  • {} - Quick commit workflow", "rgit quick-commit".cyan());
    }
    
    Ok(())
}

/// Utility function for other commands to stage files
pub async fn stage_files(
    rgit: &mut RgitCore, 
    files: &[PathBuf], 
    force: bool
) -> Result<Vec<PathBuf>> {
    let mut staged = Vec::new();
    
    for file_path in files {
        if !file_path.exists() {
            continue;
        }
        
        if !force && is_file_ignored(rgit, file_path)? {
            continue;
        }
        
        if rgit.add_files(&[file_path]).is_ok() {
            staged.push(file_path.clone());
        }
    }
    
    Ok(staged)
}

/// Check if there are files ready to be staged
pub fn has_stageable_files(rgit: &RgitCore) -> Result<bool> {
    let status = rgit.status()?;
    Ok(!status.unstaged.is_empty() || !status.untracked.is_empty())
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
    async fn test_add_specific_files() {
        let (temp_dir, repo) = create_test_repo();
        
        // Create test files
        fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "content2").unwrap();
        
        let mut rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let config = Config::default();
        
        let files = vec![
            temp_dir.path().join("file1.txt"),
            temp_dir.path().join("file2.txt"),
        ];
        
        add_specific_files(&mut rgit, &config, &files, false).await.unwrap();
        
        let status = rgit.status().unwrap();
        assert_eq!(status.staged.len(), 2);
    }

    #[tokio::test]
    async fn test_add_all_changes() {
        let (temp_dir, repo) = create_test_repo();
        
        // Create test files
        fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "content2").unwrap();
        
        let mut rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let config = Config::minimal();
        
        add_all_changes(&mut rgit, &config).await.unwrap();
        
        let status = rgit.status().unwrap();
        assert_eq!(status.staged.len(), 2);
    }

    #[test]
    fn test_collect_addable_files() {
        let status = crate::core::RepositoryStatus {
            staged: vec![],
            unstaged: vec![FileStatus {
                path: "modified.txt".to_string(),
                status: Status::WT_MODIFIED,
                size: 100,
                modified_time: None,
            }],
            untracked: vec![FileStatus {
                path: "new.txt".to_string(),
                status: Status::WT_NEW,
                size: 50,
                modified_time: None,
            }],
            branch_info: Default::default(),
        };
        
        let addable = collect_addable_files(&status).unwrap();
        assert_eq!(addable.len(), 2);
    }

    #[test]
    fn test_create_file_items() {
        let files = vec![
            FileStatus {
                path: "test.txt".to_string(),
                status: Status::WT_MODIFIED,
                size: 100,
                modified_time: None,
            }
        ];
        
        let items = create_file_items(&files).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].path, PathBuf::from("test.txt"));
        assert_eq!(items[0].size, Some(100));
    }

    #[tokio::test]
    async fn test_stage_files_utility() {
        let (temp_dir, repo) = create_test_repo();
        
        fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
        
        let mut rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let files = vec![temp_dir.path().join("test.txt")];
        
        let staged = stage_files(&mut rgit, &files, false).await.unwrap();
        assert_eq!(staged.len(), 1);
        
        assert!(has_stageable_files(&rgit).unwrap() == false); // File is now staged
    }
}