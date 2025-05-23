use anyhow::Result;
use chrono::{DateTime, Local, TimeZone, Utc};
use colored::*;
use git2::{Time, Oid, Repository, BranchType};
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_width::UnicodeWidthStr;

use crate::error::RgitError;

// =============================================================================
// Time and Date Utilities
// =============================================================================

/// Format a git2::Time as a human-readable "time ago" string
pub fn format_time_ago(time: Time) -> String {
    let now = chrono::Utc::now().timestamp();
    let commit_time = time.seconds();
    let diff = now - commit_time;

    match diff {
        0..=59 => "just now".to_string(),
        60..=3599 => {
            let minutes = diff / 60;
            format!("{} minute{} ago", minutes, if minutes == 1 { "" } else { "s" })
        }
        3600..=86399 => {
            let hours = diff / 3600;
            format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
        }
        86400..=2591999 => {
            let days = diff / 86400;
            format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
        }
        2592000..=31535999 => {
            let months = diff / 2592000;
            format!("{} month{} ago", months, if months == 1 { "" } else { "s" })
        }
        _ => {
            let years = diff / 31536000;
            format!("{} year{} ago", years, if years == 1 { "" } else { "s" })
        }
    }
}

/// Format a git2::Time as a standard date string
pub fn format_date(time: Time) -> String {
    let datetime = Utc.timestamp_opt(time.seconds(), 0)
        .single()
        .unwrap_or_else(|| Utc::now());
    
    datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

/// Format a git2::Time as a local date string
pub fn format_local_date(time: Time) -> String {
    let utc_datetime = Utc.timestamp_opt(time.seconds(), 0)
        .single()
        .unwrap_or_else(|| Utc::now());
    
    let local_datetime: DateTime<Local> = utc_datetime.into();
    local_datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Get current timestamp as git2::Time
pub fn current_time() -> Time {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    
    Time::new(now, 0)
}

// =============================================================================
// String and Text Utilities
// =============================================================================

/// Truncate a string to a maximum length with ellipsis
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Truncate string by Unicode width (for proper terminal display)
pub fn truncate_by_width(s: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut end = 0;
    
    for (i, c) in s.char_indices() {
        let char_width = UnicodeWidthStr::width(c.encode_utf8(&mut [0; 4]));
        if width + char_width > max_width {
            break;
        }
        width += char_width;
        end = i + c.len_utf8();
    }
    
    if end < s.len() {
        if max_width <= 3 {
            "...".to_string()
        } else {
            let truncated = &s[..end];
            let available = max_width - 3;
            let mut final_width = 0;
            let mut final_end = 0;
            
            for (i, c) in truncated.char_indices() {
                let char_width = UnicodeWidthStr::width(c.encode_utf8(&mut [0; 4]));
                if final_width + char_width > available {
                    break;
                }
                final_width += char_width;
                final_end = i + c.len_utf8();
            }
            
            format!("{}...", &truncated[..final_end])
        }
    } else {
        s.to_string()
    }
}

/// Pad string to a specific width (Unicode-aware)
pub fn pad_string(s: &str, width: usize, align: TextAlign) -> String {
    let current_width = UnicodeWidthStr::width(s);
    
    if current_width >= width {
        return s.to_string();
    }
    
    let padding = width - current_width;
    
    match align {
        TextAlign::Left => format!("{}{}", s, " ".repeat(padding)),
        TextAlign::Right => format!("{}{}", " ".repeat(padding), s),
        TextAlign::Center => {
            let left_pad = padding / 2;
            let right_pad = padding - left_pad;
            format!("{}{}{}", " ".repeat(left_pad), s, " ".repeat(right_pad))
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TextAlign {
    Left,
    Right,
    Center,
}

/// Word wrap text to fit within a specific width
pub fn word_wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        
        let mut current_line = String::new();
        let mut current_width = 0;
        
        for word in paragraph.split_whitespace() {
            let word_width = UnicodeWidthStr::width(word);
            let space_width = if current_line.is_empty() { 0 } else { 1 };
            
            if current_width + space_width + word_width <= width {
                if !current_line.is_empty() {
                    current_line.push(' ');
                    current_width += 1;
                }
                current_line.push_str(word);
                current_width += word_width;
            } else {
                if !current_line.is_empty() {
                    lines.push(current_line);
                }
                current_line = word.to_string();
                current_width = word_width;
            }
        }
        
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    
    lines
}

/// Highlight search terms in text
pub fn highlight_matches(text: &str, pattern: &str, case_sensitive: bool) -> String {
    if pattern.is_empty() {
        return text.to_string();
    }
    
    let regex_pattern = if case_sensitive {
        regex::escape(pattern)
    } else {
        format!("(?i){}", regex::escape(pattern))
    };
    
    match Regex::new(&regex_pattern) {
        Ok(re) => {
            re.replace_all(text, |caps: &regex::Captures| {
                caps[0].yellow().bold().to_string()
            }).to_string()
        }
        Err(_) => text.to_string(),
    }
}

// =============================================================================
// File and Path Utilities
// =============================================================================

/// Get human-readable file size
pub fn humanize_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
    const THRESHOLD: f64 = 1024.0;
    
    if bytes == 0 {
        return "0 B".to_string();
    }
    
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= THRESHOLD && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Calculate file changes (additions, deletions, modifications)
pub fn calculate_file_changes(repo: &Repository, from: Option<Oid>, to: Option<Oid>) -> Result<FileChangeStats> {
    let mut stats = FileChangeStats::default();
    
    let from_tree = if let Some(oid) = from {
        Some(repo.find_commit(oid)?.tree()?)
    } else {
        None
    };
    
    let to_tree = if let Some(oid) = to {
        Some(repo.find_commit(oid)?.tree()?)
    } else {
        None
    };
    
    let diff = repo.diff_tree_to_tree(
        from_tree.as_ref(),
        to_tree.as_ref(),
        None,
    )?;
    
    diff.foreach(
        &mut |_delta, _progress| true,
        None,
        None,
        Some(&mut |_delta, _hunk, line| {
            match line.origin() {
                '+' => stats.additions += 1,
                '-' => stats.deletions += 1,
                _ => {}
            }
            true
        }),
    )?;
    
    stats.files = diff.deltas().len();
    Ok(stats)
}

#[derive(Debug, Default, Clone)]
pub struct FileChangeStats {
    pub files: usize,
    pub additions: usize,
    pub deletions: usize,
}

impl FileChangeStats {
    pub fn total_changes(&self) -> usize {
        self.additions + self.deletions
    }
    
    pub fn format_summary(&self) -> String {
        if self.files == 0 {
            "no changes".to_string()
        } else {
            format!(
                "{} file{}, {} insertion{}, {} deletion{}",
                self.files,
                if self.files == 1 { "" } else { "s" },
                self.additions,
                if self.additions == 1 { "" } else { "s" },
                self.deletions,
                if self.deletions == 1 { "" } else { "s" }
            )
        }
    }
}

/// Get relative path from repository root
pub fn get_relative_path(repo_root: &Path, file_path: &Path) -> PathBuf {
    file_path.strip_prefix(repo_root)
        .unwrap_or(file_path)
        .to_path_buf()
}

/// Check if path is inside repository
pub fn is_path_in_repo(repo_root: &Path, file_path: &Path) -> bool {
    file_path.canonicalize()
        .map(|canonical| canonical.starts_with(repo_root))
        .unwrap_or(false)
}

/// Find common prefix of multiple paths
pub fn find_common_prefix(paths: &[PathBuf]) -> Option<PathBuf> {
    if paths.is_empty() {
        return None;
    }
    
    if paths.len() == 1 {
        return paths[0].parent().map(|p| p.to_path_buf());
    }
    
    let mut common = paths[0].clone();
    
    for path in &paths[1..] {
        common = find_common_prefix_two(&common, path)?;
    }
    
    Some(common)
}

fn find_common_prefix_two(path1: &Path, path2: &Path) -> Option<PathBuf> {
    let components1: Vec<_> = path1.components().collect();
    let components2: Vec<_> = path2.components().collect();
    
    let mut common = PathBuf::new();
    
    for (comp1, comp2) in components1.iter().zip(components2.iter()) {
        if comp1 == comp2 {
            common.push(comp1);
        } else {
            break;
        }
    }
    
    if common.as_os_str().is_empty() {
        None
    } else {
        Some(common)
    }
}

// =============================================================================
// Git Utilities
// =============================================================================

/// Get branch status information (ahead/behind counts)
pub fn get_branch_status(repo: &Repository, branch_name: &str) -> Result<BranchStatus> {
    let mut status = BranchStatus::default();
    
    let branch = repo.find_branch(branch_name, BranchType::Local)
        .map_err(|_| RgitError::BranchNotFound(branch_name.to_string()))?;
    
    let head = repo.head()?;
    let local_oid = head.target()
        .ok_or_else(|| RgitError::InvalidRepositoryState("No HEAD target".to_string()))?;
    
    // Get upstream information
    if let Ok(upstream) = branch.upstream() {
        status.has_upstream = true;
        status.upstream_name = upstream.name()?.map(|s| s.to_string());
        
        if let Some(upstream_oid) = upstream.get().target() {
            let (ahead, behind) = repo.graph_ahead_behind(local_oid, upstream_oid)?;
            status.ahead = ahead;
            status.behind = behind;
        }
    }
    
    Ok(status)
}

#[derive(Debug, Default, Clone)]
pub struct BranchStatus {
    pub has_upstream: bool,
    pub upstream_name: Option<String>,
    pub ahead: usize,
    pub behind: usize,
}

impl BranchStatus {
    pub fn is_up_to_date(&self) -> bool {
        self.ahead == 0 && self.behind == 0
    }
    
    pub fn format_status(&self) -> String {
        if !self.has_upstream {
            "no upstream".dimmed().to_string()
        } else if self.is_up_to_date() {
            "up to date".green().to_string()
        } else {
            match (self.ahead, self.behind) {
                (0, behind) => format!("{} behind", behind.to_string().red()),
                (ahead, 0) => format!("{} ahead", ahead.to_string().green()),
                (ahead, behind) => format!("{} ahead, {} behind", 
                                         ahead.to_string().green(), 
                                         behind.to_string().red()),
            }
        }
    }
}

/// Validate Git reference name
pub fn is_valid_ref_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 255 {
        return false;
    }
    
    // Basic validation rules for Git reference names
    let invalid_chars = [' ', '~', '^', ':', '?', '*', '[', '\\'];
    if name.chars().any(|c| invalid_chars.contains(&c)) {
        return false;
    }
    
    if name.starts_with('/') || name.ends_with('/') {
        return false;
    }
    
    if name.contains("//") || name.contains("..") {
        return false;
    }
    
    if name.starts_with('.') || name.ends_with('.') {
        return false;
    }
    
    if name.ends_with(".lock") {
        return false;
    }
    
    true
}

/// Parse Git URL to extract components
pub fn parse_git_url(url: &str) -> Option<GitUrlInfo> {
    // SSH format: git@host:user/repo.git
    if let Some(caps) = Regex::new(r"^git@([^:]+):(.+?)(?:\.git)?/?$").ok()?.captures(url) {
        return Some(GitUrlInfo {
            protocol: "ssh".to_string(),
            host: caps[1].to_string(),
            path: caps[2].to_string(),
            original: url.to_string(),
        });
    }
    
    // HTTPS format: https://host/user/repo.git
    if let Some(caps) = Regex::new(r"^https://([^/]+)/(.+?)(?:\.git)?/?$").ok()?.captures(url) {
        return Some(GitUrlInfo {
            protocol: "https".to_string(),
            host: caps[1].to_string(),
            path: caps[2].to_string(),
            original: url.to_string(),
        });
    }
    
    // HTTP format: http://host/user/repo.git
    if let Some(caps) = Regex::new(r"^http://([^/]+)/(.+?)(?:\.git)?/?$").ok()?.captures(url) {
        return Some(GitUrlInfo {
            protocol: "http".to_string(),
            host: caps[1].to_string(),
            path: caps[2].to_string(),
            original: url.to_string(),
        });
    }
    
    // Git protocol: git://host/user/repo.git
    if let Some(caps) = Regex::new(r"^git://([^/]+)/(.+?)(?:\.git)?/?$").ok()?.captures(url) {
        return Some(GitUrlInfo {
            protocol: "git".to_string(),
            host: caps[1].to_string(),
            path: caps[2].to_string(),
            original: url.to_string(),
        });
    }
    
    None
}

#[derive(Debug, Clone)]
pub struct GitUrlInfo {
    pub protocol: String,
    pub host: String,
    pub path: String,
    pub original: String,
}

impl GitUrlInfo {
    pub fn repository_name(&self) -> String {
        self.path
            .split('/')
            .last()
            .unwrap_or("repository")
            .to_string()
    }
    
    pub fn owner(&self) -> Option<String> {
        let parts: Vec<&str> = self.path.split('/').collect();
        if parts.len() >= 2 {
            Some(parts[parts.len() - 2].to_string())
        } else {
            None
        }
    }
}

// =============================================================================
// Terminal and Display Utilities
// =============================================================================

/// Get terminal size
pub fn get_terminal_size() -> (usize, usize) {
    terminal_size::terminal_size()
        .map(|(w, h)| (w.0 as usize, h.0 as usize))
        .unwrap_or((80, 24))
}

/// Create a horizontal line separator
pub fn create_separator(width: usize, character: char) -> String {
    character.to_string().repeat(width.min(120))
}

/// Center text within a given width
pub fn center_text(text: &str, width: usize) -> String {
    let text_width = UnicodeWidthStr::width(text);
    if text_width >= width {
        return text.to_string();
    }
    
    let padding = width - text_width;
    let left_pad = padding / 2;
    let right_pad = padding - left_pad;
    
    format!("{}{}{}", " ".repeat(left_pad), text, " ".repeat(right_pad))
}

/// Create a progress bar string
pub fn create_progress_bar(current: usize, total: usize, width: usize) -> String {
    if total == 0 {
        return "█".repeat(width);
    }
    
    let filled = (current * width) / total;
    let empty = width - filled;
    
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

// =============================================================================
// Validation Utilities
// =============================================================================

/// Validate email address format
pub fn is_valid_email(email: &str) -> bool {
    let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$");
    email_regex.map(|re| re.is_match(email)).unwrap_or(false)
}

/// Validate commit message format
pub fn validate_commit_message(message: &str) -> Vec<String> {
    let mut issues = Vec::new();
    let lines: Vec<&str> = message.lines().collect();
    
    if lines.is_empty() || lines[0].trim().is_empty() {
        issues.push("Commit message cannot be empty".to_string());
        return issues;
    }
    
    // Check subject line length
    if lines[0].len() > 50 {
        issues.push("Subject line should be 50 characters or less".to_string());
    }
    
    // Check for period at end of subject
    if lines[0].ends_with('.') {
        issues.push("Subject line should not end with a period".to_string());
    }
    
    // Check for blank line after subject
    if lines.len() > 1 && !lines[1].is_empty() {
        issues.push("Add a blank line after the subject line".to_string());
    }
    
    // Check body line length
    for (i, line) in lines.iter().enumerate().skip(2) {
        if line.len() > 72 {
            issues.push(format!("Line {} is too long (72 characters max)", i + 1));
        }
    }
    
    issues
}

// =============================================================================
// Hash and Encoding Utilities
// =============================================================================

/// Shorten Git object ID to readable format
pub fn shorten_oid(oid: &Oid, length: usize) -> String {
    let oid_str = oid.to_string();
    if length >= oid_str.len() {
        oid_str
    } else {
        oid_str[..length.min(40)].to_string()
    }
}

/// Generate random string for temporary operations
pub fn generate_random_string(length: usize) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;
    
    let mut hasher = DefaultHasher::new();
    SystemTime::now().hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    
    let hash = hasher.finish();
    let chars = "abcdefghijklmnopqrstuvwxyz0123456789";
    
    (0..length)
        .map(|i| {
            let index = ((hash >> (i * 6)) & 0x3F) as usize % chars.len();
            chars.chars().nth(index).unwrap_or('a')
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Time;

    #[test]
    fn test_time_formatting() {
        let now = chrono::Utc::now().timestamp();
        let recent_time = Time::new(now - 300, 0); // 5 minutes ago
        
        let formatted = format_time_ago(recent_time);
        assert!(formatted.contains("minute"));
    }

    #[test]
    fn test_string_truncation() {
        assert_eq!(truncate_string("hello world", 5), "he...");
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello", 3), "...");
    }

    #[test]
    fn test_size_formatting() {
        assert_eq!(humanize_size(0), "0 B");
        assert_eq!(humanize_size(1024), "1.0 KB");
        assert_eq!(humanize_size(1536), "1.5 KB");
        assert_eq!(humanize_size(1048576), "1.0 MB");
    }

    #[test]
    fn test_ref_name_validation() {
        assert!(is_valid_ref_name("feature/new-feature"));
        assert!(is_valid_ref_name("main"));
        assert!(!is_valid_ref_name("feature..name"));
        assert!(!is_valid_ref_name("/invalid"));
        assert!(!is_valid_ref_name("invalid/"));
        assert!(!is_valid_ref_name("branch name")); // space
    }

    #[test]
    fn test_git_url_parsing() {
        let ssh_url = parse_git_url("git@github.com:user/repo.git").unwrap();
        assert_eq!(ssh_url.protocol, "ssh");
        assert_eq!(ssh_url.host, "github.com");
        assert_eq!(ssh_url.path, "user/repo");
        
        let https_url = parse_git_url("https://github.com/user/repo").unwrap();
        assert_eq!(https_url.protocol, "https");
        assert_eq!(https_url.host, "github.com");
        assert_eq!(https_url.path, "user/repo");
    }

    #[test]
    fn test_email_validation() {
        assert!(is_valid_email("user@example.com"));
        assert!(is_valid_email("test.email+tag@domain.co.uk"));
        assert!(!is_valid_email("invalid-email"));
        assert!(!is_valid_email("@domain.com"));
        assert!(!is_valid_email("user@"));
    }

    #[test]
    fn test_commit_message_validation() {
        let good_message = "Fix bug in user authentication\n\nThis commit fixes the issue where users could not log in.";
        let issues = validate_commit_message(good_message);
        assert!(issues.is_empty());
        
        let bad_message = "This is a very long subject line that exceeds the recommended 50 character limit";
        let issues = validate_commit_message(bad_message);
        assert!(!issues.is_empty());
        assert!(issues[0].contains("50 characters"));
    }

    #[test]
    fn test_word_wrap() {
        let text = "This is a long line that should be wrapped at word boundaries";
        let wrapped = word_wrap(text, 20);
        
        assert!(wrapped.len() > 1);
        for line in &wrapped {
            assert!(line.len() <= 20);
        }
    }

    #[test]
    fn test_path_utilities() {
        let paths = vec![
            PathBuf::from("/home/user/project/src/main.rs"),
            PathBuf::from("/home/user/project/src/lib.rs"),
            PathBuf::from("/home/user/project/tests/test.rs"),
        ];
        
        let common = find_common_prefix(&paths).unwrap();
        assert_eq!(common, PathBuf::from("/home/user/project"));
    }

    #[test]
    fn test_progress_bar() {
        let progress = create_progress_bar(50, 100, 20);
        assert_eq!(progress.len(), 20);
        assert!(progress.contains('█'));
        assert!(progress.contains('░'));
    }

    #[test]
    fn test_oid_shortening() {
        let oid_str = "a1b2c3d4e5f6789012345678901234567890abcd";
        let oid = Oid::from_str(oid_str).unwrap();
        
        assert_eq!(shorten_oid(&oid, 7), "a1b2c3d");
        assert_eq!(shorten_oid(&oid, 12), "a1b2c3d4e5f6");
    }
}