# 🦀 rgit - A Superior Git CLI in Rust

A complete, modular Git CLI replacement with enhanced UX, intelligent submodule handling, and ease-of-use commands.

## 🚀 Features

### **Superior UX**
- 🎨 Beautiful, colorful output with emojis and clear visual hierarchy
- 🤖 Interactive prompts with smart defaults and multi-select options
- 📊 Enhanced status displays with file sizes and modification info
- ✅ Helpful success/error messages with actionable suggestions

### **Complete Git Functionality**
- 📁 **Core Operations**: init, clone, add, commit, push, pull, fetch
- 🌿 **Branch Management**: create, delete, rename, merge, rebase
- 🏷️ **Tag Operations**: create, list, delete with GPG signing
- 📦 **Stash Management**: save, list, apply, pop, drop with descriptions
- 🔄 **Remote Management**: add, remove, list, show with validation
- 🔍 **History & Search**: log, diff, show, grep, blame with syntax highlighting

### **Intelligent Submodule Handling**
- 🧠 **Smart Detection**: Automatically detects submodule states and issues
- ⚠️ **Proactive Warnings**: Alerts about uncommitted changes before operations
- 🛠️ **Auto-Fix**: Interactive resolution of common submodule problems
- 🔄 **Recursive Operations**: Supports recursive submodule operations
- 📋 **Detailed Status**: Enhanced submodule status with health indicators

### **Ease-of-Use Commands**
- ⚡ **Quick Workflows**: `sync`, `quick-commit`, `undo` commands
- 🔧 **Auto-Resolution**: Interactive conflict resolution assistance  
- 🏥 **Health Checks**: `doctor` command for repository diagnostics
- 🎓 **Learning Mode**: Interactive tutorials for Git concepts
- 💾 **Backup/Restore**: Safe backup and restore operations

## 📦 Installation

### Prerequisites
```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install required system dependencies (Ubuntu/Debian)
sudo apt install libgit2-dev pkg-config libssl-dev

# Install required system dependencies (macOS)
brew install libgit2 pkg-config openssl
```

### Build from Source
```bash
# Clone the repository
git clone https://github.com/yourusername/rgit.git
cd rgit

# Build release version
cargo build --release

# Install globally
cargo install --path .

# Verify installation
rgit --help
```

### Alternative: Install from crates.io (when published)
```bash
cargo install rgit
```

## 🎯 Quick Start

### Basic Workflow
```bash
# Initialize a new repository with smart defaults
rgit init

# Enhanced status with visual indicators
rgit status

# Interactive file selection for staging
rgit add

# Smart commit with prompts
rgit commit

# One-command sync (pull + push)
rgit sync
```

### Submodule Operations
```bash
# Add a submodule with interactive setup
rgit submodule add https://github.com/user/repo.git path/to/submodule

# Smart submodule status with health check
rgit submodule status --recursive

# Update all submodules with conflict handling
rgit submodule update --init --recursive

# Execute command in all submodules
rgit submodule foreach "git status"
```

### Advanced Workflows
```bash
# Quick commit and push
rgit quick-commit -m "fix: typo" --push

# Interactive conflict resolution
rgit resolve

# Repository health check
rgit doctor

# Undo last operation safely
rgit undo

# Learn Git interactively
rgit learn
```

## 📋 Command Reference

### Core Commands
| Command | Description | Example |
|---------|-------------|---------|
| `init` | Initialize repository with templates | `rgit init --template rust` |
| `clone` | Clone with progress and options | `rgit clone --recursive --depth 1 <url>` |
| `status` | Enhanced status with submodules | `rgit status --submodules` |
| `add` | Interactive staging | `rgit add --patch` |
| `commit` | Smart commit with validation | `rgit commit --amend` |
| `push` | Safe push with upstream tracking | `rgit push --set-upstream` |
| `pull` | Smart pull with conflict detection | `rgit pull --rebase` |

### Branch Management
| Command | Description | Example |
|---------|-------------|---------|
| `branch` | List/create/delete branches | `rgit branch feature/new-ui` |
| `checkout` | Safe checkout with checks | `rgit checkout -b feature/auth` |
| `merge` | Interactive merge | `rgit merge --no-ff develop` |
| `rebase` | Guided rebase | `rgit rebase --interactive main` |

### Submodule Commands
| Command | Description | Example |
|---------|-------------|---------|
| `submodule add` | Add new submodule | `rgit submodule add <url> <path>` |
| `submodule init` | Initialize submodules | `rgit submodule init` |
| `submodule update` | Update submodules | `rgit submodule update --recursive` |
| `submodule status` | Show submodule status | `rgit submodule status --recursive` |
| `submodule sync` | Sync submodule URLs | `rgit submodule sync --recursive` |

### Ease-of-Use Commands
| Command | Description | Example |
|---------|-------------|---------|
| `sync` | Pull and push in one command | `rgit sync --submodules` |
| `quick-commit` | Fast commit workflow | `rgit quick-commit -a -p` |
| `undo` | Safe undo operations | `rgit undo --commits 2` |
| `clean` | Interactive cleanup | `rgit clean --dry-run` |
| `doctor` | Repository health check | `rgit doctor` |
| `learn` | Interactive tutorials | `rgit learn branching` |

## ⚙️ Configuration

### Global Configuration
```bash
# Set user information
git config --global user.name "Your Name"
git config --global user.email "your.email@example.com"

# Enable colored output (default)
git config --global color.ui auto

# Set default editor
git config --global core.editor "code --wait"
```

### rgit-Specific Settings
```bash
# Disable colored output globally
rgit --no-color status

# Enable verbose logging
rgit --verbose sync
```

## 🛡️ Safety Features

### Submodule Safety
- **Pre-operation Checks**: Warns about uncommitted submodule changes
- **Interactive Resolution**: Offers solutions for submodule conflicts  
- **Recursive Awareness**: Handles nested submodule scenarios
- **State Preservation**: Safely stashes changes when needed

### General Safety
- **Confirmation Prompts**: Asks before destructive operations
- **Dry Run Options**: Preview changes before applying
- **Backup Integration**: Automatic backups before major changes
- **Undo Functionality**: Safe reversal of recent operations

## 🎨 Customization

### Color Themes
```bash
# Disable colors entirely
export RGIT_NO_COLOR=1

# Custom color scheme (future feature)
export RGIT_THEME=dark
```

### Output Format
```bash
# Compact status output
rgit status --short

# Detailed logging
rgit --verbose <command>
```

## 🔧 Troubleshooting

### Common Issues

**Submodule not updating:**
```bash
# Check submodule health
rgit submodule status
rgit doctor

# Force update
rgit submodule update --init --recursive --force
```

**Permission errors:**
```bash
# Check repository permissions
rgit doctor

# Fix with proper credentials
git config credential.helper store
```

**Merge conflicts:**
```bash
# Interactive conflict resolution
rgit resolve

# Or use traditional approach
rgit status  # Shows conflicted files
# Edit files manually
rgit add <resolved-files>
rgit commit
```

### Getting Help
```bash
# General help
rgit --help

# Command-specific help
rgit <command> --help

# Interactive learning
rgit learn

# Repository diagnostics
rgit doctor
```

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup
```bash
# Clone repository
git clone https://github.com/yourusername/rgit.git
cd rgit

# Install development dependencies
cargo build

# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run lints
cargo clippy
```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [git2-rs](https://github.com/rust-lang/git2-rs) for Git operations
- CLI interface powered by [clap](https://github.com/clap-rs/clap)
- Interactive prompts via [dialoguer](https://github.com/mitsuhiko/dialoguer)
- Inspired by the need for better Git UX and modern CLI design principles

---

**Made with ❤️ and 🦀 by the rgit team**