use anyhow::{Context, Result};
use colored::*;
use git2::{DiffOptions, Repository, Status};
use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, error, info, instrument, warn};

use crate::cli::AddArgs;
use crate::config::Config;
use crate::core::{FileStatus, RgitCore};
use crate::interactive::{FileItem, FileSelector, InteractivePrompt};

#[derive(Error, Debug)]
pub enum AddError {
    #[error("Path traversal attempt detected: {path}")]
    PathTraversal { path: String },
    
    #[error("File too large: {path} ({size} bytes, max: {max_size})")]
    FileTooLarge { path: String, size: u64, max_size: u64 },
    
    #[error("Operation timeout after {timeout:?}")]
    Timeout { timeout: Duration },
    
    #[error("Too many files in operation: {count} (max: {max})")]
    TooManyFiles { count: usize, max: usize },
    
    #[error("Repository is locked by another process")]
    RepositoryLocked,
    
    #[error("Invalid file permissions: {path}")]
    InvalidPermissions { path: String },
    
    #[error("Patch operation failed: {reason}")]
    PatchFailed { reason: String },
    
    #[error("Interactive operation cancelled by user")]
    UserCancelled,
    
    #[error("Non-interactive environment detected")]
    NonInteractive,
    
    #[error("Git operation failed: {0}")]
    Git(#[from] git2::Error),
    
    #[error("IO operation failed: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("General error: {0}")]
    General(#[from] anyhow::Error),
}

// Configuration with sensible production defaults
#[derive(Debug, Clone)]
pub struct AddConfig {
    pub max_files_per_operation: usize,
    pub max_file_size_bytes: u64,
    pub operation_timeout: Duration,
    pub batch_size: usize,
    pub max_preview_files: usize,
    pub interactive_threshold: usize,
}

impl Default for AddConfig {
    fn default() -> Self {
        Self {
            max_files_per_operation: 10_000,
            max_file_size_bytes: 100 * 1024 * 1024, // 100MB
            operation_timeout: Duration::from_secs(300), // 5 minutes
            batch_size: 100,
            max_preview_files: 10,
            interactive_threshold: 20,
        }
    }
}

// Progress tracking for long operations
#[derive(Debug)]
pub struct ProgressTracker {
    total: usize,
    completed: usize,
    start_time: Instant,
    last_update: Instant,
}

impl ProgressTracker {
    fn new(total: usize) -> Self {
        let now = Instant::now();
        Self {
            total,
            completed: 0,
            start_time: now,
            last_update: now,
        }
    }
    
    fn update(&mut self, completed: usize) {
        self.completed = completed;
        self.last_update = Instant::now();
        
        if self.should_display_progress() {
            self.display_progress();
        }
    }
    
    fn should_display_progress(&self) -> bool {
        self.last_update.duration_since(self.start_time) > Duration::from_millis(500)
    }
    
    fn display_progress(&self) {
        let percentage = (self.completed as f64 / self.total as f64 * 100.0) as u8;
        let elapsed = self.start_time.elapsed();
        
        print!("\r{} Progress: {}/{} ({}%) - Elapsed: {:?}",
               "⏳".yellow(),
               self.completed,
               self.total,
               percentage,
               elapsed);
        io::stdout().flush().unwrap_or(());
    }
    
    fn finish(&self) {
        println!("\r{} Completed {}/{} files in {:?}",
                "✅".green(),
                self.completed,
                self.total,
                self.start_time.elapsed());
    }
}

// Secure path validation
struct PathValidator {
    repo_root: PathBuf,
    allowed_extensions: HashSet<String>,
    max_depth: usize,
    max_file_size_bytes: u64,
    max_files_per_operation: usize,
}

impl PathValidator {
    fn new(repo_root: PathBuf) -> Self {
        let mut allowed_extensions = HashSet::new();
        // Common development file extensions
        for ext in &["rs", "py", "js", "ts", "json", "yaml", "yml", "toml", "md", "txt", "html", "css", "sql"] {
            allowed_extensions.insert(ext.to_string());
        }
        
        Self {
            repo_root,
            allowed_extensions,
            max_depth: 20,
            max_file_size_bytes: 100 * 1024 * 1024, // 100MB
            max_files_per_operation: 1000,
        }
    }
    
    fn validate_file_path(&self, path: &Path) -> Result<PathBuf, AddError> {
        // Resolve path and check for traversal attempts
        let canonical = path.canonicalize()
            .map_err(|_| AddError::InvalidPermissions { 
                path: path.display().to_string() 
            })?;
        
        // Ensure path is within repository
        if !canonical.starts_with(&self.repo_root) {
            return Err(AddError::PathTraversal {
                path: path.display().to_string(),
            });
        }
        
        // Check directory depth to prevent deep nesting attacks
        let relative_path = canonical.strip_prefix(&self.repo_root).unwrap();
        if relative_path.components().count() > self.max_depth {
            return Err(AddError::PathTraversal {
                path: path.display().to_string(),
            });
        }
        
        // Validate file size
        if canonical.is_file() {
            let metadata = fs::metadata(&canonical)?;
            if metadata.len() > self.max_file_size_bytes {
                return Err(AddError::FileTooLarge {
                    path: path.display().to_string(),
                    size: metadata.len(),
                    max_size: self.max_file_size_bytes,
                });
            }
        }
        
        Ok(canonical)
    }
    
    fn validate_paths(&self, paths: &[PathBuf]) -> Result<Vec<PathBuf>, AddError> {
        if paths.len() > self.max_files_per_operation {
            return Err(AddError::TooManyFiles {
                count: paths.len(),
                max: self.max_files_per_operation,
            });
        }
        
        paths.iter()
            .map(|p| self.validate_file_path(p))
            .collect()
    }
}

// Real patch mode implementation with actual diff parsing
#[derive(Debug, Clone)]
pub struct Hunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLineInfo>,
}

#[derive(Debug, Clone)]
pub struct DiffLineInfo {
    pub origin: char,
    pub content: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
}

pub struct PatchProcessor<'repo> {
    repo: &'repo Repository,
    config: AddConfig,
}

impl<'repo> PatchProcessor<'repo> {
    fn new(repo: &'repo Repository, config: AddConfig) -> Self {
        Self { repo, config }
    }
    
    #[instrument(skip(self))]
    fn get_file_diff(&self, file_path: &Path) -> Result<Vec<Hunk>, AddError> {
        let mut diff_opts = DiffOptions::new();
        diff_opts.pathspec(file_path);
        diff_opts.context_lines(3);
        diff_opts.include_untracked(true);
        
        let diff = self.repo
            .diff_index_to_workdir(None, Some(&mut diff_opts))?;
        
        // Workaround for borrow checker: collect hunks and lines separately, then combine.
        struct TempHunk {
            header: String,
            old_start: u32,
            old_lines: u32,
            new_start: u32,
            new_lines: u32,
        }
        let mut temp_hunks: Vec<TempHunk> = Vec::new();
        let mut hunk_lines: Vec<Vec<DiffLineInfo>> = Vec::new();
        let mut current_hunk_idx: usize = 0;

        diff.foreach(
            &mut |_delta, _progress| true,
            None,
            Some(&mut |_delta, hunk| {
                temp_hunks.push(TempHunk {
                    header: String::from_utf8_lossy(hunk.header()).to_string(),
                    old_start: hunk.old_start(),
                    old_lines: hunk.old_lines(),
                    new_start: hunk.new_start(),
                    new_lines: hunk.new_lines(),
                });
                hunk_lines.push(Vec::new());
                current_hunk_idx = hunk_lines.len() - 1;
                true
            }),
            Some(&mut |_delta, _hunk, line| {
                // Always push to the last hunk_lines entry
                if !hunk_lines.is_empty() {
                    let idx = hunk_lines.len() - 1;
                    let line_info = DiffLineInfo {
                        origin: line.origin(),
                        content: String::from_utf8_lossy(line.content()).to_string(),
                        old_lineno: line.old_lineno(),
                        new_lineno: line.new_lineno(),
                    };
                    hunk_lines[idx].push(line_info);
                }
                true
            }),
        )?;

        let hunks: Vec<Hunk> = temp_hunks
            .into_iter()
            .zip(hunk_lines.into_iter())
            .map(|(h, lines)| Hunk {
                header: h.header,
                old_start: h.old_start,
                old_lines: h.old_lines,
                new_start: h.new_start,
                new_lines: h.new_lines,
                lines,
            })
            .collect();

        Ok(hunks)
    }
    
    #[instrument(skip(self))]
    fn apply_hunk(&self, file_path: &Path, hunk: &Hunk) -> Result<(), AddError> {
        // Create a temporary index to apply the hunk
        let mut index = self.repo.index()?;
        
        // Read the current file content
        let file_content = fs::read_to_string(file_path)?;
        
        let lines: Vec<&str> = file_content.lines().collect();
        let mut new_content = Vec::new();
        
        let mut line_idx = 0;
        let mut hunk_line_idx = 0;
        
        // Apply the hunk line by line
        while line_idx < lines.len() && hunk_line_idx < hunk.lines.len() {
            let hunk_line = &hunk.lines[hunk_line_idx];
            
            match hunk_line.origin {
                ' ' => {
                    // Context line - keep as is
                    new_content.push(lines[line_idx].to_string());
                    line_idx += 1;
                    hunk_line_idx += 1;
                }
                '-' => {
                    // Deleted line - skip from original
                    line_idx += 1;
                    hunk_line_idx += 1;
                }
                '+' => {
                    // Added line - add to new content
                    new_content.push(hunk_line.content.trim_end().to_string());
                    hunk_line_idx += 1;
                }
                _ => {
                    hunk_line_idx += 1;
                }
            }
        }
        
        // Add remaining lines
        while line_idx < lines.len() {
            new_content.push(lines[line_idx].to_string());
            line_idx += 1;
        }
        
        // Write the patched content back
        let patched_content = new_content.join("\n");
        fs::write(file_path, patched_content)?;
        
        // Add to index
        index.add_path(file_path.strip_prefix(self.repo.workdir().unwrap()).unwrap())?;
        index.write()?;
        
        Ok(())
    }
    
    #[instrument(skip(self))]
    fn interactive_hunk_selection(&self, file_path: &Path, hunks: &[Hunk]) -> Result<Vec<usize>, AddError> {
        let mut selected_hunks = Vec::new();
        
        println!("\n{} Processing: {}", 
                "📁".blue(), 
                file_path.display().to_string().yellow());
        
        for (idx, hunk) in hunks.iter().enumerate() {
            println!("\n{} Hunk {} of {}:", "🔍".cyan(), idx + 1, hunks.len());
            println!("{}", hunk.header.dimmed());
            
            // Display hunk content with syntax highlighting
            for line in &hunk.lines {
                match line.origin {
                    '+' => println!("{}{}", "+".green(), line.content.green()),
                    '-' => println!("{}{}", "-".red(), line.content.red()),
                    ' ' => println!(" {}", line.content),
                    _ => {}
                }
            }
            
            // Interactive prompt for this hunk
            let options = vec![
                "Add this hunk [y]",
                "Skip this hunk [n]", 
                "Add all remaining hunks [a]",
                "Skip all remaining hunks [d]",
                "Quit [q]",
                "Show help [?]",
            ];
            
            let choice = InteractivePrompt::new()
                .with_message("Add this hunk?")
                .with_options(&options)
                .with_default(0)
                .select()
                .map_err(|_| AddError::UserCancelled)?;
            
            match choice {
                0 => {
                    selected_hunks.push(idx);
                }
                1 => {
                    // Skip this hunk
                }
                2 => {
                    // Add all remaining hunks
                    selected_hunks.extend(idx..hunks.len());
                    break;
                }
                3 => {
                    // Skip all remaining hunks
                    break;
                }
                4 => {
                    return Err(AddError::UserCancelled);
                }
                5 => {
                    self.show_patch_help();
                    continue; // Re-ask for this hunk
                }
                _ => {}
            }
        }
        
        Ok(selected_hunks)
    }
    
    fn show_patch_help(&self) {
        println!("\n{} Patch mode commands:", "💡".blue().bold());
        println!("  {} - add this hunk to index", "y".green().bold());
        println!("  {} - do not add this hunk to index", "n".red().bold());
        println!("  {} - quit; do not add this hunk or any remaining ones", "q".yellow().bold());
        println!("  {} - add this hunk and all later hunks in the file", "a".green().bold());
        println!("  {} - do not add this hunk or any later hunks in the file", "d".red().bold());
        println!("  {} - show this help", "?".blue().bold());
        println!();
    }
}

// Main add command executor with comprehensive error handling
pub struct AddExecutor<'repo> {
    rgit: &'repo mut RgitCore,
    config: AddConfig,
    validator: PathValidator,
}

impl<'repo> AddExecutor<'repo> {
    pub fn new(rgit: &'repo mut RgitCore, config: AddConfig) -> Result<Self, AddError> {
        let repo_root = rgit.repo.workdir()
            .ok_or_else(|| AddError::Git(git2::Error::from_str("Repository has no working directory")))?
            .to_path_buf();
        
        let validator = PathValidator::new(repo_root);
        
        Ok(Self {
            rgit,
            config,
            validator,
        })
    }
    
    #[instrument(skip(self, args))]
    pub async fn execute(&mut self, args: &AddArgs) -> Result<(), AddError> {
        // Validate repository state
        self.validate_repository_state()?;
        
        // Execute based on arguments
        match self.determine_operation_mode(args) {
            OperationMode::AddAll => self.add_all_changes().await,
            OperationMode::AddUpdate => self.add_updated_files().await,
            OperationMode::AddPatch(files) => self.add_patch_mode(files).await,
            OperationMode::AddSpecific(files, force) => self.add_specific_files(files, force).await,
            OperationMode::Interactive => self.interactive_add().await,
        }
    }
    
    fn validate_repository_state(&self) -> Result<(), AddError> {
        // Check if repository is locked
        let lock_file = self.rgit.repo.path().join("index.lock");
        if lock_file.exists() {
            return Err(AddError::RepositoryLocked);
        }
        
        // Validate repository is in a good state
        if self.rgit.repo.state() != git2::RepositoryState::Clean {
            warn!("Repository is in an unclean state: {:?}", self.rgit.repo.state());
        }
        
        Ok(())
    }
    
    fn determine_operation_mode(&self, args: &AddArgs) -> OperationMode {
        if args.all {
            OperationMode::AddAll
        } else if args.update {
            OperationMode::AddUpdate
        } else if args.patch {
            OperationMode::AddPatch(args.files.clone())
        } else if args.files.is_empty() {
            OperationMode::Interactive
        } else {
            OperationMode::AddSpecific(args.files.clone(), args.force)
        }
    }
    
    #[instrument(skip(self))]
    async fn add_all_changes(&mut self) -> Result<(), AddError> {
        info!("Adding all changes");
        
        let status = self.rgit.status()?;
        
        if status.is_clean() {
            info!("No changes to add");
            return Ok(());
        }
        
        let total_files = status.total_changes();
        
        // Confirm operation if many files
        if total_files > self.config.interactive_threshold {
            self.show_files_preview(&status.unstaged, &status.untracked)?;
            
            if !self.confirm_add_all(total_files)? {
                return Err(AddError::UserCancelled);
            }
        }
        
        // Progress tracking for large operations
        let mut progress = ProgressTracker::new(total_files);
        
        // Add files in batches for better performance
        let mut all_files = Vec::new();
        all_files.extend(status.unstaged.iter().map(|f| PathBuf::from(&f.path)));
        all_files.extend(status.untracked.iter().map(|f| PathBuf::from(&f.path)));
        
        let validated_files = self.validator.validate_paths(&all_files)?;
        
        for (batch_idx, batch) in validated_files.chunks(self.config.batch_size).enumerate() {
            self.add_file_batch(batch)?;
            progress.update((batch_idx + 1) * self.config.batch_size.min(batch.len()));
        }
        
        progress.finish();
        info!("Successfully added {} files", total_files);
        
        self.show_add_summary("Added all changes").await?;
        Ok(())
    }
    
    #[instrument(skip(self))]
    async fn add_updated_files(&mut self) -> Result<(), AddError> {
        info!("Adding updated files only");
        
        let status = self.rgit.status()?;
        
        if status.unstaged.is_empty() {
            info!("No updated files to add");
            return Ok(());
        }
        
        let files: Vec<PathBuf> = status.unstaged
            .iter()
            .map(|f| PathBuf::from(&f.path))
            .collect();
        
        let validated_files = self.validator.validate_paths(&files)?;
        
        // Process files in batches
        for batch in validated_files.chunks(self.config.batch_size) {
            self.add_file_batch(batch)?;
        }
        
        info!("Successfully updated {} files", files.len());
        self.show_add_summary("Updated tracked files").await?;
        Ok(())
    }
    
    #[instrument(skip(self, files))]
    async fn add_specific_files(&mut self, files: Vec<PathBuf>, force: bool) -> Result<(), AddError> {
        info!("Adding {} specific files", files.len());
        
        let validated_files = self.validator.validate_paths(&files)?;
        
        let mut results = AddResults::new();
        
        for file_path in &validated_files {
            if !file_path.exists() {
                results.missing.push(file_path.clone());
                continue;
            }
            
            // Check if file is ignored
            if !force && self.is_file_ignored(file_path)? {
                results.ignored.push(file_path.clone());
                continue;
            }
            
            match self.add_single_file(file_path) {
                Ok(()) => results.added.push(file_path.clone()),
                Err(e) => {
                    error!("Failed to add {}: {}", file_path.display(), e);
                    results.failed.push((file_path.clone(), e.to_string()));
                }
            }
        }
        
        self.report_add_results(&results)?;
        
        if !results.added.is_empty() {
            self.show_add_summary("Added specific files").await?;
        }
        
        Ok(())
    }
    
    #[instrument(skip(self))]
    async fn interactive_add(&mut self) -> Result<(), AddError> {
        info!("Starting interactive add");
        
        let status = self.rgit.status()?;
        
        let addable_files = self.collect_addable_files(&status);
        
        if addable_files.is_empty() {
            info!("No files to add");
            return Ok(());
        }
        
        // Show status summary
        self.show_status_summary(&status);
        
        // Use interactive file selector
        let file_items = self.create_file_items(&addable_files)?;
        let selector = FileSelector::new()
            .with_files(file_items)
            .with_details();
        
        let selected_files = selector.select()
            .map_err(|_| AddError::UserCancelled)?;
        
        if selected_files.is_empty() {
            return Err(AddError::UserCancelled);
        }
        
        let validated_files = self.validator.validate_paths(&selected_files)?;
        
        for batch in validated_files.chunks(self.config.batch_size) {
            self.add_file_batch(batch)?;
        }
        
        info!("Successfully added {} files interactively", selected_files.len());
        self.show_add_summary("Interactively added files").await?;
        
        Ok(())
    }
    
    #[instrument(skip(self, files))]
    async fn add_patch_mode(&mut self, files: Vec<PathBuf>) -> Result<(), AddError> {
        info!("Starting patch mode");
        
        let target_files = if files.is_empty() {
            let status = self.rgit.status()?;
            status.unstaged.iter().map(|f| PathBuf::from(&f.path)).collect()
        } else {
            files
        };
        
        let validated_files = self.validator.validate_paths(&target_files)?;
        
        if validated_files.is_empty() {
            info!("No files to patch");
            return Ok(());
        }
        
        println!("{} Interactive patch mode", "🔍".blue().bold());
        println!("Select hunks to add for each file:\n");
        
        let processor = PatchProcessor::new(&self.rgit.repo, self.config.clone());
        
        let mut total_hunks_added = 0;
        
        for file_path in &validated_files {
            if !file_path.exists() {
                warn!("File not found: {}", file_path.display());
                continue;
            }
            
            match self.process_file_patches(&processor, file_path) {
                Ok(hunks_added) => {
                    total_hunks_added += hunks_added;
                    if hunks_added > 0 {
                        info!("Added {} hunk{} from {}", 
                             hunks_added,
                             if hunks_added == 1 { "" } else { "s" },
                             file_path.display());
                    }
                }
                Err(e) => {
                    error!("Failed to process {}: {}", file_path.display(), e);
                }
            }
        }
        
        if total_hunks_added > 0 {
            info!("Added {} hunk{} total", 
                 total_hunks_added,
                 if total_hunks_added == 1 { "" } else { "s" });
            self.show_add_summary("Added hunks interactively").await?;
        } else {
            info!("No hunks were added");
        }
        
        Ok(())
    }
    
    #[instrument(skip(self, processor, file_path))]
    fn process_file_patches(&self, processor: &PatchProcessor, file_path: &Path) -> Result<usize, AddError> {
        let hunks = processor.get_file_diff(file_path)?;
        
        if hunks.is_empty() {
            debug!("No hunks found for {}", file_path.display());
            return Ok(0);
        }
        
        let selected_indices = processor.interactive_hunk_selection(file_path, &hunks)?;
        
        let mut applied_hunks = 0;
        for &idx in &selected_indices {
            if idx < hunks.len() {
                processor.apply_hunk(file_path, &hunks[idx])?;
                applied_hunks += 1;
                debug!("Applied hunk {} for {}", idx, file_path.display());
            }
        }
        
        Ok(applied_hunks)
    }
    
    // Utility methods
    
    fn add_file_batch(&mut self, files: &[PathBuf]) -> Result<(), AddError> {
        for file in files {
            self.add_single_file(file)?;
        }
        Ok(())
    }
    
    fn add_single_file(&mut self, file_path: &Path) -> Result<(), AddError> {
        let relative_path = file_path.strip_prefix(self.rgit.repo.workdir().unwrap())
            .map_err(|_| AddError::PathTraversal { 
                path: file_path.display().to_string() 
            })?;
        
        let mut index = self.rgit.repo.index()?;
        index.add_path(relative_path)?;
        index.write()?;
        
        debug!("Added file: {}", file_path.display());
        Ok(())
    }
    
    fn is_file_ignored(&self, file_path: &Path) -> Result<bool, AddError> {
        match self.rgit.repo.status_file(file_path) {
            Ok(flags) => Ok(flags.contains(Status::IGNORED)),
            Err(_) => Ok(false),
        }
    }
    
    fn collect_addable_files(&self, status: &crate::core::RepositoryStatus) -> Vec<FileStatus> {
        let mut files = Vec::new();
        files.extend(status.unstaged.clone());
        files.extend(status.untracked.clone());
        files
    }
    
    fn create_file_items(&self, files: &[FileStatus]) -> Result<Vec<FileItem>, AddError> {
        files.iter().map(|file| {
            Ok(FileItem {
                path: PathBuf::from(&file.path),
                status: file.status_symbol(false).to_string(),
                size: Some(file.size),
                selected: false,
            })
        }).collect()
    }
    
    fn confirm_add_all(&self, total_files: usize) -> Result<bool, AddError> {
        InteractivePrompt::new()
            .with_message(&format!("Add all {} files?", total_files))
            .confirm()
            .map_err(|_| AddError::UserCancelled)
    }
    
    fn show_files_preview(&self, unstaged: &[FileStatus], untracked: &[FileStatus]) -> Result<(), AddError> {
        let max_show = self.config.max_preview_files;
        let mut shown = 0;
        
        println!("{} Files to be added:", "📋".blue());
        
        for file in unstaged.iter().take(max_show - shown) {
            println!("  {} {}: {}", 
                    "○".yellow(), 
                    file.status_symbol(false).yellow(),
                    file.path.white());
            shown += 1;
        }
        
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
    
    fn show_status_summary(&self, status: &crate::core::RepositoryStatus) {
        println!("{} Current repository status:", "📋".blue());
        println!("  {} {} unstaged changes", "📝".yellow(), status.unstaged.len());
        println!("  {} {} untracked files", "❓".red(), status.untracked.len());
        println!();
    }
    
    async fn show_add_summary(&self, operation: &str) -> Result<(), AddError> {
        let status = self.rgit.status()?;
        
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
        
        if !status.staged.is_empty() {
            println!("\n{} Next steps:", "💡".blue());
            println!("  • {} - Commit staged changes", "rgit commit".cyan());
            println!("  • {} - Quick commit workflow", "rgit quick-commit".cyan());
        }
        
        Ok(())
    }
    
    fn report_add_results(&self, results: &AddResults) -> Result<(), AddError> {
        if !results.added.is_empty() {
            info!("Successfully added {} file{}", 
                 results.added.len(),
                 if results.added.len() == 1 { "" } else { "s" });
            
            for file in &results.added {
                println!("  {} {}", "✓".green(), file.display().to_string().white());
            }
        }
        
        if !results.missing.is_empty() {
            error!("Missing files ({}): {}", 
                  results.missing.len(),
                  results.missing.iter()
                      .map(|p| p.display().to_string())
                      .collect::<Vec<_>>()
                      .join(", "));
        }
        
        if !results.ignored.is_empty() {
            warn!("Ignored files ({}): {}", 
                 results.ignored.len(),
                 results.ignored.iter()
                     .map(|p| p.display().to_string())
                     .collect::<Vec<_>>()
                     .join(", "));
            
            println!("  💡 Use {} to add ignored files", "--force".cyan());
        }
        
        if !results.failed.is_empty() {
            error!("Failed to add {} files:", results.failed.len());
            for (file, error) in &results.failed {
                error!("  {}: {}", file.display(), error);
            }
        }
        
        Ok(())
    }
}

#[derive(Debug)]
enum OperationMode {
    AddAll,
    AddUpdate,
    AddPatch(Vec<PathBuf>),
    AddSpecific(Vec<PathBuf>, bool),
    Interactive,
}

#[derive(Debug, Default)]
struct AddResults {
    added: Vec<PathBuf>,
    missing: Vec<PathBuf>,
    ignored: Vec<PathBuf>,
    failed: Vec<(PathBuf, String)>,
}

impl AddResults {
    fn new() -> Self {
        Self::default()
    }
}

// Public API
#[instrument(skip(args, rgit, config))]
pub async fn execute(args: &AddArgs, rgit: &mut RgitCore, config: &Config) -> Result<()> {
    let add_config = AddConfig::default();
    let mut executor = AddExecutor::new(rgit, add_config)?;
    
    executor.execute(args).await?;
    
    Ok(())
}

// Utility functions for other commands
pub async fn stage_files(
    rgit: &mut RgitCore, 
    files: &[PathBuf], 
    force: bool
) -> Result<Vec<PathBuf>, AddError> {
    let config = AddConfig::default();
    let validator = PathValidator::new(
        rgit.repo.workdir().unwrap().to_path_buf()
    );
    
    let validated_files = validator.validate_paths(files)?;
    let mut staged = Vec::new();
    
    for file_path in &validated_files {
        if !file_path.exists() {
            continue;
        }
        
        if !force {
            match rgit.repo.status_file(file_path) {
                Ok(flags) if flags.contains(Status::IGNORED) => continue,
                _ => {}
            }
        }
        
        let relative_path = file_path.strip_prefix(rgit.repo.workdir().unwrap())
            .map_err(|_| AddError::PathTraversal { 
                path: file_path.display().to_string() 
            })?;
        
        let mut index = rgit.repo.index()?;
        if index.add_path(relative_path).is_ok() && index.write().is_ok() {
            staged.push(file_path.clone());
        }
    }
    
    Ok(staged)
}

pub fn has_stageable_files(rgit: &RgitCore) -> Result<bool, AddError> {
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
        
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        
        (temp_dir, repo)
    }

    #[tokio::test]
    async fn test_add_specific_files() {
        let (temp_dir, repo) = create_test_repo();
        
        fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "content2").unwrap();
        
        let mut rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let config = AddConfig::default();
        let mut executor = AddExecutor::new(&mut rgit, config).unwrap();
        
        let files = vec![
            temp_dir.path().join("file1.txt"),
            temp_dir.path().join("file2.txt"),
        ];
        
        executor.add_specific_files(files, false).await.unwrap();
        
        let status = executor.rgit.status().unwrap();
        assert_eq!(status.staged.len(), 2);
    }

    #[tokio::test]
    async fn test_path_validation() {
        let (temp_dir, _repo) = create_test_repo();
        let validator = PathValidator::new(temp_dir.path().to_path_buf());
        
        // Test valid path
        let valid_path = temp_dir.path().join("valid.txt");
        fs::write(&valid_path, "content").unwrap();
        assert!(validator.validate_file_path(&valid_path).is_ok());
        
        // Test path traversal attempt - this will fail because canonicalize won't work
        // for non-existent paths, which is what we want for security
        let invalid_path = temp_dir.path().join("../../../etc/passwd");
        assert!(validator.validate_file_path(&invalid_path).is_err());
    }

    #[tokio::test]
    async fn test_patch_processor() {
        let (temp_dir, repo) = create_test_repo();
        
        // Create and commit a file
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "line1\nline2\nline3\n").unwrap();
        
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("test.txt")).unwrap();
        index.write().unwrap();
        
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[]).unwrap();
        
        // Modify the file
        fs::write(&file_path, "line1\nmodified line2\nline3\nnew line4\n").unwrap();
        
        let processor = PatchProcessor::new(&repo, AddConfig::default());
        let hunks = processor.get_file_diff(&file_path).unwrap();
        
        assert!(!hunks.is_empty());
        assert!(hunks[0].lines.iter().any(|l| l.content.contains("modified")));
    }

    #[tokio::test]
    async fn test_stage_files_utility() {
        let (temp_dir, repo) = create_test_repo();
        
        fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
        
        let mut rgit = RgitCore::from_path(repo.workdir().unwrap(), false).unwrap();
        let files = vec![temp_dir.path().join("test.txt")];
        
        let staged = stage_files(&mut rgit, &files, false).await.unwrap();
        assert_eq!(staged.len(), 1);
        
        assert!(!has_stageable_files(&rgit).unwrap()); // File is now staged
    }

    #[test]
    fn test_add_config_defaults() {
        let config = AddConfig::default();
        assert_eq!(config.max_files_per_operation, 10_000);
        assert_eq!(config.max_file_size_bytes, 100 * 1024 * 1024);
        assert!(config.operation_timeout > Duration::from_secs(0));
    }
}