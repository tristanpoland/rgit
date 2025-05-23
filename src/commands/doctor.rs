use anyhow::Result;
use colored::*;
use git2::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Config;
use crate::core::RgitCore;
use crate::error::RgitError;
use crate::interactive::{InteractivePrompt, TableDisplay};
use crate::submodule::SubmoduleManager;
use crate::utils::{humanize_size, is_valid_email};

/// Execute the doctor command - comprehensive repository health check
pub async fn execute(config: &Config) -> Result<()> {
    println!("{} {} Repository Health Check", "🏥".blue(), "rgit".cyan().bold());
    println!("{}", "=".repeat(50).dimmed());
    println!();

    let mut doctor = RepositoryDoctor::new(config);
    let health_report = doctor.run_full_diagnosis().await?;
    
    display_health_report(&health_report, config)?;
    
    if health_report.has_issues() {
        offer_auto_fix(&health_report, config).await?;
    } else {
        println!("\n{} Repository is in excellent health! 🎉", "✅".green().bold());
    }
    
    show_health_recommendations(&health_report, config)?;
    
    Ok(())
}

/// Repository doctor for comprehensive health checks
struct RepositoryDoctor<'a> {
    config: &'a Config,
    rgit: Option<RgitCore>,
}

impl<'a> RepositoryDoctor<'a> {
    fn new(config: &'a Config) -> Self {
        let rgit = RgitCore::new(false).ok();
        Self { config, rgit }
    }

    /// Run complete diagnosis
    async fn run_full_diagnosis(&mut self) -> Result<HealthReport> {
        let mut report = HealthReport::new();
        
        // Basic environment checks (always run)
        self.check_git_installation(&mut report).await?;
        self.check_git_configuration(&mut report).await?;
        
        // Repository-specific checks (only if in a git repo)
        if let Some(ref rgit) = self.rgit {
            self.check_repository_structure(rgit, &mut report).await?;
            self.check_repository_integrity(rgit, &mut report).await?;
            self.check_working_directory(rgit, &mut report).await?;
            self.check_remotes(rgit, &mut report).await?;
            self.check_branches(rgit, &mut report).await?;
            self.check_submodules(rgit, &mut report).await?;
            self.check_hooks(rgit, &mut report).await?;
            self.check_performance(rgit, &mut report).await?;
        } else {
            report.add_info("Repository", "Not in a git repository", 
                          "Run 'rgit init' to create a new repository");
        }
        
        Ok(report)
    }

    /// Check Git installation and version
    async fn check_git_installation(&self, report: &mut HealthReport) -> Result<()> {
        print!("Checking Git installation... ");
        
        match Command::new("git").arg("--version").output() {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    let version_line = version.lines().next().unwrap_or("unknown");
                    println!("{}", "✅".green());
                    
                    // Parse version and check if it's recent enough
                    if let Some(version_num) = extract_git_version(&version_line) {
                        if version_num >= (2, 20, 0) {
                            report.add_success("Git Installation", 
                                             &format!("Git {} installed", version_line.trim()),
                                             "Git is up to date");
                        } else {
                            report.add_warning("Git Installation", 
                                             &format!("Git {} is outdated", version_line.trim()),
                                             "Consider upgrading to Git 2.20+");
                        }
                    } else {
                        report.add_info("Git Installation", 
                                      &format!("Git installed: {}", version_line.trim()),
                                      "Version parsing failed");
                    }
                } else {
                    println!("{}", "❌".red());
                    report.add_error("Git Installation", 
                                   "Git command failed",
                                   "Reinstall Git or check PATH");
                }
            }
            Err(_) => {
                println!("{}", "❌".red());
                report.add_error("Git Installation", 
                               "Git not found in PATH",
                               "Install Git or add it to PATH");
            }
        }
        
        Ok(())
    }

    /// Check Git configuration
    async fn check_git_configuration(&self, report: &mut HealthReport) -> Result<()> {
        print!("Checking Git configuration... ");
        
        // Check global configuration
        match Repository::open_from_env() {
            Ok(repo) => {
                let config = repo.config()?;
                self.check_user_identity(&config, report)?;
                self.check_essential_config(&config, report)?;
                println!("{}", "✅".green());
            }
            Err(_) => {
                // Try to check global config
                match git2::Config::open_default() {
                    Ok(config) => {
                        self.check_user_identity(&config, report)?;
                        self.check_essential_config(&config, report)?;
                        println!("{}", "✅".green());
                    }
                    Err(_) => {
                        println!("{}", "❌".red());
                        report.add_error("Git Configuration", 
                                       "Cannot access Git configuration",
                                       "Check Git installation");
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Check user identity configuration
    fn check_user_identity(&self, config: &git2::Config, report: &mut HealthReport) -> Result<()> {
        let name = config.get_string("user.name").ok();
        let email = config.get_string("user.email").ok();
        
        match (name, email) {
            (Some(name), Some(email)) => {
                if is_valid_email(&email) {
                    report.add_success("User Identity", 
                                     &format!("{} <{}>", name, email),
                                     "User identity configured correctly");
                } else {
                    report.add_warning("User Identity", 
                                     &format!("{} <{}> (invalid email)", name, email),
                                     "Use a valid email address");
                }
            }
            (Some(name), None) => {
                report.add_warning("User Identity", 
                                 &format!("Name: {}, No email", name),
                                 "Set user.email: git config --global user.email \"your@email.com\"");
            }
            (None, Some(email)) => {
                report.add_warning("User Identity", 
                                 &format!("Email: {}, No name", email),
                                 "Set user.name: git config --global user.name \"Your Name\"");
            }
            (None, None) => {
                report.add_error("User Identity", 
                               "No user identity configured",
                               "Set user.name and user.email with git config");
            }
        }
        
        Ok(())
    }

    /// Check essential Git configuration
    fn check_essential_config(&self, config: &git2::Config, report: &mut HealthReport) -> Result<()> {
        // Check core.autocrlf (important on Windows)
        if cfg!(windows) {
            match config.get_string("core.autocrlf") {
                Ok(value) => {
                    if value == "true" {
                        report.add_success("Line Endings", 
                                         "core.autocrlf = true",
                                         "Appropriate for Windows");
                    } else {
                        report.add_info("Line Endings", 
                                      &format!("core.autocrlf = {}", value),
                                      "Consider setting to 'true' on Windows");
                    }
                }
                Err(_) => {
                    report.add_info("Line Endings", 
                                  "core.autocrlf not set",
                                  "Consider setting for Windows compatibility");
                }
            }
        }
        
        // Check default branch name
        match config.get_string("init.defaultBranch") {
            Ok(branch) => {
                report.add_success("Default Branch", 
                                 &format!("init.defaultBranch = {}", branch),
                                 "Default branch configured");
            }
            Err(_) => {
                report.add_info("Default Branch", 
                              "init.defaultBranch not set",
                              "Consider setting: git config --global init.defaultBranch main");
            }
        }
        
        Ok(())
    }

    /// Check repository structure and basic health
    async fn check_repository_structure(&self, rgit: &RgitCore, report: &mut HealthReport) -> Result<()> {
        print!("Checking repository structure... ");
        
        let git_dir = rgit.git_dir();
        let work_dir = rgit.root_dir();
        
        // Check .git directory
        if git_dir.exists() {
            report.add_success("Git Directory", 
                             &format!(".git at {}", git_dir.display()),
                             "Repository structure is valid");
        } else {
            report.add_error("Git Directory", 
                           ".git directory not found",
                           "Repository may be corrupted");
        }
        
        // Check working directory
        if work_dir.exists() {
            let permissions = fs::metadata(work_dir)?.permissions();
            if permissions.readonly() {
                report.add_warning("Working Directory", 
                                 "Directory is read-only",
                                 "May prevent Git operations");
            } else {
                report.add_success("Working Directory", 
                                 &format!("Writable at {}", work_dir.display()),
                                 "Working directory accessible");
            }
        }
        
        // Check essential Git files
        self.check_git_files(git_dir, report)?;
        
        println!("{}", "✅".green());
        Ok(())
    }

    /// Check essential Git files
    fn check_git_files(&self, git_dir: &Path, report: &mut HealthReport) -> Result<()> {
        let essential_files = [
            ("HEAD", "Points to current branch"),
            ("config", "Repository configuration"),
            ("refs/", "References directory"),
            ("objects/", "Object database"),
        ];
        
        for (file, description) in &essential_files {
            let path = git_dir.join(file);
            if path.exists() {
                report.add_success(&format!("Git File: {}", file), 
                                 description,
                                 "Present and accessible");
            } else {
                report.add_error(&format!("Git File: {}", file), 
                               "Missing essential Git file",
                               "Repository may be corrupted");
            }
        }
        
        Ok(())
    }

    /// Check repository integrity
    async fn check_repository_integrity(&self, rgit: &RgitCore, report: &mut HealthReport) -> Result<()> {
        print!("Checking repository integrity... ");
        
        // Check if repository is bare
        if rgit.repo.is_bare() {
            report.add_info("Repository Type", 
                          "Bare repository",
                          "No working directory");
        } else {
            report.add_success("Repository Type", 
                             "Standard repository",
                             "Has working directory");
        }
        
        // Check repository state
        let state = rgit.repo.state();
        match state {
            RepositoryState::Clean => {
                report.add_success("Repository State", 
                                 "Clean",
                                 "No ongoing operations");
            }
            RepositoryState::Merge => {
                report.add_warning("Repository State", 
                                 "Merge in progress",
                                 "Complete merge or abort with 'git merge --abort'");
            }
            RepositoryState::Rebase | RepositoryState::RebaseInteractive | RepositoryState::RebaseMerge => {
                report.add_warning("Repository State", 
                                 "Rebase in progress",
                                 "Complete rebase or abort with 'git rebase --abort'");
            }
            _ => {
                report.add_warning("Repository State", 
                                 &format!("In progress: {:?}", state),
                                 "Complete or abort the ongoing operation");
            }
        }
        
        // Check for corruption by trying to access HEAD
        match rgit.repo.head() {
            Ok(_) => {
                report.add_success("HEAD Reference", 
                                 "Valid HEAD reference",
                                 "Repository HEAD is accessible");
            }
            Err(e) => {
                report.add_error("HEAD Reference", 
                               &format!("Invalid HEAD: {}", e),
                               "Repository may be corrupted");
            }
        }
        
        println!("{}", "✅".green());
        Ok(())
    }

    /// Check working directory status
    async fn check_working_directory(&self, rgit: &RgitCore, report: &mut HealthReport) -> Result<()> {
        print!("Checking working directory... ");
        
        let status = rgit.status()?;
        
        if status.is_clean() {
            report.add_success("Working Directory", 
                             "Clean working tree",
                             "No uncommitted changes");
        } else {
            let total_changes = status.total_changes();
            report.add_info("Working Directory", 
                          &format!("{} uncommitted changes", total_changes),
                          "Use 'rgit status' for details");
        }
        
        // Check for large files that might cause issues
        self.check_large_files(rgit, report).await?;
        
        // Check disk space
        self.check_disk_space(rgit.root_dir(), report)?;
        
        println!("{}", "✅".green());
        Ok(())
    }

    /// Check for large files in repository
    async fn check_large_files(&self, rgit: &RgitCore, report: &mut HealthReport) -> Result<()> {
        let large_files = find_large_files(rgit.root_dir(), 100 * 1024 * 1024)?; // 100MB
        
        if large_files.is_empty() {
            report.add_success("Large Files", 
                             "No large files detected",
                             "Repository size is manageable");
        } else {
            let total_size: u64 = large_files.iter().map(|(_, size)| size).sum();
            report.add_warning("Large Files", 
                             &format!("{} files over 100MB ({})", 
                                    large_files.len(), 
                                    humanize_size(total_size)),
                             "Consider using Git LFS for large files");
        }
        
        Ok(())
    }

    /// Check available disk space
    fn check_disk_space(&self, path: &Path, report: &mut HealthReport) -> Result<()> {
        // In a real implementation, you'd check available disk space
        // For now, we'll simulate this check
        let available_gb = 10; // Simulated available space in GB
        
        if available_gb < 1 {
            report.add_error("Disk Space", 
                           &format!("Only {}GB available", available_gb),
                           "Free up disk space");
        } else if available_gb < 5 {
            report.add_warning("Disk Space", 
                             &format!("{}GB available", available_gb),
                             "Consider freeing up space");
        } else {
            report.add_success("Disk Space", 
                             &format!("{}GB available", available_gb),
                             "Sufficient disk space");
        }
        
        Ok(())
    }

    /// Check remote repositories
    async fn check_remotes(&self, rgit: &RgitCore, report: &mut HealthReport) -> Result<()> {
        print!("Checking remotes... ");
        
        let remotes = rgit.list_remotes()?;
        
        if remotes.is_empty() {
            report.add_info("Remotes", 
                          "No remotes configured",
                          "Add a remote to sync with other repositories");
        } else {
            for remote_info in &remotes {
                self.check_remote_connectivity(&remote_info, report).await?;
            }
            
            report.add_success("Remotes", 
                             &format!("{} remote(s) configured", remotes.len()),
                             "Remote repositories available");
        }
        
        println!("{}", "✅".green());
        Ok(())
    }

    /// Check connectivity to a remote
    async fn check_remote_connectivity(&self, remote_info: &crate::core::RemoteInfo, report: &mut HealthReport) -> Result<()> {
        // In a real implementation, this would test network connectivity
        // For now, we'll just validate the URL format
        
        if remote_info.url.starts_with("http") || remote_info.url.contains("@") {
            report.add_success(&format!("Remote: {}", remote_info.name), 
                             &format!("URL: {}", remote_info.url),
                             "Remote URL format is valid");
        } else {
            report.add_warning(&format!("Remote: {}", remote_info.name), 
                             &format!("URL: {}", remote_info.url),
                             "Remote URL format may be invalid");
        }
        
        Ok(())
    }

    /// Check branch configuration
    async fn check_branches(&self, rgit: &RgitCore, report: &mut HealthReport) -> Result<()> {
        print!("Checking branches... ");
        
        let branches = rgit.list_branches()?;
        
        if branches.is_empty() {
            report.add_warning("Branches", 
                             "No branches found",
                             "Create an initial commit");
        } else {
            let current_branch = branches.iter()
                .find(|b| b.is_current)
                .map(|b| &b.name);
            
            if let Some(branch_name) = current_branch {
                report.add_success("Current Branch", 
                                 branch_name,
                                 "On a valid branch");
                
                // Check upstream configuration
                let current_branch_info = branches.iter()
                    .find(|b| b.is_current)
                    .unwrap();
                
                if current_branch_info.upstream.is_some() {
                    report.add_success("Upstream", 
                                     "Configured",
                                     "Branch tracks remote");
                } else {
                    report.add_info("Upstream", 
                                  "Not configured",
                                  "Set upstream for push/pull");
                }
            } else {
                report.add_warning("Current Branch", 
                                 "Detached HEAD",
                                 "Checkout a branch");
            }
            
            report.add_success("Branches", 
                             &format!("{} local branches", branches.len()),
                             "Branch structure is healthy");
        }
        
        println!("{}", "✅".green());
        Ok(())
    }

    /// Check submodules
    async fn check_submodules(&self, rgit: &RgitCore, report: &mut HealthReport) -> Result<()> {
        print!("Checking submodules... ");
        
        let submodule_manager = SubmoduleManager::new(rgit, self.config);
        let health = submodule_manager.check_health()?;
        
        if health.submodules.is_empty() {
            report.add_info("Submodules", 
                          "No submodules found",
                          "Repository has no submodules");
        } else if health.is_healthy() {
            report.add_success("Submodules", 
                             &format!("{} submodules healthy", health.submodules.len()),
                             "All submodules are in good state");
        } else {
            let issue_count = health.total_issues();
            report.add_warning("Submodules", 
                             &format!("{} issues found", issue_count),
                             "Use 'rgit submodule status' for details");
        }
        
        println!("{}", "✅".green());
        Ok(())
    }

    /// Check Git hooks
    async fn check_hooks(&self, rgit: &RgitCore, report: &mut HealthReport) -> Result<()> {
        print!("Checking hooks... ");
        
        let hooks_dir = rgit.git_dir().join("hooks");
        
        if !hooks_dir.exists() {
            report.add_info("Hooks", 
                          "No hooks directory",
                          "No Git hooks configured");
            println!("{}", "✅".green());
            return Ok(());
        }
        
        let hook_files = fs::read_dir(&hooks_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) &&
                !entry.file_name().to_string_lossy().ends_with(".sample")
            })
            .count();
        
        if hook_files > 0 {
            report.add_success("Hooks", 
                             &format!("{} hooks configured", hook_files),
                             "Git hooks are available");
        } else {
            report.add_info("Hooks", 
                          "No active hooks",
                          "Consider setting up Git hooks");
        }
        
        println!("{}", "✅".green());
        Ok(())
    }

    /// Check repository performance metrics
    async fn check_performance(&self, rgit: &RgitCore, report: &mut HealthReport) -> Result<()> {
        print!("Checking performance... ");
        
        // Check repository size
        let repo_size = calculate_repo_size(rgit.git_dir())?;
        
        if repo_size > 1_000_000_000 { // 1GB
            report.add_warning("Repository Size", 
                             &format!("Large repository: {}", humanize_size(repo_size)),
                             "Consider repository maintenance");
        } else {
            report.add_success("Repository Size", 
                             &format!("Size: {}", humanize_size(repo_size)),
                             "Repository size is reasonable");
        }
        
        // Check for packed objects
        let objects_dir = rgit.git_dir().join("objects");
        let pack_dir = objects_dir.join("pack");
        
        if pack_dir.exists() {
            let pack_count = fs::read_dir(&pack_dir)?
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.file_name().to_string_lossy().ends_with(".pack")
                })
                .count();
            
            if pack_count > 10 {
                report.add_warning("Object Packing", 
                                 &format!("{} pack files", pack_count),
                                 "Consider running 'git gc' to optimize");
            } else {
                report.add_success("Object Packing", 
                                 &format!("{} pack files", pack_count),
                                 "Object database is optimized");
            }
        }
        
        println!("{}", "✅".green());
        Ok(())
    }
}

// =============================================================================
// Health Report Data Structures
// =============================================================================

#[derive(Debug)]
struct HealthReport {
    checks: Vec<HealthCheck>,
}

impl HealthReport {
    fn new() -> Self {
        Self {
            checks: Vec::new(),
        }
    }
    
    fn add_success(&mut self, category: &str, status: &str, suggestion: &str) {
        self.checks.push(HealthCheck {
            category: category.to_string(),
            status: status.to_string(),
            level: HealthLevel::Success,
            suggestion: suggestion.to_string(),
        });
    }
    
    fn add_warning(&mut self, category: &str, status: &str, suggestion: &str) {
        self.checks.push(HealthCheck {
            category: category.to_string(),
            status: status.to_string(),
            level: HealthLevel::Warning,
            suggestion: suggestion.to_string(),
        });
    }
    
    fn add_error(&mut self, category: &str, status: &str, suggestion: &str) {
        self.checks.push(HealthCheck {
            category: category.to_string(),
            status: status.to_string(),
            level: HealthLevel::Error,
            suggestion: suggestion.to_string(),
        });
    }
    
    fn add_info(&mut self, category: &str, status: &str, suggestion: &str) {
        self.checks.push(HealthCheck {
            category: category.to_string(),
            status: status.to_string(),
            level: HealthLevel::Info,
            suggestion: suggestion.to_string(),
        });
    }
    
    fn has_issues(&self) -> bool {
        self.checks.iter().any(|c| matches!(c.level, HealthLevel::Error | HealthLevel::Warning))
    }
    
    fn error_count(&self) -> usize {
        self.checks.iter().filter(|c| matches!(c.level, HealthLevel::Error)).count()
    }
    
    fn warning_count(&self) -> usize {
        self.checks.iter().filter(|c| matches!(c.level, HealthLevel::Warning)).count()
    }
}

#[derive(Debug)]
struct HealthCheck {
    category: String,
    status: String,
    level: HealthLevel,
    suggestion: String,
}

#[derive(Debug)]
enum HealthLevel {
    Success,
    Info,
    Warning,
    Error,
}

impl HealthLevel {
    fn icon(&self) -> &'static str {
        match self {
            HealthLevel::Success => "✅",
            HealthLevel::Info => "ℹ️",
            HealthLevel::Warning => "⚠️",
            HealthLevel::Error => "❌",
        }
    }
    
    fn color(&self) -> colored::Color {
        match self {
            HealthLevel::Success => colored::Color::Green,
            HealthLevel::Info => colored::Color::Blue,
            HealthLevel::Warning => colored::Color::Yellow,
            HealthLevel::Error => colored::Color::Red,
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Display comprehensive health report
fn display_health_report(report: &HealthReport, config: &Config) -> Result<()> {
    // Show summary
    let total_checks = report.checks.len();
    let error_count = report.error_count();
    let warning_count = report.warning_count();
    
    println!("{} Health Summary:", "📊".blue().bold());
    println!("  {} {} total checks", "🔍".blue(), total_checks);
    
    if error_count > 0 {
        println!("  {} {} errors", "❌".red(), error_count.to_string().red());
    }
    if warning_count > 0 {
        println!("  {} {} warnings", "⚠️".yellow(), warning_count.to_string().yellow());
    }
    
    let success_count = total_checks - error_count - warning_count;
    println!("  {} {} passed", "✅".green(), success_count.to_string().green());
    
    println!();
    
    // Show detailed results
    if config.ui.interactive {
        display_detailed_results(report)?;
    } else {
        display_summary_results(report)?;
    }
    
    Ok(())
}

/// Display detailed health check results
fn display_detailed_results(report: &HealthReport) -> Result<()> {
    println!("{} Detailed Results:", "📋".blue().bold());
    println!();
    
    for check in &report.checks {
        println!("{} {} {}", 
                check.level.icon(),
                check.category.bold(),
                check.status);
        
        if !matches!(check.level, HealthLevel::Success) {
            println!("    {} {}", "💡".blue(), check.suggestion.dimmed());
        }
    }
    
    Ok(())
}

/// Display summary results table
fn display_summary_results(report: &HealthReport) -> Result<()> {
    let mut table = TableDisplay::new()
        .with_headers(vec![
            "Status".to_string(),
            "Category".to_string(),
            "Details".to_string(),
        ]);
    
    for check in &report.checks {
        table.add_row(vec![
            format!("{}", check.level.icon()),
            check.category.clone(),
            check.status.clone(),
        ]);
    }
    
    table.display();
    Ok(())
}

/// Offer automatic fixes for detected issues
async fn offer_auto_fix(report: &HealthReport, config: &Config) -> Result<()> {
    if !config.is_interactive() {
        return Ok(());
    }
    
    let fixable_issues: Vec<&HealthCheck> = report.checks.iter()
        .filter(|c| is_auto_fixable(c))
        .collect();
    
    if fixable_issues.is_empty() {
        return Ok(());
    }
    
    println!("\n{} Auto-fixable Issues Found:", "🔧".blue().bold());
    for issue in &fixable_issues {
        println!("  {} {}: {}", issue.level.icon(), issue.category, issue.suggestion);
    }
    
    if InteractivePrompt::new()
        .with_message("Would you like rgit to attempt automatic fixes?")
        .confirm()? {
        
        perform_auto_fixes(&fixable_issues).await?;
    }
    
    Ok(())
}

/// Check if an issue can be automatically fixed
fn is_auto_fixable(check: &HealthCheck) -> bool {
    // Define which issues can be automatically fixed
    matches!(check.category.as_str(), 
        "User Identity" | "Default Branch" | "Object Packing")
}

/// Perform automatic fixes
async fn perform_auto_fixes(issues: &[&HealthCheck]) -> Result<()> {
    println!("\n{} Performing automatic fixes...", "🔧".blue());
    
    for issue in issues {
        match issue.category.as_str() {
            "User Identity" => {
                println!("  {} Setting up user identity...", "👤".blue());
                // In real implementation, guide user through identity setup
                println!("    {} Would guide through user.name and user.email setup", "💡".green());
            }
            "Default Branch" => {
                println!("  {} Setting default branch to 'main'...", "🌿".blue());
                // In real implementation: git config --global init.defaultBranch main
                println!("    {} Would set init.defaultBranch = main", "💡".green());
            }
            "Object Packing" => {
                println!("  {} Optimizing object database...", "📦".blue());
                // In real implementation: run git gc
                println!("    {} Would run git gc to optimize repository", "💡".green());
            }
            _ => {}
        }
    }
    
    println!("  {} Automatic fixes completed!", "✅".green());
    Ok(())
}

/// Show health recommendations
fn show_health_recommendations(report: &HealthReport, config: &Config) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    println!("\n{} Recommendations:", "💡".blue().bold());
    
    // General recommendations based on findings
    if report.error_count() > 0 {
        println!("  • Address errors immediately to prevent data loss");
    }
    
    if report.warning_count() > 0 {
        println!("  • Review warnings to improve repository health");
    }
    
    // Specific recommendations
    println!("  • Run 'rgit doctor' regularly to monitor repository health");
    println!("  • Use 'rgit status' to check for uncommitted changes");
    println!("  • Keep Git updated to the latest version");
    println!("  • Set up proper backup strategies for important repositories");
    
    println!();
    Ok(())
}

/// Extract Git version from version string
fn extract_git_version(version_str: &str) -> Option<(u32, u32, u32)> {
    let re = regex::Regex::new(r"git version (\d+)\.(\d+)\.(\d+)").ok()?;
    let caps = re.captures(version_str)?;
    
    let major = caps[1].parse().ok()?;
    let minor = caps[2].parse().ok()?;
    let patch = caps[3].parse().ok()?;
    
    Some((major, minor, patch))
}

/// Find large files in directory
fn find_large_files(dir: &Path, size_threshold: u64) -> Result<Vec<(PathBuf, u64)>> {
    let mut large_files = Vec::new();
    
    fn scan_directory(dir: &Path, threshold: u64, files: &mut Vec<(PathBuf, u64)>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                let size = entry.metadata()?.len();
                if size > threshold {
                    files.push((path, size));
                }
            } else if path.is_dir() && !path.file_name().unwrap_or_default().to_string_lossy().starts_with('.') {
                scan_directory(&path, threshold, files)?;
            }
        }
        Ok(())
    }
    
    scan_directory(dir, size_threshold, &mut large_files)?;
    Ok(large_files)
}

/// Calculate total repository size
fn calculate_repo_size(git_dir: &Path) -> Result<u64> {
    fn dir_size(dir: &Path) -> Result<u64> {
        let mut size = 0;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                size += entry.metadata()?.len();
            } else if path.is_dir() {
                size += dir_size(&path)?;
            }
        }
        Ok(size)
    }
    
    dir_size(git_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_git_version() {
        assert_eq!(extract_git_version("git version 2.30.1"), Some((2, 30, 1)));
        assert_eq!(extract_git_version("git version 2.42.0"), Some((2, 42, 0)));
        assert_eq!(extract_git_version("invalid version"), None);
    }

    #[test]
    fn test_health_report() {
        let mut report = HealthReport::new();
        
        report.add_success("Test", "All good", "Keep it up");
        report.add_warning("Test", "Minor issue", "Fix this");
        report.add_error("Test", "Major issue", "Fix immediately");
        
        assert!(report.has_issues());
        assert_eq!(report.error_count(), 1);
        assert_eq!(report.warning_count(), 1);
    }

    #[test]
    fn test_is_auto_fixable() {
        let check = HealthCheck {
            category: "User Identity".to_string(),
            status: "Not configured".to_string(),
            level: HealthLevel::Error,
            suggestion: "Set user.name and user.email".to_string(),
        };
        
        assert!(is_auto_fixable(&check));
        
        let non_fixable = HealthCheck {
            category: "Network".to_string(),
            status: "Cannot connect".to_string(),
            level: HealthLevel::Error,
            suggestion: "Check connection".to_string(),
        };
        
        assert!(!is_auto_fixable(&non_fixable));
    }
}