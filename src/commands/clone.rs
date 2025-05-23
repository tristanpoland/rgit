use anyhow::Result;
use colored::*;
use git2::{build::RepoBuilder, FetchOptions, Progress, RemoteCallbacks};
use std::cell::RefCell;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::cli::CloneArgs;
use crate::config::Config;
use crate::core::RgitCore;
use crate::error::RgitError;
use crate::interactive::InteractivePrompt;

/// Execute the clone command
pub async fn execute(args: &CloneArgs, _rgit: &RgitCore, config: &Config) -> Result<()> {
    println!("{} Cloning repository...", "🚀".blue().bold());
    
    let repo_url = &args.url; // Fixed: changed from args.repository to args.url
    let target_dir = args.directory.as_ref()
        .map(|d| PathBuf::from(d)) // Fixed: convert String to PathBuf
        .unwrap_or_else(|| PathBuf::from(extract_repo_name(repo_url)));
    
    // Validate URL
    if !is_valid_git_url(repo_url) {
        return Err(RgitError::InvalidRemoteUrl(repo_url.clone().to_owned()).into());
    }
    
    // Check if directory already exists
    if target_dir.exists() { // Fixed: now works with PathBuf
        if !target_dir.read_dir()?.next().is_none() { // Fixed: now works with PathBuf
            if config.is_interactive() {
                let overwrite = InteractivePrompt::new()
                    .with_message(&format!("Directory '{}' is not empty. Continue anyway?", target_dir.display()))
                    .confirm()?;
                
                if !overwrite {
                    println!("{} Clone cancelled", "❌".red());
                    return Ok(());
                }
            } else {
                // Note: You'll need to add DirectoryNotEmpty variant to RgitError enum
                return Err(anyhow::anyhow!("Directory '{}' is not empty", target_dir.display()));
            }
        }
    }
    
    // Show clone details
    println!("{} Repository: {}", "📡".blue(), repo_url.cyan());
    println!("{} Target: {}", "📁".blue(), target_dir.display().to_string().yellow());
    
    if let Some(branch) = &args.branch {
        println!("{} Branch: {}", "🌿".green(), branch.green());
    }
    
    if let Some(depth) = args.depth {
        println!("{} Depth: {} (shallow clone)", "📏".yellow(), depth);
    }
    
    // Note: You'll need to add these fields to CloneArgs or remove these checks
    // if args.bare {
    //     println!("{} Mode: Bare repository", "📦".blue());
    // }
    
    // if args.mirror {
    //     println!("{} Mode: Mirror repository", "🪞".blue());
    // }
    
    // Perform the clone
    println!("\n{} Cloning...", "⏳".yellow());
    
    let progress = Arc::new(RefCell::new(CloneProgress::new()));
    let cancelled = Arc::new(AtomicBool::new(false));
    
    match perform_clone(repo_url, &target_dir, args, progress.clone(), cancelled.clone()).await {
        Ok(repo) => {
            println!("\n{} Successfully cloned to {}", 
                    "✅".green().bold(), 
                    target_dir.display().to_string().cyan());
            
            // Show repository info
            show_repo_info(&repo, config)?;
            
            // Show next steps
            println!("\n{} Next steps:", "💡".blue());
            println!("  • {} - Enter the repository", format!("cd {}", target_dir.display()).cyan());
            println!("  • {} - Check repository status", "rgit status".cyan());
            println!("  • {} - View recent commits", "rgit log".cyan());
            
            if args.recursive {
                println!("  • {} - Initialize submodules", "rgit submodule update --init".cyan());
            }
        }
        Err(e) => {
            // Clean up on failure
            if target_dir.exists() {
                let _ = std::fs::remove_dir_all(&target_dir);
            }
            
            println!("{} Clone failed: {}", "❌".red().bold(), e);
            return Err(e);
        }
    }
    
    Ok(())
}

/// Progress tracking for clone operations
struct CloneProgress {
    total_objects: usize,
    received_objects: usize,
    received_bytes: usize,
    indexed_objects: usize,
    indexed_deltas: usize,
    total_deltas: usize,
}

impl CloneProgress {
    fn new() -> Self {
        Self {
            total_objects: 0,
            received_objects: 0,
            received_bytes: 0,
            indexed_objects: 0,
            indexed_deltas: 0,
            total_deltas: 0,
        }
    }
    
    fn update(&mut self, progress: Progress) {
        self.total_objects = progress.total_objects();
        self.received_objects = progress.received_objects();
        self.received_bytes = progress.received_bytes();
        self.indexed_objects = progress.indexed_objects();
        self.indexed_deltas = progress.indexed_deltas();
        self.total_deltas = progress.total_deltas();
        
        self.display();
    }
    
    fn display(&self) {
        if self.total_objects > 0 {
            let receive_percent = (self.received_objects * 100) / self.total_objects;
            print!("\r{} Receiving objects: {}% ({}/{}), {} bytes", 
                   "📥".green(),
                   receive_percent,
                   self.received_objects,
                   self.total_objects,
                   format_bytes(self.received_bytes));
        }
        
        if self.total_deltas > 0 && self.indexed_deltas > 0 {
            let delta_percent = (self.indexed_deltas * 100) / self.total_deltas;
            print!("\r{} Resolving deltas: {}% ({}/{})", 
                   "🔧".yellow(),
                   delta_percent,
                   self.indexed_deltas,
                   self.total_deltas);
        }
        
        io::stdout().flush().unwrap();
    }
}

/// Perform the actual clone operation
async fn perform_clone(
    url: &str,
    target: &Path,
    args: &CloneArgs,
    progress: Arc<RefCell<CloneProgress>>,
    _cancelled: Arc<AtomicBool>,
) -> Result<git2::Repository> {
    let mut builder = RepoBuilder::new();
    
    // Set up progress callback
    let mut callbacks = RemoteCallbacks::new();
    // Fixed: use correct method name for git2
    callbacks.transfer_progress(|stats| {
        progress.borrow_mut().update(stats);
        true
    });
    
    // Set up fetch options
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);
    
    // Configure clone options - commented out since fields don't exist in CloneArgs
    // if args.bare {
    //     builder.bare(true);
    // }
    
    // Note: mirror() method may not exist in git2 - check documentation
    // if args.mirror {
    //     builder.mirror(true);
    // }
    
    if let Some(branch) = &args.branch {
        builder.branch(branch);
    }
    
    if let Some(depth) = args.depth {
        fetch_options.depth(depth as i32);
    }
    
    builder.fetch_options(fetch_options);
    
    // Perform clone
    let repo = builder.clone(url, target)
        .map_err(|e| anyhow::anyhow!("Clone failed: {}", e.message()))?;
    
    // Handle submodules if requested
    if args.recursive {
        println!("\n{} Initializing submodules...", "🔗".blue());
        init_submodules(&repo)?;
    }
    
    println!(); // New line after progress
    Ok(repo)
}

/// Initialize submodules recursively
fn init_submodules(repo: &git2::Repository) -> Result<()> {
    let submodules = repo.submodules()?;
    
    for mut submodule in submodules {
        println!("  {} Initializing submodule: {}", 
                "🔗".blue(), 
                submodule.name().unwrap_or("unnamed").cyan());
        
        submodule.init(false)?;
        
        submodule.update(true, None)?;
        
        // Recursively init submodules in submodules
        let subrepo = submodule.open()?;
        let sub_submodules = subrepo.submodules();
        if let Ok(sub_submodules) = &sub_submodules {
            if !sub_submodules.is_empty() {
                init_submodules(&subrepo)?;
            }
        }
    }
    
    Ok(())
}

/// Extract repository name from URL
fn extract_repo_name(url: &str) -> String {
    url
        .trim_end_matches(".git")
        .trim_end_matches('/')
        .split('/')
        .last()
        .unwrap_or("repository")
        .to_string()
}

/// Validate if the URL is a valid git repository URL
fn is_valid_git_url(url: &str) -> bool {
    // Basic validation - can be extended
    url.starts_with("http://") 
        || url.starts_with("https://") 
        || url.starts_with("git://")
        || url.starts_with("ssh://")
        || url.starts_with("git@")
        || url.ends_with(".git")
        || std::path::Path::new(url).exists()
}

/// Show repository information after successful clone
fn show_repo_info(repo: &git2::Repository, config: &Config) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    println!("\n{} Repository Information:", "📊".blue().bold());
    
    // Show HEAD reference
    if let Ok(head) = repo.head() {
        if let Some(name) = head.shorthand() {
            println!("  {} Current branch: {}", "🌿".green(), name.cyan());
        }
        
        if let Ok(commit) = head.peel_to_commit() {
            let summary = commit.summary().unwrap_or("No commit message");
            let author = commit.author();
            
            println!("  {} Latest commit: {}", "📝".yellow(), 
                    commit.id().to_string()[..8].yellow());
            println!("    {} {}", "💬".blue(), summary.white());
            println!("    {} {} <{}>", "👤".blue(), 
                    author.name().unwrap_or("Unknown"),
                    author.email().unwrap_or("unknown@example.com"));
        }
    }
    
    // Show remotes
    if let Ok(remotes) = repo.remotes() {
        if let Some(remote_names) = remotes.iter().collect::<Option<Vec<_>>>() {
            if !remote_names.is_empty() {
                println!("  {} Remotes:", "🌐".blue());
                for remote_name in remote_names {
                    if let Ok(remote) = repo.find_remote(remote_name) {
                        if let Some(url) = remote.url() {
                            println!("    {} {} -> {}", "•".green(), remote_name.cyan(), url.dimmed());
                        }
                    }
                }
            }
        }
    }
    
    // Show file count
    if let Ok(index) = repo.index() {
        let file_count = index.len();
        if file_count > 0 {
            println!("  {} Files: {}", "📁".blue(), file_count.to_string().yellow());
        }
    }
    
    Ok(())
}

/// Format bytes for display
fn format_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", size as usize, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_repo_name() {
        assert_eq!(extract_repo_name("https://github.com/user/repo.git"), "repo");
        assert_eq!(extract_repo_name("git@github.com:user/repo.git"), "repo");
        assert_eq!(extract_repo_name("https://github.com/user/repo"), "repo");
        assert_eq!(extract_repo_name("/local/path/repo"), "repo");
    }

    #[test]
    fn test_is_valid_git_url() {
        assert!(is_valid_git_url("https://github.com/user/repo.git"));
        assert!(is_valid_git_url("git@github.com:user/repo.git"));
        assert!(is_valid_git_url("ssh://git@github.com/user/repo.git"));
        assert!(is_valid_git_url("file:///local/repo.git"));
        assert!(!is_valid_git_url("invalid-url"));
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
    }
}