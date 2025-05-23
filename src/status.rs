use anyhow::Result;
use colored::*;
use git2::{Status, StatusOptions};
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

use crate::core::{RgitCore, RepositoryStatus, FileStatus, BranchInfo};
use crate::utils::{format_time_ago, humanize_size, truncate_string};

/// Enhanced status display with beautiful formatting
pub struct StatusDisplay {
    /// Show detailed file information
    pub show_details: bool,
    /// Use short format
    pub short_format: bool,
    /// Show ignored files
    pub show_ignored: bool,
    /// Show submodule status
    pub show_submodules: bool,
    /// Show ahead/behind information
    pub show_ahead_behind: bool,
    /// Show file timestamps
    pub show_timestamps: bool,
    /// Terminal width for formatting
    pub terminal_width: usize,
}

impl Default for StatusDisplay {
    fn default() -> Self {
        Self {
            show_details: true,
            short_format: false,
            show_ignored: false,
            show_submodules: false,
            show_ahead_behind: true,
            show_timestamps: false,
            terminal_width: terminal_size::terminal_size()
                .map(|(w, _)| w.0 as usize)
                .unwrap_or(80),
        }
    }
}

impl StatusDisplay {
    /// Create a new status display with custom settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure display options from CLI arguments
    pub fn from_args(
        short: bool,
        ignored: bool,
        submodules: bool,
        ahead_behind: bool,
        timestamps: bool,
    ) -> Self {
        Self {
            show_details: !short,
            short_format: short,
            show_ignored: ignored,
            show_submodules: submodules,
            show_ahead_behind: ahead_behind,
            show_timestamps: timestamps,
            ..Default::default()
        }
    }

    /// Display the complete repository status
    pub fn display(&self, rgit: &RgitCore) -> Result<()> {
        let status = rgit.status()?;

        if self.short_format {
            self.display_short_format(&status)?;
        } else {
            self.display_detailed_format(rgit, &status)?;
        }

        Ok(())
    }

    /// Display status in short format (similar to git status --short)
    fn display_short_format(&self, status: &RepositoryStatus) -> Result<()> {
        // Show branch info first
        if !self.short_format {
            self.display_branch_header(&status.branch_info)?;
        }

        // Display files in short format
        for file in &status.staged {
            let index_status = self.get_short_status_char(&file.status, true);
            let workdir_status = self.get_short_status_char(&file.status, false);
            println!("{}{} {}", 
                    index_status.green(), 
                    workdir_status.red(), 
                    file.path);
        }

        for file in &status.unstaged {
            let index_status = self.get_short_status_char(&file.status, true);
            let workdir_status = self.get_short_status_char(&file.status, false);
            println!("{}{} {}", 
                    index_status.green(), 
                    workdir_status.red(), 
                    file.path);
        }

        for file in &status.untracked {
            println!("?? {}", file.path.red());
        }

        Ok(())
    }

    /// Display status in detailed format with beautiful formatting
    fn display_detailed_format(&self, rgit: &RgitCore, status: &RepositoryStatus) -> Result<()> {
        // Display header with repository info
        self.display_repository_header(rgit)?;
        
        // Display branch information
        self.display_branch_info(&status.branch_info)?;

        // Show summary if there are changes
        if !status.is_clean() {
            self.display_change_summary(status)?;
            println!();
        }

        // Display sections for different types of changes
        if !status.staged.is_empty() {
            self.display_staged_changes(&status.staged)?;
        }

        if !status.unstaged.is_empty() {
            self.display_unstaged_changes(&status.unstaged)?;
        }

        if !status.untracked.is_empty() {
            self.display_untracked_files(&status.untracked)?;
        }

        // Show clean status if no changes
        if status.is_clean() {
            self.display_clean_status()?;
        }

        // Display helpful hints
        self.display_hints(status)?;

        // Show submodule status if requested
        if self.show_submodules {
            self.display_submodule_status(rgit)?;
        }

        Ok(())
    }

    /// Display repository header with basic info
    fn display_repository_header(&self, rgit: &RgitCore) -> Result<()> {
        let repo_name = rgit.repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repository");

        println!("{} {} {}", 
                "📁".blue(), 
                "Repository:".bold(), 
                repo_name.cyan().bold());

        if self.show_details {
            let repo_path = rgit.repo_path.display().to_string();
            let truncated_path = if repo_path.len() > self.terminal_width - 15 {
                format!("...{}", &repo_path[repo_path.len() - (self.terminal_width - 18)..])
            } else {
                repo_path
            };
            println!("   📍 {}", truncated_path.dimmed());
        }

        Ok(())
    }

    /// Display detailed branch information
    fn display_branch_info(&self, branch_info: &BranchInfo) -> Result<()> {
        // Branch name with status
        let branch_icon = if branch_info.is_current { "🌿" } else { "📋" };
        print!("{} {} {}", 
               branch_icon.blue(), 
               "On branch".bold(), 
               branch_info.name.cyan().bold());

        // Detached HEAD warning
        if branch_info.name.contains("detached") {
            print!(" {}", "(detached HEAD)".yellow().bold());
        }
        println!();

        // Upstream tracking information
        if self.show_ahead_behind {
            self.display_tracking_info(branch_info)?;
        }

        // Last commit information
        if let Some(ref commit) = branch_info.last_commit {
            self.display_last_commit_info(commit)?;
        }

        println!();
        Ok(())
    }

    /// Display tracking information with ahead/behind status
    fn display_tracking_info(&self, branch_info: &BranchInfo) -> Result<()> {
        match &branch_info.upstream {
            Some(upstream) => {
                print!("   🔗 Tracking {}", upstream.cyan());
                
                match (branch_info.ahead, branch_info.behind) {
                    (0, 0) => println!(" {}", "(up to date)".green()),
                    (ahead, 0) if ahead > 0 => {
                        println!(" {} {} ahead", 
                                "↑".green().bold(), 
                                format!("({} commit{})", ahead, if ahead == 1 { "" } else { "s" }).green())
                    }
                    (0, behind) if behind > 0 => {
                        println!(" {} {} behind", 
                                "↓".red().bold(), 
                                format!("({} commit{})", behind, if behind == 1 { "" } else { "s" }).red())
                    }
                    (ahead, behind) if ahead > 0 && behind > 0 => {
                        println!(" {} {} ahead, {} {} behind",
                                "↑".green().bold(),
                                format!("({} commit{})", ahead, if ahead == 1 { "" } else { "s" }).green(),
                                "↓".red().bold(),
                                format!("({} commit{})", behind, if behind == 1 { "" } else { "s" }).red())
                    }
                    _ => println!(),
                }
            }
            None => {
                println!("   {} {}", "⚠️".yellow(), "No upstream branch configured".yellow());
                if !branch_info.name.contains("detached") {
                    println!("      Use 'rgit push --set-upstream origin {}' to set upstream", 
                            branch_info.name.cyan());
                }
            }
        }
        Ok(())
    }

    /// Display last commit information
    fn display_last_commit_info(&self, commit: &crate::core::CommitInfo) -> Result<()> {
        if self.show_details {
            let time_ago = format_time_ago(commit.time);
            let short_message = truncate_string(&commit.message.lines().next().unwrap_or(""), 60);
            
            println!("   📝 Last commit: {} {} by {} {}",
                    commit.oid[..8].yellow(),
                    short_message.white(),
                    commit.author.cyan(),
                    time_ago.dimmed());
        }
        Ok(())
    }

    /// Display change summary with statistics
    fn display_change_summary(&self, status: &RepositoryStatus) -> Result<()> {
        let total_changes = status.total_changes();
        let staged_count = status.staged.len();
        let unstaged_count = status.unstaged.len();
        let untracked_count = status.untracked.len();

        println!("{} {} total changes:", "📊".blue(), total_changes.to_string().bold());
        
        if staged_count > 0 {
            println!("   {} {} staged", "✅".green(), staged_count.to_string().green().bold());
        }
        if unstaged_count > 0 {
            println!("   {} {} unstaged", "📝".yellow(), unstaged_count.to_string().yellow().bold());
        }
        if untracked_count > 0 {
            println!("   {} {} untracked", "❓".red(), untracked_count.to_string().red().bold());
        }

        Ok(())
    }

    /// Display staged changes section
    fn display_staged_changes(&self, staged: &[FileStatus]) -> Result<()> {
        println!("{} {} to be committed:", 
                "📦".green().bold(), 
                "Changes".green().bold());
        
        for file in staged {
            self.display_file_status(file, true)?;
        }
        
        println!();
        Ok(())
    }

    /// Display unstaged changes section
    fn display_unstaged_changes(&self, unstaged: &[FileStatus]) -> Result<()> {
        println!("{} {} not staged for commit:", 
                "📝".yellow().bold(), 
                "Changes".yellow().bold());
        
        for file in unstaged {
            self.display_file_status(file, false)?;
        }
        
        println!("  {} Use \"{}\" to stage changes",
                "💡".blue(),
                "rgit add <file>...".cyan());
        println!();
        Ok(())
    }

    /// Display untracked files section
    fn display_untracked_files(&self, untracked: &[FileStatus]) -> Result<()> {
        println!("{} {} files:", 
                "❓".red().bold(), 
                "Untracked".red().bold());
        
        for file in untracked {
            self.display_file_status(file, false)?;
        }
        
        println!("  {} Use \"{}\" to include in what will be committed",
                "💡".blue(),
                "rgit add <file>...".cyan());
        println!();
        Ok(())
    }

    /// Display individual file status with formatting
    fn display_file_status(&self, file: &FileStatus, staged: bool) -> Result<()> {
        let status_symbol = file.status_symbol(staged);
        let status_color = if staged { Green } else if status_symbol == "untracked" { Red } else { Yellow };
        
        let mut line = format!("  {} {}:",
                              if staged { "✓" } else if status_symbol == "untracked" { "?" } else { "○" }.color(status_color).bold(),
                              status_symbol.color(status_color));

        // File path with proper formatting
        let file_path = if file.path.len() > 50 {
            format!("...{}", &file.path[file.path.len() - 47..])
        } else {
            file.path.clone()
        };
        
        line.push_str(&format!(" {}", file_path.white()));

        // Additional file information
        if self.show_details {
            let mut details = Vec::new();
            
            // File size
            details.push(file.format_size());
            
            // Modification time
            if self.show_timestamps {
                if let Some(modified) = file.modified_time {
                    details.push(format_time_ago_from_systemtime(modified));
                }
            }
            
            if !details.is_empty() {
                line.push_str(&format!(" {}", format!("({})", details.join(", ")).dimmed()));
            }
        }

        println!("{}", line);
        Ok(())
    }

    /// Display clean working tree status
    fn display_clean_status(&self) -> Result<()> {
        println!("{} {}", 
                "✨".green(), 
                "Working tree clean".green().bold());
        
        if self.show_details {
            println!("   Nothing to commit, working tree clean");
        }
        
        Ok(())
    }

    /// Display helpful hints based on current status
    fn display_hints(&self, status: &RepositoryStatus) -> Result<()> {
        if !status.is_clean() {
            println!("{} {} Helpful commands:", "💡".blue(), "Tip:".bold());
            
            if !status.unstaged.is_empty() || !status.untracked.is_empty() {
                println!("   • {} - Interactive file selection", "rgit add".cyan());
            }
            
            if !status.staged.is_empty() {
                println!("   • {} - Commit staged changes", "rgit commit".cyan());
            }
            
            if !status.is_clean() {
                println!("   • {} - Quick commit workflow", "rgit quick-commit".cyan());
                println!("   • {} - Sync with remote", "rgit sync".cyan());
            }
            
            println!();
        }
        
        Ok(())
    }

    /// Display submodule status if enabled
    fn display_submodule_status(&self, rgit: &RgitCore) -> Result<()> {
        let submodules = rgit.repo.submodules()?;
        
        if !submodules.is_empty() {
            println!("{} {} status:", "📦".blue().bold(), "Submodule".blue().bold());
            
            for submodule in submodules {
                let name = submodule.name().unwrap_or("unknown");
                let path = submodule.path().display();
                
                let status_icon = if submodule.open().is_ok() {
                    "✅"
                } else {
                    "❓"
                };
                
                println!("  {} {} {}", 
                        status_icon,
                        name.cyan(),
                        format!("({})", path).dimmed());
            }
            
            println!("   💡 Use \"{}\" for detailed submodule information",
                    "rgit submodule status".cyan());
            println!();
        }
        
        Ok(())
    }

    /// Get short status character for git status --short format
    fn get_short_status_char(&self, status: Status, index: bool) -> char {
        if index {
            if status.contains(Status::INDEX_NEW) { 'A' }
            else if status.contains(Status::INDEX_MODIFIED) { 'M' }
            else if status.contains(Status::INDEX_DELETED) { 'D' }
            else if status.contains(Status::INDEX_RENAMED) { 'R' }
            else if status.contains(Status::INDEX_TYPECHANGE) { 'T' }
            else { ' ' }
        } else {
            if status.contains(Status::WT_NEW) { '?' }
            else if status.contains(Status::WT_MODIFIED) { 'M' }
            else if status.contains(Status::WT_DELETED) { 'D' }
            else if status.contains(Status::WT_RENAMED) { 'R' }
            else if status.contains(Status::WT_TYPECHANGE) { 'T' }
            else { ' ' }
        }
    }
}

/// Format system time as "time ago" string
fn format_time_ago_from_systemtime(time: std::time::SystemTime) -> String {
    match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => {
            let seconds = duration.as_secs() as i64;
            let git_time = git2::Time::new(seconds, 0);
            format_time_ago(git_time)
        }
        Err(_) => "unknown".to_string(),
    }
}

/// Create a visual separator line
pub fn create_separator(width: usize, char: char) -> String {
    char.to_string().repeat(width.min(80))
}

/// Format a table row with proper spacing
pub fn format_table_row(columns: &[&str], widths: &[usize]) -> String {
    columns
        .iter()
        .zip(widths.iter())
        .map(|(col, width)| format!("{:<width$}", col, width = width))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Calculate optimal column widths for table display
pub fn calculate_column_widths(rows: &[Vec<String>], terminal_width: usize) -> Vec<usize> {
    if rows.is_empty() {
        return Vec::new();
    }

    let num_cols = rows[0].len();
    let mut widths = vec![0; num_cols];
    
    // Calculate maximum width for each column
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(UnicodeWidthStr::width(cell.as_str()));
            }
        }
    }
    
    // Adjust widths to fit terminal
    let total_width: usize = widths.iter().sum();
    let separators = (num_cols - 1) * 3; // " | " between columns
    let available = terminal_width.saturating_sub(separators);
    
    if total_width > available {
        // Proportionally reduce column widths
        let ratio = available as f64 / total_width as f64;
        for width in &mut widths {
            *width = (*width as f64 * ratio) as usize;
        }
    }
    
    widths
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_repo_with_files() -> (TempDir, git2::Repository) {
        let temp_dir = TempDir::new().unwrap();
        let repo = git2::Repository::init(temp_dir.path()).unwrap();
        
        // Create some test files
        fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();
        fs::write(temp_dir.path().join("new_file.txt"), "new content").unwrap();
        
        (temp_dir, repo)
    }

    #[test]
    fn test_status_display_creation() {
        let display = StatusDisplay::new();
        assert!(display.show_details);
        assert!(!display.short_format);
    }

    #[test]
    fn test_short_status_chars() {
        let display = StatusDisplay::new();
        assert_eq!(display.get_short_status_char(Status::INDEX_NEW, true), 'A');
        assert_eq!(display.get_short_status_char(Status::WT_MODIFIED, false), 'M');
        assert_eq!(display.get_short_status_char(Status::WT_NEW, false), '?');
    }

    #[test]
    fn test_column_width_calculation() {
        let rows = vec![
            vec!["short".to_string(), "medium_length".to_string()],
            vec!["very_long_content".to_string(), "short".to_string()],
        ];
        
        let widths = calculate_column_widths(&rows, 80);
        assert_eq!(widths.len(), 2);
        assert!(widths[0] >= "very_long_content".len());
        assert!(widths[1] >= "medium_length".len());
    }

    #[test]
    fn test_separator_creation() {
        let separator = create_separator(10, '-');
        assert_eq!(separator, "----------");
        
        let long_separator = create_separator(100, '=');
        assert_eq!(long_separator.len(), 80); // Should be capped at 80
    }
}