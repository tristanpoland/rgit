use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// A superior Git CLI written in Rust with enhanced UX and intelligent submodule handling
#[derive(Parser, Debug)]
#[command(
    name = "rgit",
    version = env!("CARGO_PKG_VERSION"),
    about = "A superior Git CLI written in Rust",
    long_about = r#"
rgit is a modern Git CLI replacement written in Rust that provides:
• Enhanced user experience with beautiful, colorful output
• Intelligent submodule management with proactive conflict detection
• Interactive prompts and guided workflows
• Safety-first approach with confirmations and undo functionality
• Built-in tutorials and help system

Use 'rgit learn' for interactive tutorials or 'rgit doctor' for health checks.
"#,
    author = "rgit contributors",
    help_template = r#"{before-help}{name} {version}
{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}
"#,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose output with detailed logging
    #[arg(
        short,
        long,
        global = true,
        help = "Show detailed information about operations"
    )]
    pub verbose: bool,

    /// Disable colored output
    #[arg(long, global = true, help = "Disable all colored output")]
    pub no_color: bool,

    /// Use alternative configuration file
    #[arg(
        long,
        global = true,
        value_name = "FILE",
        help = "Use custom configuration file"
    )]
    pub config: Option<PathBuf>,

    /// Set working directory
    #[arg(
        short = 'C',
        long,
        global = true,
        value_name = "PATH",
        help = "Change to directory before executing"
    )]
    pub directory: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    // ===== Repository Management =====
    /// Initialize a new repository with smart defaults and templates
    #[command(visible_alias = "i")]
    Init(InitArgs),

    /// Clone a repository with enhanced progress tracking
    #[command(visible_alias = "cl")]
    Clone(CloneArgs),

    // ===== Core Git Operations =====
    /// Show repository status with enhanced visualization
    #[command(visible_alias = "st")]
    Status(StatusArgs),

    /// Smart add with interactive file selection
    #[command(visible_alias = "a")]
    Add(AddArgs),

    /// Intelligent commit with validation and templates
    #[command(visible_alias = "c")]
    Commit(CommitArgs),

    /// Enhanced push with safety checks and progress
    #[command(visible_alias = "p")]
    Push(PushArgs),

    /// Smart pull with automatic conflict detection
    #[command(visible_alias = "pl")]
    Pull(PullArgs),

    /// Fetch with detailed progress and pruning
    #[command(visible_alias = "f")]
    Fetch(FetchArgs),

    // ===== Branch Management =====
    /// Enhanced branch operations with safety checks
    #[command(visible_alias = "b")]
    Branch(BranchArgs),

    /// Smart checkout with uncommitted change detection
    #[command(visible_alias = "co")]
    Checkout(CheckoutArgs),

    /// Interactive merge with conflict resolution assistance
    #[command(visible_alias = "m")]
    Merge(MergeArgs),

    /// Guided rebase with step-by-step assistance
    #[command(visible_alias = "rb")]
    Rebase(RebaseArgs),

    /// Cherry-pick with conflict handling
    #[command(visible_alias = "cp")]
    CherryPick(CherryPickArgs),

    // ===== History and Information =====
    /// Enhanced log with beautiful formatting and filtering
    #[command(visible_alias = "l")]
    Log(LogArgs),

    /// Smart diff with syntax highlighting and context
    #[command(visible_alias = "d")]
    Diff(DiffArgs),

    /// Show commit details with enhanced formatting
    Show(ShowArgs),

    /// Search through commit history and content
    Grep(GrepArgs),

    /// Show file blame with context and history
    Blame(BlameArgs),

    // ===== Remote Management =====
    /// Manage remotes with URL validation
    #[command(visible_alias = "r")]
    Remote(RemoteArgs),

    // ===== Tag Management =====
    /// Tag operations with GPG signing support
    #[command(visible_alias = "t")]
    Tag(TagArgs),

    // ===== Stash Operations =====
    /// Interactive stash management with descriptions
    #[command(visible_alias = "s")]
    Stash(StashArgs),

    // ===== Submodule Operations =====
    /// Complete submodule management with health checking
    #[command(visible_alias = "sub")]
    Submodule(SubmoduleArgs),

    // ===== Advanced Git Operations =====
    /// Interactive bisect for bug hunting
    Bisect(BisectArgs),

    /// Show reference logs with filtering
    Reflog(ReflogArgs),

    /// Repository maintenance and optimization
    Gc(GcArgs),

    /// File system check with repair options
    Fsck(FsckArgs),

    // ===== Ease-of-Use Commands =====
    /// Quick sync (pull + push) with safety checks
    #[command(visible_alias = "sy")]
    Sync(SyncArgs),

    /// Streamlined commit workflow
    #[command(name = "quick-commit", visible_alias = "qc")]
    QuickCommit(QuickCommitArgs),

    /// Safe undo operations with confirmation
    #[command(visible_alias = "u")]
    Undo(UndoArgs),

    /// Interactive workspace cleaning
    Clean(CleanArgs),

    /// Interactive conflict resolution assistant
    Resolve,

    /// Backup current repository state
    Backup(BackupArgs),

    /// Restore from backup
    Restore(RestoreArgs),

    // ===== Utility Commands =====
    /// Repository health check and diagnostics
    #[command(visible_alias = "doc")]
    Doctor,

    /// Interactive Git tutorials and learning
    Learn(LearnArgs),
}

// ============================================================================
// Command Arguments Definitions
// ============================================================================

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Directory to initialize (default: current directory)
    #[arg(
        value_name = "DIRECTORY",
        help = "Directory to initialize as a Git repository"
    )]
    pub path: Option<PathBuf>,

    /// Skip creating default .gitignore file
    #[arg(long, help = "Do not create a default .gitignore file")]
    pub no_ignore: bool,

    /// Use specific .gitignore template
    #[arg(long, value_enum, help = "Use a specific .gitignore template")]
    pub template: Option<GitignoreTemplate>,

    /// Initialize as bare repository
    #[arg(long, help = "Create a bare repository without a working directory")]
    pub bare: bool,

    /// Set initial branch name
    #[arg(long, value_name = "NAME", help = "Set the initial branch name")]
    pub initial_branch: Option<String>,
}

#[derive(ValueEnum, Debug, Clone)]
pub enum GitignoreTemplate {
    Rust,
    Node,
    Python,
    Go,
    Java,
    Default,
}

#[derive(Args, Debug)]
pub struct CloneArgs {
    /// Repository URL to clone
    #[arg(value_name = "URL", help = "Git repository URL to clone")]
    pub url: String,

    /// Directory name (optional, defaults to repository name)
    #[arg(
        value_name = "DIRECTORY",
        help = "Directory name for the cloned repository"
    )]
    pub directory: Option<String>,

    /// Clone depth for shallow clone
    #[arg(
        long,
        value_name = "DEPTH",
        help = "Create a shallow clone with history truncated to depth"
    )]
    pub depth: Option<u32>,

    /// Clone specific branch only
    #[arg(
        short,
        long,
        value_name = "BRANCH",
        help = "Clone only the specified branch"
    )]
    pub branch: Option<String>,

    /// Clone recursively (including submodules)
    #[arg(long, help = "Initialize and clone submodules recursively")]
    pub recursive: bool,

    /// Use single branch mode
    #[arg(
        long,
        help = "Clone only one branch, either HEAD or specified by --branch"
    )]
    pub single_branch: bool,

    /// Clone with specific protocol
    #[arg(long, value_enum, help = "Force specific protocol for cloning")]
    pub protocol: Option<Protocol>,
}

#[derive(ValueEnum, Debug, Clone)]
pub enum Protocol {
    Https,
    Ssh,
    Git,
}

#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Show ignored files
    #[arg(long, help = "Show ignored files in output")]
    pub ignored: bool,

    /// Use short format output
    #[arg(short, long, help = "Give output in short format")]
    pub short: bool,

    /// Include submodule status
    #[arg(long, help = "Show submodule status")]
    pub submodules: bool,

    /// Show ahead/behind information
    #[arg(long, help = "Show ahead/behind counts for tracking branches")]
    pub ahead_behind: bool,

    /// Include file modification times
    #[arg(long, help = "Show file modification times")]
    pub timestamps: bool,
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Files to add (interactive selection if none specified)
    #[arg(value_name = "FILES", help = "Files to add to the staging area")]
    pub files: Vec<PathBuf>,

    /// Add all changes (tracked and untracked)
    #[arg(short, long, help = "Add all changes in the working directory")]
    pub all: bool,

    /// Add only tracked files that have been modified
    #[arg(short, long, help = "Add changes from tracked files only")]
    pub update: bool,

    /// Force add ignored files
    #[arg(short, long, help = "Allow adding ignored files")]
    pub force: bool,

    /// Add changes interactively by hunks
    #[arg(short, long, help = "Interactively select hunks to add")]
    pub patch: bool,

    /// Add with intent to add (stage for commit without content)
    #[arg(
        short = 'N',
        long,
        help = "Record only that the path will be added later"
    )]
    pub intent_to_add: bool,
}

#[derive(Args, Debug)]
pub struct CommitArgs {
    /// Commit message
    #[arg(short, long, value_name = "MESSAGE", help = "Commit message")]
    pub message: Option<String>,

    /// Use message from file
    #[arg(
        short = 'F',
        long,
        value_name = "FILE",
        help = "Read commit message from file"
    )]
    pub file: Option<PathBuf>,

    /// Amend the last commit
    #[arg(short, long, help = "Amend the previous commit")]
    pub amend: bool,

    /// Skip pre-commit and commit-msg hooks
    #[arg(long, help = "Bypass pre-commit and commit-msg hooks")]
    pub no_verify: bool,

    /// Allow empty commit
    #[arg(long, help = "Allow recording an empty commit")]
    pub allow_empty: bool,

    /// Sign commit with GPG
    #[arg(short = 'S', long, help = "Sign the commit with GPG")]
    pub gpg_sign: bool,

    /// Automatically stage modified files
    #[arg(short, long, help = "Automatically stage modified and deleted files")]
    pub all: bool,

    /// Use commit template
    #[arg(long, help = "Use a commit message template")]
    pub template: bool,
}

#[derive(Args, Debug)]
pub struct PushArgs {
    /// Remote name (default: origin)
    #[arg(value_name = "REMOTE", help = "Remote repository name")]
    pub remote: Option<String>,

    /// Branch name (default: current branch)
    #[arg(value_name = "BRANCH", help = "Branch name to push")]
    pub branch: Option<String>,

    /// Set upstream tracking for the branch
    #[arg(short, long, help = "Set upstream for git pull/status")]
    pub set_upstream: bool,

    /// Force push (use with caution)
    #[arg(
        short,
        long,
        help = "Force the push even if it results in non-fast-forward"
    )]
    pub force: bool,

    /// Force push with lease (safer force push)
    #[arg(long, help = "Force push but fail if remote has unexpected changes")]
    pub force_with_lease: bool,

    /// Push all branches
    #[arg(long, help = "Push all branches to the remote")]
    pub all: bool,

    /// Push tags along with branches
    #[arg(long, help = "Push all tags")]
    pub tags: bool,

    /// Delete remote branch
    #[arg(long, help = "Delete the remote branch")]
    pub delete: bool,
}

#[derive(Args, Debug)]
pub struct SubmoduleArgs {
    #[command(subcommand)]
    pub action: SubmoduleCommands,
}

#[derive(Subcommand, Debug)]
pub enum SubmoduleCommands {
    /// Add a new submodule to the repository
    Add {
        /// Repository URL to add as submodule
        #[arg(value_name = "URL", help = "Git repository URL")]
        url: String,

        /// Path where to place the submodule
        #[arg(value_name = "PATH", help = "Path for the submodule")]
        path: String,

        /// Branch to track in the submodule
        #[arg(short, long, value_name = "BRANCH", help = "Branch to track")]
        branch: Option<String>,

        /// Name for the submodule
        #[arg(long, value_name = "NAME", help = "Name for the submodule")]
        name: Option<String>,

        /// Clone depth for shallow submodule
        #[arg(long, value_name = "DEPTH", help = "Shallow clone depth")]
        depth: Option<u32>,
    },

    /// Initialize submodules
    Init {
        /// Specific submodule paths
        #[arg(value_name = "PATHS", help = "Specific submodules to initialize")]
        paths: Vec<String>,

        /// Initialize all submodules
        #[arg(long, help = "Initialize all submodules")]
        all: bool,
    },

    /// Update submodules to latest commits
    Update {
        /// Specific submodule paths
        #[arg(value_name = "PATHS", help = "Specific submodules to update")]
        paths: Vec<String>,

        /// Initialize submodules if needed
        #[arg(long, help = "Initialize any submodules not yet initialized")]
        init: bool,

        /// Update recursively
        #[arg(long, help = "Update submodules recursively")]
        recursive: bool,

        /// Merge instead of checkout
        #[arg(long, help = "Merge commit into current branch")]
        merge: bool,

        /// Rebase current branch onto commit
        #[arg(long, help = "Rebase current branch onto the commit")]
        rebase: bool,

        /// Use remote tracking branch
        #[arg(long, help = "Update to latest remote commit")]
        remote: bool,

        /// Force update
        #[arg(short, long, help = "Discard local changes when updating")]
        force: bool,
    },

    /// Show submodule status with health information
    Status {
        /// Show recursive status
        #[arg(long, help = "Show status recursively")]
        recursive: bool,

        /// Show detailed health information
        #[arg(long, help = "Show detailed submodule health")]
        health: bool,
    },

    /// Sync submodule URLs from .gitmodules
    Sync {
        /// Specific submodule paths
        #[arg(value_name = "PATHS", help = "Specific submodules to sync")]
        paths: Vec<String>,

        /// Sync recursively
        #[arg(long, help = "Sync submodules recursively")]
        recursive: bool,
    },

    /// Remove a submodule (deinitialize and remove)
    #[command(visible_alias = "rm")]
    Deinit {
        /// Submodule path to remove
        #[arg(value_name = "PATH", help = "Submodule path to deinitialize")]
        path: String,

        /// Force removal even with local changes
        #[arg(short, long, help = "Force removal of submodule")]
        force: bool,

        /// Remove from .gitmodules and .git/config
        #[arg(long, help = "Remove submodule from .gitmodules")]
        remove: bool,
    },

    /// Execute command in each submodule
    Foreach {
        /// Command to execute in each submodule
        #[arg(value_name = "COMMAND", help = "Command to execute")]
        command: String,

        /// Execute recursively in nested submodules
        #[arg(long, help = "Execute recursively")]
        recursive: bool,

        /// Continue on command failure
        #[arg(long, help = "Continue even if command fails")]
        continue_on_error: bool,
    },
}

// Additional command argument structs with comprehensive options...
#[derive(Args, Debug)]
pub struct PullArgs {
    pub remote: Option<String>,
    pub branch: Option<String>,
    #[arg(short, long)]
    pub rebase: bool,
    #[arg(long)]
    pub no_edit: bool,
    #[arg(long)]
    pub no_commit: bool,
    #[arg(short, long)]
    pub force: bool,
}
#[derive(Args, Debug)]
pub struct FetchArgs {
    pub remote: Option<String>,
    #[arg(long)]
    pub all: bool,
    #[arg(long)]
    pub prune: bool,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub tags: bool,
}
#[derive(Args, Debug)]
pub struct BranchArgs {
    pub name: Option<String>,
    #[arg(short, long)]
    pub delete: Option<String>,
    #[arg(short = 'D', long)]
    pub force_delete: Option<String>,
    #[arg(short, long)]
    pub list: bool,
    #[arg(short, long)]
    pub rename: Option<String>,
    #[arg(short, long)]
    pub move_to: Option<String>,
    #[arg(short, long)]
    pub copy: Option<String>,
    #[arg(long)]
    pub merged: bool,
    #[arg(long)]
    pub no_merged: bool,
}
#[derive(Args, Debug)]
pub struct CheckoutArgs {
    pub target: String,
    #[arg(short = 'b', long)]
    pub new_branch: bool,
    #[arg(short = 'B', long)]
    pub force_new_branch: bool,
    #[arg(short, long)]
    pub force: bool,
    #[arg(long)]
    pub track: bool,
    #[arg(long)]
    pub no_track: bool,
}
#[derive(Args, Debug)]
pub struct LogArgs {
    #[arg(short, long, default_value = "10")]
    pub limit: usize,
    #[arg(long)]
    pub oneline: bool,
    #[arg(long)]
    pub graph: bool,
    #[arg(long)]
    pub decorate: bool,
    #[arg(long)]
    pub stat: bool,
    pub file: Option<String>,
    #[arg(long)]
    pub since: Option<String>,
    #[arg(long)]
    pub until: Option<String>,
    #[arg(long)]
    pub author: Option<String>,
}
#[derive(Args, Debug)]
pub struct DiffArgs {
    pub target: Option<String>,
    #[arg(long)]
    pub staged: bool,
    #[arg(long)]
    pub cached: bool,
    pub file: Option<String>,
    #[arg(long)]
    pub word_diff: bool,
    #[arg(long)]
    pub stat: bool,
    #[arg(long)]
    pub name_only: bool,
}
#[derive(Args, Debug)]
pub struct SyncArgs {
    #[arg(long)]
    pub push_only: bool,
    #[arg(long)]
    pub pull_only: bool,
    #[arg(short, long)]
    pub force: bool,
    #[arg(long)]
    pub submodules: bool,
    #[arg(long)]
    pub dry_run: bool,
}
#[derive(Args, Debug)]
pub struct QuickCommitArgs {
    #[arg(short, long)]
    pub message: Option<String>,
    #[arg(short, long)]
    pub all: bool,
    #[arg(short, long)]
    pub push: bool,
    #[arg(long)]
    pub amend: bool,
}
#[derive(Args, Debug)]
pub struct UndoArgs {
    #[arg(short, long, default_value = "1")]
    pub commits: usize,
    #[arg(long)]
    pub operation: Option<String>,
    #[arg(long)]
    pub soft: bool,
    #[arg(long)]
    pub hard: bool,
}
#[derive(Args, Debug)]
pub struct CleanArgs {
    #[arg(short, long)]
    pub force: bool,
    #[arg(short, long)]
    pub ignored: bool,
    #[arg(short, long)]
    pub dry_run: bool,
    #[arg(short, long)]
    pub directories: bool,
    #[arg(short, long)]
    pub interactive: bool,
}
#[derive(Args, Debug)]
pub struct MergeArgs {
    pub branch: String,
    #[arg(long)]
    pub no_ff: bool,
    #[arg(long)]
    pub no_commit: bool,
    #[arg(long)]
    pub squash: bool,
    #[arg(short, long)]
    pub message: Option<String>,
}
#[derive(Args, Debug)]
pub struct RebaseArgs {
    pub target: Option<String>,
    #[arg(short, long)]
    pub interactive: bool,
    #[arg(long)]
    pub continue_rebase: bool,
    #[arg(long)]
    pub abort: bool,
    #[arg(long)]
    pub skip: bool,
}
#[derive(Args, Debug)]
pub struct CherryPickArgs {
    pub commits: Vec<String>,
    #[arg(short, long)]
    pub no_commit: bool,
    #[arg(short, long)]
    pub edit: bool,
    #[arg(long)]
    pub continue_pick: bool,
    #[arg(long)]
    pub abort: bool,
}
#[derive(Args, Debug)]
pub struct ShowArgs {
    pub commit: Option<String>,
    #[arg(long)]
    pub stat: bool,
    #[arg(long)]
    pub name_only: bool,
}
#[derive(Args, Debug)]
pub struct GrepArgs {
    pub pattern: String,
    pub files: Vec<String>,
    #[arg(short, long)]
    pub ignore_case: bool,
    #[arg(short, long)]
    pub line_number: bool,
}
#[derive(Args, Debug)]
pub struct BlameArgs {
    pub file: String,
    #[arg(short, long)]
    pub line_range: Option<String>,
    #[arg(short, long)]
    pub reverse: bool,
}
#[derive(Args, Debug)]
pub struct RemoteArgs {
    #[command(subcommand)]
    pub action: Option<RemoteCommands>,
}
#[derive(Subcommand, Debug)]
pub enum RemoteCommands {
    Add {
        name: String,
        url: String,
        #[arg(short, long)]
        fetch: bool,
    },
    Remove {
        name: String,
    },
    Rename {
        old_name: String,
        new_name: String,
    },
    List {
        #[arg(short, long)]
        verbose: bool,
    },
    Show {
        name: String,
    },
    Prune {
        name: Option<String>,
    },
}
#[derive(Args, Debug)]
pub struct TagArgs {
    #[command(subcommand)]
    pub action: Option<TagCommands>,
}
#[derive(Subcommand, Debug)]
pub enum TagCommands {
    Create {
        name: String,
        commit: Option<String>,
        #[arg(short, long)]
        message: Option<String>,
        #[arg(short, long)]
        sign: bool,
    },
    Delete {
        name: String,
    },
    List {
        pattern: Option<String>,
    },
    Show {
        name: String,
    },
}
#[derive(Args, Debug)]
pub struct StashArgs {
    #[command(subcommand)]
    pub action: Option<StashCommands>,
}
#[derive(Subcommand, Debug)]
pub enum StashCommands {
    Save {
        message: Option<String>,
        #[arg(short, long)]
        include_untracked: bool,
    },
    List,
    Apply {
        index: Option<usize>,
    },
    Pop {
        index: Option<usize>,
    },
    Drop {
        index: Option<usize>,
    },
    Show {
        index: Option<usize>,
    },
    Clear,
}
#[derive(Args, Debug)]
pub struct BisectArgs {
    #[command(subcommand)]
    pub action: BisectCommands,
}
#[derive(Subcommand, Debug)]
pub enum BisectCommands {
    Start,
    Good { commit: Option<String> },
    Bad { commit: Option<String> },
    Reset,
    Skip,
}
#[derive(Args, Debug)]
pub struct ReflogArgs {
    pub reference: Option<String>,
    #[arg(short, long)]
    pub all: bool,
}
#[derive(Args, Debug)]
pub struct GcArgs {
    #[arg(long)]
    pub aggressive: bool,
    #[arg(long)]
    pub prune: bool,
}
#[derive(Args, Debug)]
pub struct FsckArgs {
    #[arg(long)]
    pub full: bool,
    #[arg(long)]
    pub strict: bool,
}
#[derive(Args, Debug)]
pub struct BackupArgs {
    pub name: Option<String>,
    #[arg(long)]
    pub include_untracked: bool,
}
#[derive(Args, Debug)]
pub struct RestoreArgs {
    pub name: String,
    #[arg(short, long)]
    pub force: bool,
}
#[derive(Args, Debug)]
pub struct LearnArgs {
    pub topic: Option<String>,
    #[arg(long)]
    pub interactive: bool,
}
