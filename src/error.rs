use std::path::PathBuf;
use thiserror::Error;

/// Comprehensive error types for rgit operations
#[derive(Error, Debug)]
pub enum RgitError {
    // =========================================================================
    // Repository Errors
    // =========================================================================
    
    #[error("Not in a git repository")]
    NotInRepository,
    
    #[error("Repository is not initialized")]
    RepositoryNotInitialized,
    
    #[error("Repository path does not exist: {0}")]
    RepositoryNotFound(PathBuf),
    
    #[error("Repository is corrupted or damaged")]
    RepositoryCorrupted,
    
    #[error("Repository is in an invalid state: {0}")]
    InvalidRepositoryState(String),

    /// Directory is not empty error
    #[error("Directory '{0}' is not empty")]
    DirectoryNotEmpty(String),
    
    /// Clone operation failed
    #[error("Clone failed: {0}")]
    CloneFailed(String),

    #[error("You have uncommitted changes. Commit or stash them before pulling.")]
    UncommittedChanges,
    
    #[error("No upstream branch configured for the current branch")]
    NoUpstreamBranch,
    
    #[error("Fast-forward merge is not possible")]
    FastForwardNotPossible,
    
    #[error("Merge is not possible")]
    MergeNotPossible,
    
    // =========================================================================
    // File and Index Errors
    // =========================================================================
    
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),
    
    #[error("File is ignored and cannot be added: {0}")]
    FileIgnored(PathBuf),
    
    #[error("Index is locked, another git process may be running")]
    IndexLocked,
    
    #[error("Index is corrupted")]
    IndexCorrupted,
    
    #[error("Cannot add empty directory: {0}")]
    EmptyDirectory(PathBuf),
    
    #[error("Permission denied accessing file: {0}")]
    PermissionDenied(PathBuf),
    
    // =========================================================================
    // Commit Errors
    // =========================================================================
    
    #[error("Commit message cannot be empty")]
    EmptyCommitMessage,
    
    #[error("Nothing to commit, working tree clean")]
    NothingToCommit,
    
    #[error("Cannot amend initial commit")]
    CannotAmendInitialCommit,
    
    #[error("Commit failed: {0}")]
    CommitFailed(String),
    
    #[error("Invalid commit reference: {0}")]
    InvalidCommit(String),
    
    #[error("User identity not configured. Set user.name and user.email")]
    UserIdentityNotConfigured,
    
    // =========================================================================
    // Branch Errors
    // =========================================================================
    
    #[error("Branch not found: {0}")]
    BranchNotFound(String),
    
    #[error("Branch already exists: {0}")]
    BranchAlreadyExists(String),
    
    #[error("Cannot delete current branch: {0}")]
    CannotDeleteCurrentBranch(String),
    
    #[error("Branch has uncommitted changes")]
    BranchHasUncommittedChanges,
    
    #[error("Cannot checkout: {0}")]
    CheckoutFailed(String),
    
    #[error("Detached HEAD state")]
    DetachedHead,

    #[error("Invalid branch name: {0}")]
    InvalidBranchName(String),
    
    // =========================================================================
    // Remote Errors
    // =========================================================================
    
    #[error("No remote configured")]
    NoRemoteConfigured,
    
    #[error("Remote not found: {0}")]
    RemoteNotFound(String),
    
    #[error("Remote already exists: {0}")]
    RemoteAlreadyExists(String),
    
    #[error("Invalid remote URL: {0}")]
    InvalidRemoteUrl(String),
    
    #[error("Push rejected: {0}")]
    PushRejected(String),
    
    #[error("Pull failed: {0}")]
    PullFailed(String),
    
    #[error("Fetch failed: {0}")]
    FetchFailed(String),
    
    // =========================================================================
    // Authentication and Network Errors
    // =========================================================================
    
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("SSH key not found or invalid")]
    SshKeyError,
    
    #[error("Certificate verification failed")]
    CertificateError,
    
    #[error("Connection timeout")]
    ConnectionTimeout,
    
    #[error("Remote server unavailable")]
    RemoteUnavailable,
    
    // =========================================================================
    // Merge and Rebase Errors
    // =========================================================================
    
    #[error("Merge conflict in: {0:?}")]
    MergeConflict(Vec<String>),
    
    #[error("Cannot merge: working tree has uncommitted changes")]
    MergeWorkingTreeDirty,
    
    #[error("Merge aborted")]
    MergeAborted,
    
    #[error("Rebase failed: {0}")]
    RebaseFailed(String),
    
    #[error("Rebase conflict in: {0}")]
    RebaseConflict(String),
    
    #[error("Nothing to rebase")]
    NothingToRebase,
    
    #[error("Cherry-pick failed: {0}")]
    CherryPickFailed(String),
    
    // =========================================================================
    // Submodule Errors
    // =========================================================================
    
    #[error("Submodule error: {0}")]
    SubmoduleError(String),
    
    #[error("Submodule not found: {0}")]
    SubmoduleNotFound(String),
    
    #[error("Submodule not initialized: {0}")]
    SubmoduleNotInitialized(String),
    
    #[error("Submodule has uncommitted changes: {0}")]
    SubmoduleUncommittedChanges(String),
    
    #[error("Submodule URL is invalid: {0}")]
    SubmoduleInvalidUrl(String),
    
    #[error("Submodule operation failed: {0}")]
    SubmoduleOperationFailed(String),
    
    // =========================================================================
    // Stash Errors
    // =========================================================================
    
    #[error("No stash entries found")]
    NoStashEntries,
    
    #[error("Stash index out of range: {0}")]
    StashIndexOutOfRange(usize),
    
    #[error("Nothing to stash")]
    NothingToStash,
    
    #[error("Stash apply failed: {0}")]
    StashApplyFailed(String),
    
    // =========================================================================
    // Tag Errors
    // =========================================================================
    
    #[error("Tag not found: {0}")]
    TagNotFound(String),
    
    #[error("Tag already exists: {0}")]
    TagAlreadyExists(String),
    
    #[error("Invalid tag name: {0}")]
    InvalidTagName(String),
    
    #[error("GPG signing failed: {0}")]
    GpgSigningFailed(String),
    
    // =========================================================================
    // Configuration Errors
    // =========================================================================
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Invalid configuration value for {key}: {value}")]
    InvalidConfigValue { key: String, value: String },
    
    #[error("Configuration file not found: {0}")]
    ConfigFileNotFound(PathBuf),
    
    #[error("Permission denied reading configuration")]
    ConfigPermissionDenied,
    
    // =========================================================================
    // Operation Errors
    // =========================================================================
    
    #[error("Operation cancelled by user")]
    OperationCancelled,
    
    #[error("Operation not supported: {0}")]
    OperationNotSupported(String),
    
    #[error("Operation failed: {0}")]
    OperationFailed(String),
    
    #[error("Interactive operation not available in non-TTY environment")]
    NonInteractiveEnvironment,
    
    #[error("Command execution failed: {0}")]
    CommandExecutionFailed(String),
    
    // =========================================================================
    // Validation Errors
    // =========================================================================
    
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    
    #[error("Invalid path: {0}")]
    InvalidPath(PathBuf),
    
    #[error("Invalid reference: {0}")]
    InvalidReference(String),
    
    #[error("Invalid object ID: {0}")]
    InvalidObjectId(String),
    
    #[error("Path is outside repository: {0}")]
    PathOutsideRepository(PathBuf),
    
    // =========================================================================
    // I/O and System Errors
    // =========================================================================
    
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("File system error: {0}")]
    FileSystemError(String),
    
    #[error("Disk space insufficient")]
    InsufficientDiskSpace,
    
    #[error("Temporary file creation failed")]
    TempFileCreationFailed,
    
    // =========================================================================
    // Parse and Format Errors
    // =========================================================================
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Invalid date format: {0}")]
    InvalidDateFormat(String),
    
    #[error("Invalid time format: {0}")]
    InvalidTimeFormat(String),
    
    #[error("Encoding error: {0}")]
    EncodingError(String),
    
    // =========================================================================
    // External Tool Errors
    // =========================================================================
    
    #[error("External editor failed: {0}")]
    ExternalEditorFailed(String),
    
    #[error("Diff tool failed: {0}")]
    DiffToolFailed(String),
    
    #[error("Merge tool failed: {0}")]
    MergeToolFailed(String),
    
    #[error("GPG tool not found or failed")]
    GpgToolFailed,
    
    // =========================================================================
    // Wrapped External Errors
    // =========================================================================
    
    #[error("Git2 library error: {0}")]
    Git2Error(#[from] git2::Error),
    
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("TOML parsing error: {0}")]
    TomlError(#[from] toml::de::Error),
    
    #[error("Regular expression error: {0}")]
    RegexError(#[from] regex::Error),
    
    #[error("UTF-8 encoding error: {0}")]
    Utf8Error(#[from] std::str::Utf8Error),
    
    #[error("Date/time error: {0}")]
    ChronoError(#[from] chrono::format::ParseError),
}

/// Result type alias for rgit operations
pub type RgitResult<T> = Result<T, RgitError>;

impl RgitError {
    /// Check if this error suggests a recoverable operation
    pub fn is_recoverable(&self) -> bool {
        match self {
            // Recoverable errors that user can fix
            RgitError::EmptyCommitMessage
            | RgitError::UserIdentityNotConfigured
            | RgitError::BranchHasUncommittedChanges
            | RgitError::MergeWorkingTreeDirty
            | RgitError::AuthenticationError(_)
            | RgitError::NetworkError(_)
            | RgitError::ConfigurationError(_)
            | RgitError::OperationCancelled => true,
            
            // Conflict errors that can be resolved
            RgitError::MergeConflict(_)
            | RgitError::RebaseConflict(_)
            | RgitError::StashApplyFailed(_) => true,
            
            // Validation errors that can be corrected
            RgitError::InvalidArgument(_)
            | RgitError::InvalidPath(_)
            | RgitError::InvalidReference(_)
            | RgitError::FileNotFound(_) => true,
            
            // Non-recoverable errors
            _ => false,
        }
    }

    /// Get suggested recovery actions for this error
    pub fn recovery_suggestions(&self) -> Vec<&'static str> {
        match self {
            RgitError::NotInRepository => vec![
                "Navigate to a git repository directory",
                "Run 'rgit init' to create a new repository",
            ],
            RgitError::UserIdentityNotConfigured => vec![
                "Set your name: git config user.name \"Your Name\"",
                "Set your email: git config user.email \"your@email.com\"",
            ],
            RgitError::EmptyCommitMessage => vec![
                "Provide a meaningful commit message",
                "Use 'rgit commit -m \"your message\"'",
            ],
            RgitError::MergeConflict(_) => vec![
                "Use 'rgit resolve' for interactive conflict resolution",
                "Edit conflicted files manually and then 'rgit add' them",
                "Use 'rgit status' to see all conflicts",
            ],
            RgitError::BranchHasUncommittedChanges => vec![
                "Commit your changes: 'rgit commit'",
                "Stash your changes: 'rgit stash save'",
                "Discard changes: 'rgit checkout -- .'",
            ],
            RgitError::AuthenticationError(_) => vec![
                "Check your credentials",
                "Set up SSH keys for authentication",
                "Use 'rgit doctor' to verify configuration",
            ],
            RgitError::NetworkError(_) => vec![
                "Check your internet connection",
                "Verify the remote repository URL",
                "Try again later if the remote server is temporarily unavailable",
            ],
            RgitError::SubmoduleUncommittedChanges(_) => vec![
                "Commit changes in the submodule",
                "Use 'rgit submodule status' to see all submodule states",
                "Stash submodule changes if needed",
            ],
            RgitError::NoRemoteConfigured => vec![
                "Add a remote: 'rgit remote add origin <url>'",
                "Clone from a remote repository instead",
            ],
            _ => vec!["Use 'rgit doctor' for diagnostics", "Check 'rgit --help' for usage"],
        }
    }

    /// Get the error category for grouping similar errors
    pub fn category(&self) -> ErrorCategory {
        match self {
            RgitError::NotInRepository
            | RgitError::RepositoryNotInitialized
            | RgitError::RepositoryNotFound(_)
            | RgitError::RepositoryCorrupted
            | RgitError::InvalidRepositoryState(_) => ErrorCategory::Repository,
            
            RgitError::FileNotFound(_)
            | RgitError::FileIgnored(_)
            | RgitError::IndexLocked
            | RgitError::IndexCorrupted
            | RgitError::EmptyDirectory(_)
            | RgitError::PermissionDenied(_) => ErrorCategory::FileSystem,
            
            RgitError::EmptyCommitMessage
            | RgitError::NothingToCommit
            | RgitError::CannotAmendInitialCommit
            | RgitError::CommitFailed(_)
            | RgitError::InvalidCommit(_)
            | RgitError::UserIdentityNotConfigured => ErrorCategory::Commit,
            
            RgitError::BranchNotFound(_)
            | RgitError::BranchAlreadyExists(_)
            | RgitError::CannotDeleteCurrentBranch(_)
            | RgitError::BranchHasUncommittedChanges
            | RgitError::CheckoutFailed(_)
            | RgitError::DetachedHead => ErrorCategory::Branch,
            
            RgitError::NoRemoteConfigured
            | RgitError::RemoteNotFound(_)
            | RgitError::RemoteAlreadyExists(_)
            | RgitError::InvalidRemoteUrl(_)
            | RgitError::PushRejected(_)
            | RgitError::PullFailed(_)
            | RgitError::FetchFailed(_) => ErrorCategory::Remote,
            
            RgitError::AuthenticationError(_)
            | RgitError::NetworkError(_)
            | RgitError::SshKeyError
            | RgitError::CertificateError
            | RgitError::ConnectionTimeout
            | RgitError::RemoteUnavailable => ErrorCategory::Network,
            
            RgitError::MergeConflict(_)
            | RgitError::MergeWorkingTreeDirty
            | RgitError::MergeAborted
            | RgitError::RebaseFailed(_)
            | RgitError::RebaseConflict(_)
            | RgitError::NothingToRebase
            | RgitError::CherryPickFailed(_) => ErrorCategory::Merge,
            
            RgitError::SubmoduleError(_)
            | RgitError::SubmoduleNotFound(_)
            | RgitError::SubmoduleNotInitialized(_)
            | RgitError::SubmoduleUncommittedChanges(_)
            | RgitError::SubmoduleInvalidUrl(_)
            | RgitError::SubmoduleOperationFailed(_) => ErrorCategory::Submodule,
            
            RgitError::ConfigurationError(_)
            | RgitError::InvalidConfigValue { .. }
            | RgitError::ConfigFileNotFound(_)
            | RgitError::ConfigPermissionDenied => ErrorCategory::Configuration,
            
            _ => ErrorCategory::Other,
        }
    }

    /// Check if this error should trigger a help message
    pub fn show_help(&self) -> bool {
        match self {
            RgitError::NotInRepository
            | RgitError::UserIdentityNotConfigured
            | RgitError::NoRemoteConfigured
            | RgitError::InvalidArgument(_) => true,
            _ => false,
        }
    }
}

/// Categories for grouping similar error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Repository,
    FileSystem,
    Commit,
    Branch,
    Remote,
    Network,
    Merge,
    Submodule,
    Configuration,
    Other,
}

impl ErrorCategory {
    pub fn icon(&self) -> &'static str {
        match self {
            ErrorCategory::Repository => "🏗️",
            ErrorCategory::FileSystem => "📁",
            ErrorCategory::Commit => "📝",
            ErrorCategory::Branch => "🌿",
            ErrorCategory::Remote => "🌐",
            ErrorCategory::Network => "📡",
            ErrorCategory::Merge => "🔀",
            ErrorCategory::Submodule => "📦",
            ErrorCategory::Configuration => "⚙️",
            ErrorCategory::Other => "❓",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ErrorCategory::Repository => "Repository",
            ErrorCategory::FileSystem => "File System",
            ErrorCategory::Commit => "Commit",
            ErrorCategory::Branch => "Branch",
            ErrorCategory::Remote => "Remote",
            ErrorCategory::Network => "Network",
            ErrorCategory::Merge => "Merge/Rebase",
            ErrorCategory::Submodule => "Submodule",
            ErrorCategory::Configuration => "Configuration",
            ErrorCategory::Other => "Other",
        }
    }
}

/// Helper trait for converting git2 errors to more specific rgit errors
pub trait Git2ErrorExt {
    fn into_rgit_error(self) -> RgitError;
}

impl Git2ErrorExt for git2::Error {
    fn into_rgit_error(self) -> RgitError {
        match self.class() {
            git2::ErrorClass::Repository => {
                if self.message().contains("not found") {
                    RgitError::RepositoryNotFound(std::env::current_dir().unwrap_or_default())
                } else {
                    RgitError::RepositoryCorrupted
                }
            }
            git2::ErrorClass::Index => RgitError::IndexCorrupted,
            git2::ErrorClass::Object => RgitError::InvalidObjectId(self.message().to_string()),
            git2::ErrorClass::Reference => RgitError::InvalidReference(self.message().to_string()),
            git2::ErrorClass::Net => RgitError::NetworkError(self.message().to_string()),
            git2::ErrorClass::Ssh => RgitError::SshKeyError,
            git2::ErrorClass::Ssl => RgitError::CertificateError,
            git2::ErrorClass::Merge => {
                if self.message().contains("conflict") {
                    RgitError::MergeConflict(vec![self.message().to_string()])
                } else {
                    RgitError::OperationFailed(self.message().to_string())
                }
            }
            _ => RgitError::Git2Error(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categorization() {
        assert_eq!(RgitError::NotInRepository.category(), ErrorCategory::Repository);
        assert_eq!(RgitError::FileNotFound(PathBuf::new()).category(), ErrorCategory::FileSystem);
        assert_eq!(RgitError::MergeConflict(vec![]).category(), ErrorCategory::Merge);
    }

    #[test]
    fn test_recoverable_errors() {
        assert!(RgitError::EmptyCommitMessage.is_recoverable());
        assert!(RgitError::MergeConflict(vec![]).is_recoverable());
        assert!(!RgitError::RepositoryCorrupted.is_recoverable());
    }

    #[test]
    fn test_recovery_suggestions() {
        let suggestions = RgitError::NotInRepository.recovery_suggestions();
        assert!(!suggestions.is_empty());
        assert!(suggestions[0].contains("repository"));
    }

    #[test]
    fn test_error_category_properties() {
        let category = ErrorCategory::Repository;
        assert_eq!(category.icon(), "🏗️");
        assert_eq!(category.description(), "Repository");
    }
}