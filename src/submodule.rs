use anyhow::{Context, Result};
use colored::*;
use git2::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::core::RgitCore;
use crate::error::RgitError;
use crate::interactive::InteractivePrompt;
use crate::config::Config;

/// Intelligent submodule manager with proactive health checking
pub struct SubmoduleManager<'a> {
    pub rgit: &'a RgitCore,
    pub config: &'a Config,
}

impl<'a> SubmoduleManager<'a> {
    /// Create a new submodule manager
    pub fn new(rgit: &'a RgitCore, config: &'a Config) -> Self {
        Self { rgit, config }
    }

    /// Perform comprehensive submodule health check
    pub fn check_health(&self) -> Result<SubmoduleHealth> {
        debug!("Checking submodule health");
        let mut health = SubmoduleHealth::default();
        
        let submodules = self.rgit.repo.submodules()
            .context("Failed to get submodules")?;

        for submodule in &submodules {
            let name = submodule.name().unwrap_or("unknown").to_string();
            let path = submodule.path().to_path_buf();
            
            debug!("Checking submodule: {}", name);
            
            let status = self.check_submodule_status(submodule)?;
            health.add_submodule(name, status);
        }

        Ok(health)
    }

    /// Check individual submodule status
    fn check_submodule_status(&self, submodule: &Submodule) -> Result<SubmoduleStatus> {
        let name = submodule.name().unwrap_or("unknown");
        let mut status = SubmoduleStatus {
            name: name.to_string(),
            path: submodule.path().to_path_buf(),
            url: submodule.url().map(|s| s.to_string()),
            branch: submodule.branch().map(|s| s.to_string()),
            ..Default::default()
        };

        // Check if submodule is initialized
        match submodule.open() {
            Ok(sub_repo) => {
                status.initialized = true;
                status.issues.extend(self.check_submodule_repo(&sub_repo, submodule)?);
            }
            Err(_) => {
                status.initialized = false;
                status.issues.push(SubmoduleIssue::NotInitialized);
            }
        }

        // Check if submodule directory exists but is empty
        if status.path.exists() && !status.initialized {
            if self.is_directory_empty(&status.path)? {
                status.issues.push(SubmoduleIssue::EmptyDirectory);
            } else {
                status.issues.push(SubmoduleIssue::DirectoryNotEmpty);
            }
        }

        // Check URL validity
        if let Some(ref url) = status.url {
            if !self.is_valid_url(url) {
                status.issues.push(SubmoduleIssue::InvalidUrl(url.clone()));
            }
        }

        Ok(status)
    }

    /// Check submodule repository for issues
    fn check_submodule_repo(&self, sub_repo: &Repository, submodule: &Submodule) -> Result<Vec<SubmoduleIssue>> {
        let mut issues = Vec::new();

        // Check for uncommitted changes
        if self.has_uncommitted_changes(sub_repo)? {
            issues.push(SubmoduleIssue::UncommittedChanges);
        }

        // Check if HEAD is detached
        if self.is_detached_head(sub_repo)? {
            issues.push(SubmoduleIssue::DetachedHead);
        }

        // Check if submodule is ahead/behind remote
        if let Ok((ahead, behind)) = self.get_ahead_behind_count(sub_repo, submodule) {
            if ahead > 0 {
                issues.push(SubmoduleIssue::AheadOfRemote(ahead));
            }
            if behind > 0 {
                issues.push(SubmoduleIssue::BehindRemote(behind));
            }
        }

        // Check for merge conflicts
        if self.has_merge_conflicts(sub_repo)? {
            issues.push(SubmoduleIssue::MergeConflicts);
        }

        Ok(issues)
    }

    /// Interactive submodule health check with user prompts
    pub fn interactive_health_check(&self) -> Result<bool> {
        if !self.config.submodules.health_check {
            return Ok(true);
        }

        let health = self.check_health()?;
        
        if health.is_healthy() {
            if self.rgit.verbose {
                self.rgit.success("All submodules are healthy");
            }
            return Ok(true);
        }

        self.display_health_summary(&health)?;
        
        if !self.config.is_interactive() {
            warn!("Submodule issues detected but running in non-interactive mode");
            return Ok(false);
        }

        let options = vec![
            "Continue anyway",
            "Fix issues automatically",
            "Show detailed status",
            "Abort operation",
        ];

        let prompt = InteractivePrompt::new()
            .with_message("Submodule issues detected. How would you like to proceed?")
            .with_options(&options)
            .with_default(1);

        match prompt.select()? {
            0 => Ok(true), // Continue anyway
            1 => {
                self.auto_fix_issues(&health)?;
                // Recheck after fixes
                let new_health = self.check_health()?;
                if new_health.is_healthy() {
                    self.rgit.success("All submodule issues resolved");
                    Ok(true)
                } else {
                    self.rgit.warning("Some issues remain unresolved");
                    Ok(false)
                }
            }
            2 => {
                self.display_detailed_status(&health)?;
                Ok(false)
            }
            _ => Ok(false), // Abort
        }
    }

    /// Display health summary
    fn display_health_summary(&self, health: &SubmoduleHealth) -> Result<()> {
        self.rgit.warning("Submodule health issues detected:");
        
        for (name, status) in &health.submodules {
            if !status.issues.is_empty() {
                println!("  {} {} ({} issues)", 
                        "📦".red(), 
                        name.yellow(), 
                        status.issues.len().to_string().red());
                
                for issue in &status.issues {
                    println!("    {} {}", "•".red(), issue.description());
                }
            }
        }
        
        println!();
        Ok(())
    }

    /// Display detailed submodule status
    fn display_detailed_status(&self, health: &SubmoduleHealth) -> Result<()> {
        println!("\n{} Detailed Submodule Status:", "📋".blue().bold());
        
        for (name, status) in &health.submodules {
            self.display_submodule_details(name, status)?;
        }
        
        Ok(())
    }

    /// Display details for a single submodule
    fn display_submodule_details(&self, name: &str, status: &SubmoduleStatus) -> Result<()> {
        println!("\n🗂️  {} {}", name.cyan().bold(), 
                format!("({})", status.path.display()).dimmed());
        
        if let Some(ref url) = status.url {
            println!("   {} {}", "🔗".blue(), url.cyan());
        }
        
        if let Some(ref branch) = status.branch {
            println!("   {} {}", "🌿".green(), branch.green());
        }
        
        if status.initialized {
            println!("   {} Initialized", "✅".green());
        } else {
            println!("   {} Not initialized", "❓".red());
        }
        
        if status.issues.is_empty() {
            println!("   {} No issues detected", "🎉".green());
        } else {
            println!("   {} Issues:", "⚠️".yellow());
            for issue in &status.issues {
                println!("     {} {}", "•".red(), issue.description());
                
                // Show suggested fixes
                for suggestion in issue.suggestions() {
                    println!("       {} {}", "💡".yellow(), suggestion.dimmed());
                }
            }
        }
        
        Ok(())
    }

    /// Automatically fix common submodule issues
    pub fn auto_fix_issues(&self, health: &SubmoduleHealth) -> Result<()> {
        info!("Attempting to auto-fix submodule issues");
        
        for (name, status) in &health.submodules {
            if status.issues.is_empty() {
                continue;
            }
            
            self.rgit.log(&format!("Fixing issues in submodule: {}", name));
            
            for issue in &status.issues {
                match self.fix_issue(name, issue) {
                    Ok(()) => self.rgit.success(&format!("Fixed: {}", issue.description())),
                    Err(e) => self.rgit.warning(&format!("Could not fix {}: {}", issue.description(), e)),
                }
            }
        }
        
        Ok(())
    }

    /// Fix a specific submodule issue
    fn fix_issue(&self, name: &str, issue: &SubmoduleIssue) -> Result<()> {
        match issue {
            SubmoduleIssue::NotInitialized => {
                self.init_submodule(name)?;
            }
            SubmoduleIssue::UncommittedChanges => {
                if self.config.submodules.auto_stash {
                    self.stash_submodule_changes(name)?;
                } else {
                    return Err(RgitError::SubmoduleUncommittedChanges(name.to_string()).into());
                }
            }
            SubmoduleIssue::EmptyDirectory => {
                self.remove_empty_directory(name)?;
                self.init_submodule(name)?;
            }
            SubmoduleIssue::DirectoryNotEmpty => {
                // This requires manual intervention
                return Err(RgitError::SubmoduleOperationFailed(
                    format!("Directory {} is not empty and not a git repository", name)
                ).into());
            }
            SubmoduleIssue::InvalidUrl(_) => {
                // URL issues require manual configuration
                return Err(RgitError::SubmoduleOperationFailed(
                    "Invalid URL requires manual configuration".to_string()
                ).into());
            }
            _ => {
                // Other issues may not be auto-fixable
                return Err(RgitError::SubmoduleOperationFailed(
                    format!("Cannot auto-fix: {}", issue.description())
                ).into());
            }
        }
        
        Ok(())
    }

    /// Initialize a specific submodule
    fn init_submodule(&self, name: &str) -> Result<()> {
        debug!("Initializing submodule: {}", name);
        
        let mut submodule = self.rgit.repo.find_submodule(name)
            .with_context(|| format!("Submodule not found: {}", name))?;
        
        submodule.init(false)
            .with_context(|| format!("Failed to initialize submodule: {}", name))?;
        
        Ok(())
    }

    /// Stash changes in a submodule
    fn stash_submodule_changes(&self, name: &str) -> Result<()> {
        debug!("Stashing changes in submodule: {}", name);
        
        let submodule = self.rgit.repo.find_submodule(name)
            .with_context(|| format!("Submodule not found: {}", name))?;
        
        let sub_repo = submodule.open()
            .with_context(|| format!("Failed to open submodule: {}", name))?;
        
        let signature = sub_repo.signature()
            .context("Failed to get signature for stash")?;
        
        sub_repo.stash_save(&signature, &format!("rgit auto-stash for {}", name), None)
            .with_context(|| format!("Failed to stash changes in submodule: {}", name))?;
        
        Ok(())
    }

    /// Remove empty directory
    fn remove_empty_directory(&self, name: &str) -> Result<()> {
        let submodule = self.rgit.repo.find_submodule(name)?;
        let path = submodule.path();
        
        if path.exists() && self.is_directory_empty(path)? {
            std::fs::remove_dir(path)
                .with_context(|| format!("Failed to remove empty directory: {}", path.display()))?;
        }
        
        Ok(())
    }

    // =========================================================================
    // Utility Methods
    // =========================================================================

    /// Check if submodule has uncommitted changes
    pub fn has_uncommitted_changes(&self, repo: &Repository) -> Result<bool> {
        let statuses = repo.statuses(None)?;
        Ok(!statuses.is_empty())
    }

    /// Check if repository has detached HEAD
    pub fn is_detached_head(&self, repo: &Repository) -> Result<bool> {
        match repo.head() {
            Ok(head) => Ok(!head.is_branch()),
            Err(_) => Ok(true), // Assume detached if we can't get HEAD
        }
    }

    /// Get ahead/behind count for submodule
    pub fn get_ahead_behind_count(&self, sub_repo: &Repository, submodule: &Submodule) -> Result<(usize, usize)> {
        let head = sub_repo.head()?;
        let local_oid = head.target().context("No target for HEAD")?;
        
        // Try to get upstream reference
        if let Ok(branch) = sub_repo.find_branch(&head.shorthand().unwrap_or("HEAD"), BranchType::Local) {
            if let Ok(upstream) = branch.upstream() {
                if let Some(upstream_oid) = upstream.get().target() {
                    return sub_repo.graph_ahead_behind(local_oid, upstream_oid)
                        .map_err(|e| e.into());
                }
            }
        }
        
        Ok((0, 0))
    }

    /// Check if repository has merge conflicts
    fn has_merge_conflicts(&self, repo: &Repository) -> Result<bool> {
        match repo.state() {
            RepositoryState::Merge => Ok(true),
            RepositoryState::Revert => Ok(true),
            RepositoryState::CherryPick => Ok(true),
            RepositoryState::Bisect => Ok(true),
            RepositoryState::Rebase | RepositoryState::RebaseInteractive | RepositoryState::RebaseMerge => Ok(true),
            _ => Ok(false),
        }
    }

    /// Check if directory is empty
    fn is_directory_empty(&self, path: &Path) -> Result<bool> {
        if !path.is_dir() {
            return Ok(false);
        }
        
        let entries = std::fs::read_dir(path)?;
        Ok(entries.count() == 0)
    }

    /// Validate URL format
    fn is_valid_url(&self, url: &str) -> bool {
        // Basic URL validation - could be more sophisticated
        url.starts_with("http://") || 
        url.starts_with("https://") || 
        url.starts_with("git://") || 
        url.starts_with("ssh://") ||
        url.contains("@") && url.contains(":")
    }

    /// Update all submodules
    pub fn update_all(&self, recursive: bool, init: bool) -> Result<()> {
        info!("Updating all submodules (recursive: {}, init: {})", recursive, init);
        
        let submodules = self.rgit.repo.submodules()?;
        
        for mut submodule in submodules {
            let name = submodule.name().unwrap_or("unknown");
            self.rgit.log(&format!("Updating submodule: {}", name));
            
            if init && !submodule.open().is_ok() {
                submodule.init(false)?;
            }
            
            submodule.update(init, None)?;
            
            if recursive {
                // Recursively update nested submodules
                if let Ok(sub_repo) = submodule.open() {
                    let sub_manager = SubmoduleManager {
                        rgit: &RgitCore::from_path(sub_repo.workdir().unwrap(), self.rgit.verbose)?,
                        config: self.config,
                    };
                    sub_manager.update_all(true, init)?;
                }
            }
        }
        
        Ok(())
    }

    /// Execute command in all submodules
    pub fn foreach<F>(&self, recursive: bool, mut command: F) -> Result<()>
    where
        F: FnMut(&str, &Path) -> Result<()>,
    {
        let submodules = self.rgit.repo.submodules()?;
        
        for submodule in submodules {
            let name = submodule.name().unwrap_or("unknown");
            let path = submodule.path();
            
            if path.exists() {
                command(name, path)?;
                
                if recursive {
                    if let Ok(sub_repo) = submodule.open() {
                        let sub_manager = SubmoduleManager {
                            rgit: &RgitCore::from_path(sub_repo.workdir().unwrap(), self.rgit.verbose)?,
                            config: self.config,
                        };
                        sub_manager.foreach(true, &mut command)?;
                    }
                }
            }
        }
        
        Ok(())
    }
}

// =============================================================================
// Data Structures
// =============================================================================

#[derive(Debug, Default)]
pub struct SubmoduleHealth {
    pub submodules: HashMap<String, SubmoduleStatus>,
}

impl SubmoduleHealth {
    pub fn add_submodule(&mut self, name: String, status: SubmoduleStatus) {
        self.submodules.insert(name, status);
    }

    pub fn is_healthy(&self) -> bool {
        self.submodules.values().all(|status| status.issues.is_empty())
    }

    pub fn total_issues(&self) -> usize {
        self.submodules.values().map(|status| status.issues.len()).sum()
    }

    pub fn unhealthy_submodules(&self) -> Vec<&String> {
        self.submodules.iter()
            .filter(|(_, status)| !status.issues.is_empty())
            .map(|(name, _)| name)
            .collect()
    }
}

#[derive(Debug, Default)]
pub struct SubmoduleStatus {
    pub name: String,
    pub path: PathBuf,
    pub url: Option<String>,
    pub branch: Option<String>,
    pub initialized: bool,
    pub issues: Vec<SubmoduleIssue>,
}

#[derive(Debug, Clone)]
pub enum SubmoduleIssue {
    NotInitialized,
    UncommittedChanges,
    DetachedHead,
    AheadOfRemote(usize),
    BehindRemote(usize),
    MergeConflicts,
    EmptyDirectory,
    DirectoryNotEmpty,
    InvalidUrl(String),
    MissingRemote,
    NetworkError(String),
}

impl SubmoduleIssue {
    pub fn description(&self) -> String {
        match self {
            SubmoduleIssue::NotInitialized => "Not initialized".to_string(),
            SubmoduleIssue::UncommittedChanges => "Has uncommitted changes".to_string(),
            SubmoduleIssue::DetachedHead => "Detached HEAD state".to_string(),
            SubmoduleIssue::AheadOfRemote(n) => format!("{} commits ahead of remote", n),
            SubmoduleIssue::BehindRemote(n) => format!("{} commits behind remote", n),
            SubmoduleIssue::MergeConflicts => "Has merge conflicts".to_string(),
            SubmoduleIssue::EmptyDirectory => "Directory is empty".to_string(),
            SubmoduleIssue::DirectoryNotEmpty => "Directory exists but is not a git repository".to_string(),
            SubmoduleIssue::InvalidUrl(url) => format!("Invalid URL: {}", url),
            SubmoduleIssue::MissingRemote => "No remote configured".to_string(),
            SubmoduleIssue::NetworkError(msg) => format!("Network error: {}", msg),
        }
    }

    pub fn suggestions(&self) -> Vec<String> {
        match self {
            SubmoduleIssue::NotInitialized => vec![
                "Run 'rgit submodule init'".to_string(),
            ],
            SubmoduleIssue::UncommittedChanges => vec![
                "Commit changes in submodule".to_string(),
                "Stash changes with 'git stash'".to_string(),
            ],
            SubmoduleIssue::DetachedHead => vec![
                "Checkout a branch in the submodule".to_string(),
                "Create a new branch from current state".to_string(),
            ],
            SubmoduleIssue::AheadOfRemote(_) => vec![
                "Push changes to remote".to_string(),
                "Update parent repository reference".to_string(),
            ],
            SubmoduleIssue::BehindRemote(_) => vec![
                "Pull latest changes".to_string(),
                "Update submodule with 'rgit submodule update'".to_string(),
            ],
            SubmoduleIssue::MergeConflicts => vec![
                "Resolve conflicts in submodule".to_string(),
                "Use 'rgit resolve' for assistance".to_string(),
            ],
            SubmoduleIssue::EmptyDirectory => vec![
                "Remove directory and reinitialize".to_string(),
            ],
            SubmoduleIssue::DirectoryNotEmpty => vec![
                "Back up directory contents".to_string(),
                "Remove directory and reinitialize submodule".to_string(),
            ],
            SubmoduleIssue::InvalidUrl(_) => vec![
                "Update .gitmodules with correct URL".to_string(),
                "Run 'rgit submodule sync'".to_string(),
            ],
            SubmoduleIssue::MissingRemote => vec![
                "Add remote to submodule".to_string(),
                "Check .gitmodules configuration".to_string(),
            ],
            SubmoduleIssue::NetworkError(_) => vec![
                "Check internet connection".to_string(),
                "Verify remote repository access".to_string(),
            ],
        }
    }

    pub fn severity(&self) -> IssueSeverity {
        match self {
            SubmoduleIssue::NotInitialized => IssueSeverity::Warning,
            SubmoduleIssue::UncommittedChanges => IssueSeverity::Warning,
            SubmoduleIssue::DetachedHead => IssueSeverity::Info,
            SubmoduleIssue::AheadOfRemote(_) => IssueSeverity::Info,
            SubmoduleIssue::BehindRemote(_) => IssueSeverity::Warning,
            SubmoduleIssue::MergeConflicts => IssueSeverity::Error,
            SubmoduleIssue::EmptyDirectory => IssueSeverity::Warning,
            SubmoduleIssue::DirectoryNotEmpty => IssueSeverity::Error,
            SubmoduleIssue::InvalidUrl(_) => IssueSeverity::Error,
            SubmoduleIssue::MissingRemote => IssueSeverity::Warning,
            SubmoduleIssue::NetworkError(_) => IssueSeverity::Warning,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
}

impl IssueSeverity {
    pub fn icon(&self) -> &'static str {
        match self {
            IssueSeverity::Info => "ℹ️",
            IssueSeverity::Warning => "⚠️",
            IssueSeverity::Error => "❌",
        }
    }

    pub fn color(&self) -> colored::Color {
        match self {
            IssueSeverity::Info => colored::Color::Blue,
            IssueSeverity::Warning => colored::Color::Yellow,
            IssueSeverity::Error => colored::Color::Red,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_repo_with_submodule() -> (TempDir, Repository) {
        let temp_dir = TempDir::new().unwrap();
        let repo = Repository::init(temp_dir.path()).unwrap();
        
        // This would normally add a real submodule, but for testing
        // we just create the basic structure
        
        (temp_dir, repo)
    }

    #[test]
    fn test_submodule_health_default() {
        let health = SubmoduleHealth::default();
        assert!(health.is_healthy());
        assert_eq!(health.total_issues(), 0);
    }

    #[test]
    fn test_submodule_issue_descriptions() {
        let issue = SubmoduleIssue::NotInitialized;
        assert_eq!(issue.description(), "Not initialized");
        
        let suggestions = issue.suggestions();
        assert!(!suggestions.is_empty());
        assert!(suggestions[0].contains("init"));
    }

    #[test]
    fn test_issue_severity() {
        assert_eq!(SubmoduleIssue::NotInitialized.severity(), IssueSeverity::Warning);
        assert_eq!(SubmoduleIssue::MergeConflicts.severity(), IssueSeverity::Error);
        assert_eq!(SubmoduleIssue::AheadOfRemote(1).severity(), IssueSeverity::Info);
    }

    #[test]
    fn test_severity_properties() {
        let warning = IssueSeverity::Warning;
        assert_eq!(warning.icon(), "⚠️");
        assert_eq!(warning.color(), colored::Color::Yellow);
    }
}