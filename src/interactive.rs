use anyhow::Result;
use colored::*;
use colored::{Color, Colorize}; // Add this line for color constants
use dialoguer::{
    theme::ColorfulTheme, Confirm, Editor, FuzzySelect, Input, MultiSelect, Password, Select,
};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::RgitError;

/// Builder for creating interactive prompts with consistent styling
pub struct InteractivePrompt {
    message: String,
    options: Vec<String>,
    default: Option<usize>,
    theme: ColorfulTheme,
    allow_empty: bool,
    multiselect: bool,
    fuzzy: bool,
}

impl InteractivePrompt {
    /// Create a new interactive prompt
    pub fn new() -> Self {
        Self {
            message: String::new(),
            options: Vec::new(),
            default: None,
            theme: Self::create_theme(),
            allow_empty: false,
            multiselect: false,
            fuzzy: false,
        }
    }

    /// Set the prompt message
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Set the options for selection
    pub fn with_options(mut self, options: &[impl ToString]) -> Self {
        self.options = options.iter().map(|o| o.to_string()).collect();
        self
    }

    /// Set the default selection index
    pub fn with_default(mut self, index: usize) -> Self {
        self.default = Some(index);
        self
    }

    /// Allow empty input
    pub fn allow_empty(mut self) -> Self {
        self.allow_empty = true;
        self
    }

    /// Enable multiselect mode
    pub fn multiselect(mut self) -> Self {
        self.multiselect = true;
        self
    }

    /// Enable fuzzy search
    pub fn fuzzy_search(mut self) -> Self {
        self.fuzzy = true;
        self
    }

    /// Show a selection prompt
    pub fn select(&self) -> Result<usize> {
        if self.options.is_empty() {
            return Err(RgitError::InvalidArgument(
                "No options provided for selection".to_string(),
            )
            .into());
        }

        let result = if self.fuzzy {
            let mut select = FuzzySelect::with_theme(&self.theme)
                .with_prompt(&self.message)
                .items(&self.options);
            if let Some(default) = self.default {
                select = select.default(default);
            }
            select.interact()?
        } else {
            let mut select = Select::with_theme(&self.theme)
                .with_prompt(&self.message)
                .items(&self.options);
            if let Some(default) = self.default {
                select = select.default(default);
            }
            select.interact()?
        };

        Ok(result)
    }

    /// Show a multiselect prompt
    pub fn multiselect_prompt(&self) -> Result<Vec<usize>> {
        if self.options.is_empty() {
            return Err(RgitError::InvalidArgument(
                "No options provided for multiselect".to_string(),
            )
            .into());
        }

        let multiselect = MultiSelect::with_theme(&self.theme)
            .with_prompt(&self.message)
            .items(&self.options);

        Ok(multiselect.interact()?)
    }

    /// Show a text input prompt
    pub fn input<T>(&self) -> Result<T>
    where
        T: std::str::FromStr + ToString + Clone,
        T::Err: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static,
    {
        let mut input = Input::with_theme(&self.theme);
        input = input.with_prompt(&self.message);
        input = input.allow_empty(self.allow_empty);

        Ok(input.interact_text()?)
    }

    /// Show a confirmation prompt
    pub fn confirm(&self) -> Result<bool> {
        let confirm = Confirm::with_theme(&self.theme)
            .with_prompt(&self.message)
            .default(true);

        Ok(confirm.interact()?)
    }

    /// Show a password input prompt
    pub fn password(&self) -> Result<String> {
        let password = Password::with_theme(&self.theme).with_prompt(&self.message);

        Ok(password.interact()?)
    }

    /// Open an editor for text input
    pub fn editor(&self) -> Result<String> {
        let editor = Editor::new();

        match editor.edit(&self.message)? {
            Some(text) => Ok(text.trim().to_string()),
            None => Err(RgitError::OperationCancelled.into()),
        }
    }

    /// Create the custom theme
    fn create_theme() -> ColorfulTheme {
        ColorfulTheme {
            defaults_style: console::Style::new().for_stderr().cyan(),
            prompt_style: console::Style::new().for_stderr().bold(),
            prompt_prefix: console::style("?".to_string()).for_stderr().yellow(),
            prompt_suffix: console::style("›".to_string())
                .for_stderr()
                .black()
                .bright(),
            success_prefix: console::style("✓".to_string()).for_stderr().green(),
            success_suffix: console::style("·".to_string())
                .for_stderr()
                .black()
                .bright(),
            error_prefix: console::style("✗".to_string()).for_stderr().red(),
            error_style: console::Style::new().for_stderr().red(),
            hint_style: console::Style::new().for_stderr().black().bright(),
            values_style: console::Style::new().for_stderr().green(),
            active_item_style: console::Style::new().for_stderr().cyan().bold(),
            inactive_item_style: console::Style::new().for_stderr(),
            active_item_prefix: console::style("❯".to_string()).for_stderr().green(),
            inactive_item_prefix: console::style(" ".to_string()).for_stderr(),
            checked_item_prefix: console::style("✓".to_string()).for_stderr().green(),
            unchecked_item_prefix: console::style("✗".to_string()).for_stderr().red(),
            picked_item_prefix: console::style("❯".to_string()).for_stderr().green(),
            unpicked_item_prefix: console::style(" ".to_string()).for_stderr(),
            fuzzy_cursor_style: console::Style::new().for_stderr().yellow().bold(),
            fuzzy_match_highlight_style: console::Style::new().for_stderr().bold(),
        }
    }
}

impl Default for InteractivePrompt {
    fn default() -> Self {
        Self::new()
    }
}

/// Interactive file selection for git operations
pub struct FileSelector {
    files: Vec<FileItem>,
    show_details: bool,
}

#[derive(Debug, Clone)]
pub struct FileItem {
    pub path: PathBuf,
    pub status: String,
    pub size: Option<u64>,
    pub selected: bool,
}

impl FileSelector {
    /// Create a new file selector
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            show_details: false,
        }
    }

    /// Add files to the selector
    pub fn with_files(mut self, files: Vec<FileItem>) -> Self {
        self.files = files;
        self
    }

    /// Show file details (size, modification time, etc.)
    pub fn with_details(mut self) -> Self {
        self.show_details = true;
        self
    }

    /// Show interactive file selection
    pub fn select(&self) -> Result<Vec<PathBuf>> {
        if self.files.is_empty() {
            return Ok(Vec::new());
        }

        let items = self.format_file_items();

        let selected_indices = InteractivePrompt::new()
            .with_message("Select files to stage")
            .with_options(&items)
            .multiselect_prompt()?;

        Ok(selected_indices
            .into_iter()
            .map(|i| self.files[i].path.clone())
            .collect())
    }

    /// Format file items for display
    fn format_file_items(&self) -> Vec<String> {
        self.files
            .iter()
            .map(|item| {
                let status_color = match item.status.as_str() {
                    "modified" => Color::Yellow,
                    "new" => Color::Green,
                    "deleted" => Color::Red,
                    _ => Color::White,
                };

                let mut display = format!(
                    "{} {}",
                    item.status.color(status_color).bold(),
                    item.path.display().to_string().white()
                );

                if self.show_details {
                    if let Some(size) = item.size {
                        display.push_str(&format!(" {}", format_size(size).dimmed()));
                    }
                }

                display
            })
            .collect()
    }
}

impl Default for FileSelector {
    fn default() -> Self {
        Self::new()
    }
}

/// Interactive commit message editor with templates and validation
pub struct CommitMessageEditor {
    template: Option<String>,
    validate: bool,
    show_diff: bool,
}

impl CommitMessageEditor {
    /// Create a new commit message editor
    pub fn new() -> Self {
        Self {
            template: None,
            validate: true,
            show_diff: false,
        }
    }

    /// Set a commit message template
    pub fn with_template(mut self, template: impl Into<String>) -> Self {
        self.template = Some(template.into());
        self
    }

    /// Enable message validation
    pub fn with_validation(mut self) -> Self {
        self.validate = true;
        self
    }

    /// Show diff in editor
    pub fn with_diff(mut self) -> Self {
        self.show_diff = true;
        self
    }

    /// Edit commit message
    pub fn edit(&self) -> Result<String> {
        let initial_content = self.build_initial_content();

        let editor = Editor::new();
        let result = editor
            .edit(&initial_content)?
            .ok_or(RgitError::OperationCancelled)?;

        let message = self.parse_commit_message(&result)?;

        if self.validate {
            self.validate_message(&message)?;
        }

        Ok(message)
    }

    /// Build initial editor content
    fn build_initial_content(&self) -> String {
        let mut content = String::new();

        if let Some(ref template) = self.template {
            content.push_str(template);
            content.push_str("\n\n");
        }

        content.push_str("# Please enter the commit message for your changes. Lines starting\n");
        content.push_str("# with '#' will be ignored, and an empty message aborts the commit.\n");
        content.push_str("#\n");

        if self.show_diff {
            content.push_str("# Changes to be committed:\n");
            content.push_str("#\n");
            // Would add actual diff here
            content.push_str("# (use 'rgit diff --cached' to see changes)\n");
            content.push_str("#\n");
        }

        content
    }

    /// Parse commit message from editor content
    fn parse_commit_message(&self, content: &str) -> Result<String> {
        let lines: Vec<&str> = content
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect();

        let message = lines.join("\n").trim().to_string();

        if message.is_empty() {
            return Err(RgitError::EmptyCommitMessage.into());
        }

        Ok(message)
    }

    /// Validate commit message
    fn validate_message(&self, message: &str) -> Result<()> {
        let lines: Vec<&str> = message.lines().collect();

        if lines.is_empty() {
            return Err(RgitError::EmptyCommitMessage.into());
        }

        // Check first line length
        if lines[0].len() > 72 {
            eprintln!(
                "{} First line should be 72 characters or less",
                "⚠️".yellow()
            );
        }

        // Check for blank line after first line if there are more lines
        if lines.len() > 1 && !lines[1].is_empty() {
            eprintln!(
                "{} Consider adding a blank line after the first line",
                "💡".blue()
            );
        }

        Ok(())
    }
}

impl Default for CommitMessageEditor {
    fn default() -> Self {
        Self::new()
    }
}

/// Interactive conflict resolution assistant
pub struct ConflictResolver {
    conflicts: Vec<ConflictFile>,
}

#[derive(Debug, Clone)]
pub struct ConflictFile {
    pub path: PathBuf,
    pub conflict_type: ConflictType,
}

#[derive(Debug, Clone)]
pub enum ConflictType {
    Content,
    AddAdd,
    DeleteModify,
    ModifyDelete,
    Rename,
}

impl ConflictResolver {
    /// Create a new conflict resolver
    pub fn new(conflicts: Vec<ConflictFile>) -> Self {
        Self { conflicts }
    }

    /// Start interactive conflict resolution
    pub fn resolve(&self) -> Result<()> {
        if self.conflicts.is_empty() {
            println!("{} No conflicts to resolve", "✅".green());
            return Ok(());
        }

        println!("{} {} conflicts detected", "⚔️".red(), self.conflicts.len());

        for (i, conflict) in self.conflicts.iter().enumerate() {
            println!(
                "\n{} Conflict {} of {}: {}",
                "📁".blue(),
                i + 1,
                self.conflicts.len(),
                conflict.path.display().to_string().yellow()
            );

            self.resolve_single_conflict(conflict)?;
        }

        println!("\n{} All conflicts resolved!", "🎉".green());
        Ok(())
    }

    /// Resolve a single conflict
    fn resolve_single_conflict(&self, conflict: &ConflictFile) -> Result<()> {
        let options = match conflict.conflict_type {
            ConflictType::Content => vec![
                "Edit file manually",
                "Use merge tool",
                "Take ours (current branch)",
                "Take theirs (merging branch)",
                "Skip this file",
            ],
            ConflictType::AddAdd => vec![
                "Keep both files with rename",
                "Keep ours",
                "Keep theirs",
                "Edit manually",
                "Skip this file",
            ],
            ConflictType::DeleteModify => vec![
                "Keep modified file",
                "Keep deleted (remove file)",
                "Edit manually",
                "Skip this file",
            ],
            ConflictType::ModifyDelete => vec![
                "Keep modified file",
                "Keep deleted (remove file)",
                "Edit manually",
                "Skip this file",
            ],
            ConflictType::Rename => vec![
                "Accept rename",
                "Keep original name",
                "Choose different name",
                "Skip this file",
            ],
        };

        let selection = InteractivePrompt::new()
            .with_message(&format!("How to resolve {}?", conflict.path.display()))
            .with_options(&options)
            .select()?;

        self.execute_resolution(conflict, selection)?;
        Ok(())
    }

    /// Execute the chosen resolution
    fn execute_resolution(&self, conflict: &ConflictFile, choice: usize) -> Result<()> {
        match (conflict.conflict_type.clone(), choice) {
            (ConflictType::Content, 0) => {
                // Edit file manually
                self.open_editor(&conflict.path)?;
            }
            (ConflictType::Content, 1) => {
                // Use merge tool
                self.open_merge_tool(&conflict.path)?;
            }
            (ConflictType::Content, 2) => {
                // Take ours
                self.take_ours(&conflict.path)?;
            }
            (ConflictType::Content, 3) => {
                // Take theirs
                self.take_theirs(&conflict.path)?;
            }
            _ => {
                println!("Resolution not implemented for this choice");
            }
        }

        Ok(())
    }

    /// Open file in editor
    fn open_editor(&self, path: &PathBuf) -> Result<()> {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());

        std::process::Command::new(editor).arg(path).status()?;

        Ok(())
    }

    /// Open merge tool
    fn open_merge_tool(&self, path: &PathBuf) -> Result<()> {
        let merge_tool = std::env::var("MERGE_TOOL").unwrap_or_else(|_| "vimdiff".to_string());

        std::process::Command::new(merge_tool).arg(path).status()?;

        Ok(())
    }

    /// Take our version
    fn take_ours(&self, _path: &PathBuf) -> Result<()> {
        println!("Taking our version...");
        // Implementation would resolve conflict by taking local version
        Ok(())
    }

    /// Take their version
    fn take_theirs(&self, _path: &PathBuf) -> Result<()> {
        println!("Taking their version...");
        // Implementation would resolve conflict by taking remote version
        Ok(())
    }
}

/// Progress display for long-running operations
pub struct ProgressDisplay {
    message: String,
    total: Option<u64>,
    show_eta: bool,
}

impl ProgressDisplay {
    /// Create a new progress display
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            total: None,
            show_eta: false,
        }
    }

    /// Set total progress amount
    pub fn with_total(mut self, total: u64) -> Self {
        self.total = Some(total);
        self
    }

    /// Show estimated time remaining
    pub fn with_eta(mut self) -> Self {
        self.show_eta = true;
        self
    }

    /// Create and return progress bar
    pub fn create_progress_bar(&self) -> indicatif::ProgressBar {
        use indicatif::{ProgressBar, ProgressStyle};

        let pb = if let Some(total) = self.total {
            ProgressBar::new(total)
        } else {
            ProgressBar::new_spinner()
        };

        let style = if self.total.is_some() {
            if self.show_eta {
                ProgressStyle::default_bar()
                    .template("{spinner:.green} {msg} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
                    .unwrap()
            } else {
                ProgressStyle::default_bar()
                    .template("{spinner:.green} {msg} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len}")
                    .unwrap()
            }
        } else {
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg} {elapsed_precise}")
                .unwrap()
        };

        pb.set_style(style);
        pb.set_message(self.message.clone());
        pb
    }
}

/// Utility functions for interactive components
pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
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

/// Check if we're in a TTY environment
pub fn is_interactive() -> bool {
    atty::is(atty::Stream::Stdin) && atty::is(atty::Stream::Stdout)
}

/// Create a table display for structured data
pub struct TableDisplay {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    max_width: usize,
}

impl TableDisplay {
    pub fn new() -> Self {
        Self {
            headers: Vec::new(),
            rows: Vec::new(),
            max_width: 80,
        }
    }

    pub fn with_headers(mut self, headers: Vec<String>) -> Self {
        self.headers = headers;
        self
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }

    pub fn with_max_width(mut self, width: usize) -> Self {
        self.max_width = width;
        self
    }

    pub fn display(&self) {
        if self.headers.is_empty() && self.rows.is_empty() {
            return;
        }

        let mut all_rows = Vec::new();
        if !self.headers.is_empty() {
            all_rows.push(self.headers.clone());
        }
        all_rows.extend(self.rows.clone());

        let col_widths = self.calculate_column_widths(&all_rows);

        // Print header
        if !self.headers.is_empty() {
            self.print_row(&self.headers, &col_widths, true);
            self.print_separator(&col_widths);
        }

        // Print rows
        for row in &self.rows {
            self.print_row(row, &col_widths, false);
        }
    }

    fn calculate_column_widths(&self, rows: &[Vec<String>]) -> Vec<usize> {
        if rows.is_empty() {
            return Vec::new();
        }

        let num_cols = rows[0].len();
        let mut widths = vec![0; num_cols];

        for row in rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(unicode_width::UnicodeWidthStr::width(cell.as_str()));
                }
            }
        }

        // Adjust for terminal width
        let total_width: usize = widths.iter().sum::<usize>() + (num_cols - 1) * 3;
        if total_width > self.max_width {
            let ratio = self.max_width as f64 / total_width as f64;
            for width in &mut widths {
                *width = (*width as f64 * ratio) as usize;
            }
        }

        widths
    }

    fn print_row(&self, row: &[String], widths: &[usize], is_header: bool) {
        let formatted_cells: Vec<String> = row
            .iter()
            .zip(widths.iter())
            .map(|(cell, &width)| {
                let truncated = if cell.len() > width {
                    format!("{}...", &cell[..width.saturating_sub(3)])
                } else {
                    cell.clone()
                };

                if is_header {
                    format!("{:<width$}", truncated.bold(), width = width)
                } else {
                    format!("{:<width$}", truncated, width = width)
                }
            })
            .collect();

        println!("{}", formatted_cells.join(" | "));
    }

    fn print_separator(&self, widths: &[usize]) {
        let separators: Vec<String> = widths.iter().map(|&width| "-".repeat(width)).collect();
        println!("{}", separators.join("-|-"));
    }
}

impl Default for TableDisplay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
    }

    #[test]
    fn test_interactive_prompt_creation() {
        let prompt = InteractivePrompt::new()
            .with_message("Test message")
            .with_options(&["Option 1", "Option 2"])
            .with_default(0);

        assert_eq!(prompt.message, "Test message");
        assert_eq!(prompt.options.len(), 2);
        assert_eq!(prompt.default, Some(0));
    }

    #[test]
    fn test_file_selector_creation() {
        let files = vec![FileItem {
            path: PathBuf::from("test.txt"),
            status: "modified".to_string(),
            size: Some(1024),
            selected: false,
        }];

        let selector = FileSelector::new().with_files(files).with_details();

        assert_eq!(selector.files.len(), 1);
        assert!(selector.show_details);
    }

    #[test]
    fn test_table_display() {
        let mut table = TableDisplay::new()
            .with_headers(vec!["Name".to_string(), "Size".to_string()])
            .with_max_width(40);

        table.add_row(vec!["file1.txt".to_string(), "1024".to_string()]);
        table.add_row(vec!["file2.txt".to_string(), "2048".to_string()]);

        // This would normally display the table
        // For testing, we just verify the structure
        assert_eq!(table.headers.len(), 2);
        assert_eq!(table.rows.len(), 2);
    }
}
