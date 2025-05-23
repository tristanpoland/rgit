use anyhow::{Context, Result};
use git2::*;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use tracing::{debug, info, warn};
use colored::*;
use crate::error::RgitError;
// Remove unused imports
// use crate::utils::{format_time, calculate_file_changes, get_branch_status};

/// Central Git repository manager with enhanced functionality
pub struct RgitCore {
    /// The underlying git2 repository
    pub repo: Repository,
    /// Working directory path
    pub repo_path: PathBuf,
    /// Verbose logging enabled
    pub verbose: bool,
    /// Configuration cache
    config_cache: HashMap<String, String>,
}

// Manual Debug implementation to handle Repository which doesn't implement Debug
impl std::fmt::Debug for RgitCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RgitCore")
            .field("repo_path", &self.repo_path)
            .field("verbose", &self.verbose)
            .field("config_cache", &self.config_cache)
            .finish_non_exhaustive() // Indicates we're not showing all fields
    }
}

impl RgitCore {
    /// Create a new RgitCore instance by discovering the repository
    pub fn new(verbose: bool) -> Result<Self> {
        let repo = Repository::discover(".")
            .context("Not in a git repository. Use 'rgit init' to create one.")?;
        
        let repo_path = repo.workdir()
            .ok_or_else(|| RgitError::NotInRepository)?
            .to_path_buf();

        let mut core = RgitCore {
            repo,
            repo_path,
            verbose,
            config_cache: HashMap::new(),
        };

        // Cache frequently used configuration values
        core.cache_config()?;

        Ok(core)
    }

    /// Create RgitCore from an existing repository path
    pub fn from_path<P: AsRef<Path>>(path: P, verbose: bool) -> Result<Self> {
        let repo = Repository::open(path.as_ref())
            .context("Failed to open repository")?;
        
        let repo_path = repo.workdir()
            .ok_or_else(|| RgitError::NotInRepository)?
            .to_path_buf();

        let mut core = RgitCore {
            repo,
            repo_path,
            verbose,
            config_cache: HashMap::new(),
        };

        core.cache_config()?;
        Ok(core)
    }

    /// Log a message if verbose mode is enabled
    pub fn log(&self, message: &str) {
        if self.verbose {
            println!("{} {}", "🔍".blue(), message.dimmed());
        }
        debug!("{}", message);
    }

    /// Print success message
    pub fn success(&self, message: &str) {
        println!("{} {}", "✅".green(), message);
        info!("Success: {}", message);
    }

    /// Print warning message
    pub fn warning(&self, message: &str) {
        println!("{} {}", "⚠️".yellow(), message.yellow());
        warn!("{}", message);
    }

    /// Print error message
    pub fn error(&self, message: &str) {
        println!("{} {}", "❌".red(), message.red());
    }

    /// Print info message with icon
    pub fn info(&self, message: &str) {
        println!("{} {}", "ℹ️".blue(), message);
    }

    // =========================================================================
    // Repository Information
    // =========================================================================

    /// Get the current branch name
    pub fn current_branch(&self) -> Result<String> {
        let head = self.repo.head()
            .context("Failed to get HEAD reference")?;
        
        if head.is_branch() {
            Ok(head.shorthand().unwrap_or("HEAD").to_string())
        } else {
            Ok("HEAD (detached)".to_string())
        }
    }

    /// Get repository status with enhanced information
    pub fn status(&self) -> Result<RepositoryStatus> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        opts.include_ignored(false);
        
        let statuses = self.repo.statuses(Some(&mut opts))?;
        let mut status = RepositoryStatus::default();

        // Process each file status
        for entry in statuses.iter() {
            let file_status = entry.status();
            let path = entry.path().unwrap_or("???").to_string();
            
            let file_info = FileStatus {
                path: path.clone(),
                status: file_status,
                size: std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
                modified_time: std::fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .ok(),
            };

            if file_status.contains(Status::INDEX_NEW) || 
               file_status.contains(Status::INDEX_MODIFIED) ||
               file_status.contains(Status::INDEX_DELETED) {
                status.staged.push(file_info);
            } else if file_status.contains(Status::WT_MODIFIED) ||
                      file_status.contains(Status::WT_DELETED) {
                status.unstaged.push(file_info);
            } else if file_status.contains(Status::WT_NEW) {
                status.untracked.push(file_info);
            }
        }

        // Get branch tracking information
        status.branch_info = self.get_branch_info()?;

        Ok(status)
    }

    /// Get detailed branch information including upstream tracking
    pub fn get_branch_info(&self) -> Result<BranchInfo> {
        let head = self.repo.head()?;
        let branch_name = if head.is_branch() {
            head.shorthand().unwrap_or("HEAD").to_string()
        } else {
            return Ok(BranchInfo {
                name: "HEAD (detached)".to_string(),
                ..Default::default()
            });
        };

        let mut info = BranchInfo {
            name: branch_name.clone(),
            ..Default::default()
        };

        // Get upstream information if available
        if let Ok(branch) = self.repo.find_branch(&branch_name, BranchType::Local) {
            if let Ok(upstream) = branch.upstream() {
                let upstream_name = upstream.name()?.unwrap_or("unknown").to_string();
                info.upstream = Some(upstream_name.clone());

                // Calculate ahead/behind commits
                if let (Some(local_oid), Some(upstream_oid)) = (
                    head.target(),
                    upstream.get().target()  // FIX: This was the issue at line 219
                ) {
                    if let Ok((ahead, behind)) = self.repo.graph_ahead_behind(local_oid, upstream_oid) {
                        info.ahead = ahead;
                        info.behind = behind;
                    }
                }
            }
        }

        Ok(info)
    }

    /// List all local branches
    pub fn list_branches(&self) -> Result<Vec<BranchInfo>> {
        let branches = self.repo.branches(Some(BranchType::Local))?;
        let current_branch = self.current_branch().unwrap_or_default();
        let mut branch_list = Vec::new();

        for branch_result in branches {
            let (branch, _) = branch_result?;
            let name = branch.name()?.unwrap_or("???").to_string();
            
            let mut info = BranchInfo {
                name: name.clone(),
                is_current: name == current_branch,
                ..Default::default()
            };

            // Get last commit info
            let reference = branch.get();
            if let Some(oid) = reference.target() {
                if let Ok(commit) = self.repo.find_commit(oid) {
                    info.last_commit = Some(CommitInfo {
                        oid: oid.to_string(),
                        message: commit.message().unwrap_or("").to_string(),
                        author: commit.author().name().unwrap_or("Unknown").to_string(),
                        time: commit.time(),
                    });
                }
            }

            // Get upstream info
            if let Ok(upstream) = branch.upstream() {
                info.upstream = upstream.name()?.map(|s| s.to_string());
            }

            branch_list.push(info);
        }

        Ok(branch_list)
    }

    // =========================================================================
    // Index Operations
    // =========================================================================

    /// Add files to the staging area
    pub fn add_files(&mut self, paths: &[impl AsRef<Path>]) -> Result<()> {
        let mut index = self.repo.index()?;
        
        for path in paths {
            let path_ref = path.as_ref();
            self.log(&format!("Adding file: {}", path_ref.display()));
            
            if path_ref.exists() {
                index.add_path(path_ref)
                    .with_context(|| format!("Failed to add file: {}", path_ref.display()))?;
            } else {
                return Err(RgitError::FileNotFound(path_ref.to_path_buf()).into());
            }
        }
        
        index.write()?;
        Ok(())
    }

    /// Add all changes to the staging area
    pub fn add_all(&mut self) -> Result<()> {
        let mut index = self.repo.index()?;
        self.log("Adding all changes...");
        
        index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
        index.write()?;
        
        Ok(())
    }

    /// Update only tracked files in the staging area
    pub fn add_update(&mut self) -> Result<()> {
        let mut index = self.repo.index()?;
        self.log("Updating tracked files...");
        
        index.update_all(["*"].iter(), None)?;
        index.write()?;
        
        Ok(())
    }

    // =========================================================================
    // Commit Operations
    // =========================================================================

    /// Create a commit with the given message
    pub fn commit(&self, message: &str, amend: bool) -> Result<Oid> {
        if message.trim().is_empty() {
            return Err(RgitError::EmptyCommitMessage.into());
        }

        let signature = self.get_signature()?;
        let mut index = self.repo.index()?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        let commit_id = if amend {
            self.log("Amending previous commit...");
            let head_commit = self.repo.head()?.peel_to_commit()?;
            let parents: Vec<Commit> = head_commit.parents().collect();
            let parent_refs: Vec<&Commit> = parents.iter().collect();
            
            self.repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parent_refs,
            )?
        } else {
            self.log("Creating new commit...");
            let parent_commit = if let Ok(head) = self.repo.head() {
                Some(head.peel_to_commit()?)
            } else {
                None
            };

            let parents = if let Some(ref commit) = parent_commit {
                vec![commit]
            } else {
                vec![]
            };

            self.repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )?
        };

        Ok(commit_id)
    }

    /// Get or create a signature for commits
    pub fn get_signature(&self) -> Result<Signature> {
        // Try to get from cache first
        if let (Some(name), Some(email)) = (
            self.config_cache.get("user.name"),
            self.config_cache.get("user.email")
        ) {
            return Ok(Signature::now(name, email)?);
        }

        // Fall back to repository config
        let config = self.repo.config()?;
        let name = config.get_string("user.name")
            .context("user.name not configured. Run 'git config user.name \"Your Name\"'")?;
        let email = config.get_string("user.email")
            .context("user.email not configured. Run 'git config user.email \"your@email.com\"'")?;

        Ok(Signature::now(&name, &email)?)
    }

    // =========================================================================
    // Remote Operations
    // =========================================================================

    /// Get the default remote (usually 'origin')
    pub fn get_default_remote(&self) -> Result<String> {
        let remotes = self.repo.remotes()?;
        
        // Look for 'origin' first
        if remotes.iter().any(|r| r == Some("origin")) {
            return Ok("origin".to_string());
        }
        
        // Otherwise return the first remote
        remotes.get(0)
            .and_then(|r| Some(r))
            .map(|s| s.to_string())
            .ok_or_else(|| RgitError::NoRemoteConfigured.into())
    }

    /// List all configured remotes
    pub fn list_remotes(&self) -> Result<Vec<RemoteInfo>> {
        let remotes = self.repo.remotes()?;
        let mut remote_list = Vec::new();

        for remote_name in remotes.iter() {
            if let Some(name) = remote_name {
                if let Ok(remote) = self.repo.find_remote(name) {
                    let info = RemoteInfo {
                        name: name.to_string(),
                        url: remote.url().unwrap_or("").to_string(),
                        // FIX: Wrap in Some() to match Option<String> type
                        push_url: remote.pushurl().map(|s| s.to_string()),
                    };
                    remote_list.push(info);
                }
            }
        }

        Ok(remote_list)
    }

    // =========================================================================
    // Utility Methods
    // =========================================================================

    /// Cache frequently used configuration values
    fn cache_config(&mut self) -> Result<()> {
        let config = self.repo.config()?;
        
        // Cache user information
        if let Ok(name) = config.get_string("user.name") {
            self.config_cache.insert("user.name".to_string(), name);
        }
        if let Ok(email) = config.get_string("user.email") {
            self.config_cache.insert("user.email".to_string(), email);
        }

        Ok(())
    }

    /// Check if the repository is in a clean state
    pub fn is_clean(&self) -> Result<bool> {
        let statuses = self.repo.statuses(None)?;
        Ok(statuses.is_empty())
    }

    /// Check if there are staged changes
    pub fn has_staged_changes(&self) -> Result<bool> {
        let statuses = self.repo.statuses(None)?;
        
        for entry in statuses.iter() {
            let status = entry.status();
            if status.contains(Status::INDEX_NEW) ||
               status.contains(Status::INDEX_MODIFIED) ||
               status.contains(Status::INDEX_DELETED) {
                return Ok(true);
            }
        }
        
        Ok(false)
    }

    /// Get the repository's root directory
    pub fn root_dir(&self) -> &Path {
        &self.repo_path
    }

    /// Get the .git directory
    pub fn git_dir(&self) -> &Path {
        self.repo.path()
    }
}

// =============================================================================
// Data Structures
// =============================================================================

#[derive(Debug, Default)]
pub struct RepositoryStatus {
    pub staged: Vec<FileStatus>,
    pub unstaged: Vec<FileStatus>,
    pub untracked: Vec<FileStatus>,
    pub branch_info: BranchInfo,
}

#[derive(Debug, Clone)]
pub struct FileStatus {
    pub path: String,
    pub status: Status,
    pub size: u64,
    pub modified_time: Option<std::time::SystemTime>,
}

#[derive(Debug, Default, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub is_current: bool,
    pub last_commit: Option<CommitInfo>,
}

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub oid: String,
    pub message: String,
    pub author: String,
    pub time: Time,
}

#[derive(Debug)]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
    pub push_url: Option<String>,
}

impl RepositoryStatus {
    pub fn is_clean(&self) -> bool {
        self.staged.is_empty() && self.unstaged.is_empty() && self.untracked.is_empty()
    }

    pub fn total_changes(&self) -> usize {
        self.staged.len() + self.unstaged.len() + self.untracked.len()
    }
}

impl FileStatus {
    pub fn status_symbol(&self, staged: bool) -> &'static str {
        if staged {
            if self.status.contains(Status::INDEX_NEW) { "new file" }
            else if self.status.contains(Status::INDEX_MODIFIED) { "modified" }
            else if self.status.contains(Status::INDEX_DELETED) { "deleted" }
            else if self.status.contains(Status::INDEX_RENAMED) { "renamed" }
            else if self.status.contains(Status::INDEX_TYPECHANGE) { "typechange" }
            else { "changed" }
        } else {
            if self.status.contains(Status::WT_MODIFIED) { "modified" }
            else if self.status.contains(Status::WT_DELETED) { "deleted" }
            else if self.status.contains(Status::WT_RENAMED) { "renamed" }
            else if self.status.contains(Status::WT_TYPECHANGE) { "typechange" }
            else if self.status.contains(Status::WT_NEW) { "untracked" }
            else { "changed" }
        }
    }

    pub fn format_size(&self) -> String {
        if self.size < 1024 {
            format!("{}B", self.size)
        } else if self.size < 1024 * 1024 {
            format!("{:.1}KB", self.size as f64 / 1024.0)
        } else if self.size < 1024 * 1024 * 1024 {
            format!("{:.1}MB", self.size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1}GB", self.size as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

impl BranchInfo {
    pub fn format_tracking_info(&self) -> String {
        match (&self.upstream, self.ahead, self.behind) {
            (Some(upstream), 0, 0) => format!("up to date with {}", upstream.cyan()),
            (Some(upstream), ahead, 0) if ahead > 0 => {
                format!("{} commits ahead of {}", ahead.to_string().green(), upstream.cyan())
            }
            (Some(upstream), 0, behind) if behind > 0 => {
                format!("{} commits behind {}", behind.to_string().red(), upstream.cyan())
            }
            (Some(upstream), ahead, behind) if ahead > 0 && behind > 0 => {
                format!("{} ahead, {} behind {}", 
                       ahead.to_string().green(), 
                       behind.to_string().red(), 
                       upstream.cyan())
            }
            (Some(upstream), _, _) => format!("tracking {}", upstream.cyan()),
            (None, _, _) => "no upstream".dimmed().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, Repository) {
        let temp_dir = TempDir::new().unwrap();
        let repo = Repository::init(temp_dir.path()).unwrap();
        (temp_dir, repo)
    }

    #[test]
    fn test_rgit_core_creation() {
        let (_temp_dir, _repo) = create_test_repo();
        // Test repository creation and basic operations
    }

    #[test]
    fn test_status_calculation() {
        let (_temp_dir, _repo) = create_test_repo();
        // Test status calculation with various file states
    }

    #[test]
    fn test_branch_info() {
        let (_temp_dir, _repo) = create_test_repo();
        // Test branch information retrieval
    }
}