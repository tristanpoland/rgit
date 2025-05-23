use anyhow::Result;
use colored::*;
use git2::Repository;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::{InitArgs, GitignoreTemplate};
use crate::config::Config;
use crate::error::RgitError;
use crate::interactive::InteractivePrompt;

/// Execute the init command
pub async fn execute(args: &InitArgs, config: &Config) -> Result<()> {
    let target_path = get_target_path(args)?;
    
    // Show initialization preview
    show_init_preview(&target_path, args, config)?;
    
    // Confirm if directory exists and is not empty
    if target_path.exists() && !is_directory_empty(&target_path)? {
        if !confirm_init_existing_directory(&target_path, config)? {
            println!("{} Initialization cancelled", "ℹ️".blue());
            return Ok(());
        }
    }
    
    // Create the repository
    let repo = create_repository(&target_path, args)?;
    
    // Setup initial configuration
    setup_initial_config(&repo, args, config)?;
    
    // Create .gitignore if requested
    if !args.no_ignore {
        create_gitignore_file(&target_path, args.template.as_ref(), config)?;
    }
    
    // Create initial files and structure
    create_initial_structure(&target_path, args, config)?;
    
    // Show success message and next steps
    show_init_success(&target_path, args, config)?;
    
    Ok(())
}

/// Get the target path for initialization
fn get_target_path(args: &InitArgs) -> Result<PathBuf> {
    let path = args.path.as_ref()
        .map(|p| PathBuf::from(p))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    
    // Resolve to absolute path
    let absolute_path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    
    Ok(absolute_path)
}

/// Show initialization preview
fn show_init_preview(target_path: &Path, args: &InitArgs, config: &Config) -> Result<()> {
    if !config.ui.interactive {
        return Ok(());
    }
    
    println!("{} Git Repository Initialization", "🎯".blue().bold());
    println!();
    
    println!("  {} {}", "Directory:".bold(), target_path.display().to_string().cyan());
    println!("  {} {}", "Type:".bold(), 
            if args.bare { "Bare repository".yellow() } else { "Standard repository".green() });
    
    if let Some(ref branch) = args.initial_branch {
        println!("  {} {}", "Initial branch:".bold(), branch.green());
    }
    
    if let Some(ref template) = args.template {
        println!("  {} {:?} template", "Gitignore:".bold(), template);
    } else if !args.no_ignore {
        println!("  {} Default template", "Gitignore:".bold());
    } else {
        println!("  {} None", "Gitignore:".bold());
    }
    
    println!();
    Ok(())
}

/// Check if directory is empty
fn is_directory_empty(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(true);
    }
    
    if !path.is_dir() {
        return Err(RgitError::InvalidPath(path.to_path_buf()).into());
    }
    
    let entries = fs::read_dir(path)?;
    Ok(entries.count() == 0)
}

/// Confirm initialization in existing non-empty directory
fn confirm_init_existing_directory(path: &Path, config: &Config) -> Result<bool> {
    if !config.is_interactive() {
        return Ok(true);
    }
    
    // Check what's in the directory
    let entries: Vec<_> = fs::read_dir(path)?
        .filter_map(|entry| entry.ok())
        .take(5) // Show first 5 entries
        .collect();
    
    println!("{} Directory is not empty:", "⚠️".yellow());
    for entry in &entries {
        println!("  {} {}", "•".dimmed(), entry.file_name().to_string_lossy().white());
    }
    
    let entry_count = fs::read_dir(path)?.count();
    if entry_count > 5 {
        println!("  {} and {} more files/directories...", "...".dimmed(), entry_count - 5);
    }
    
    println!();
    
    InteractivePrompt::new()
        .with_message(&format!("Initialize Git repository in {}?", path.display()))
        .confirm()
}

/// Create the Git repository
fn create_repository(path: &Path, args: &InitArgs) -> Result<Repository> {
    // Ensure directory exists
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    
    let repo = if args.bare {
        Repository::init_bare(path)?
    } else {
        Repository::init(path)?
    };
    
    println!("{} Initialized {} Git repository in {}", 
            "✅".green(),
            if args.bare { "bare" } else { "empty" },
            repo.path().display().to_string().cyan());
    
    Ok(repo)
}

/// Setup initial repository configuration
fn setup_initial_config(repo: &Repository, args: &InitArgs, config: &Config) -> Result<()> {
    let mut repo_config = repo.config()?;
    
    // Set initial branch name if specified
    if let Some(ref branch_name) = args.initial_branch {
        repo_config.set_str("init.defaultBranch", branch_name)?;
        println!("  {} Set initial branch to '{}'", "🌿".green(), branch_name.cyan());
    } else if let Ok(global_config) = git2::Config::open_default() {
        // Check if global default branch is set
        if let Ok(default_branch) = global_config.get_string("init.defaultBranch") {
            println!("  {} Using default branch '{}'", "🌿".blue(), default_branch.cyan());
        }
    }
    
    // Apply any template-specific configuration
    if let Some(ref template_type) = args.template {
        apply_template_config(repo, template_type, config)?;
    }
    
    // Set up recommended configuration for new repositories
    setup_recommended_config(repo, config)?;
    
    Ok(())
}

/// Apply template-specific configuration
fn apply_template_config(repo: &Repository, template: &GitignoreTemplate, _config: &Config) -> Result<()> {
    let mut repo_config = repo.config()?;
    
    match template {
        GitignoreTemplate::Rust => {
            // Rust-specific configuration
            repo_config.set_str("core.autocrlf", "false")?;
            println!("  {} Applied Rust project configuration", "🦀".yellow());
        }
        GitignoreTemplate::Node => {
            // Node.js-specific configuration
            repo_config.set_str("core.ignorecase", "true")?;
            println!("  {} Applied Node.js project configuration", "📦".green());
        }
        GitignoreTemplate::Python => {
            // Python-specific configuration
            repo_config.set_str("core.autocrlf", "false")?;
            println!("  {} Applied Python project configuration", "🐍".blue());
        }
        GitignoreTemplate::Go => {
            // Go-specific configuration
            repo_config.set_str("core.autocrlf", "false")?;
            println!("  {} Applied Go project configuration", "🔵".cyan());
        }
        GitignoreTemplate::Java => {
            // Java-specific configuration
            repo_config.set_str("core.autocrlf", "true")?;
            println!("  {} Applied Java project configuration", "☕".yellow());
        }
        GitignoreTemplate::Default => {
            // Default configuration
            println!("  {} Applied default configuration", "⚙️".blue());
        }
    }
    
    Ok(())
}

/// Setup recommended configuration for new repositories
fn setup_recommended_config(repo: &Repository, _config: &Config) -> Result<()> {
    let mut repo_config = repo.config()?;
    
    // Set up recommended core settings
    repo_config.set_bool("core.precomposeUnicode", true)?;
    repo_config.set_bool("core.quotePath", false)?;
    
    // Platform-specific settings
    if cfg!(windows) {
        repo_config.set_str("core.autocrlf", "true")?;
    } else {
        repo_config.set_str("core.autocrlf", "input")?;
    }
    
    println!("  {} Applied recommended Git configuration", "⚙️".blue());
    Ok(())
}

/// Create .gitignore file
fn create_gitignore_file(path: &Path, template: Option<&GitignoreTemplate>, config: &Config) -> Result<()> {
    let gitignore_path = path.join(".gitignore");
    
    // Don't overwrite existing .gitignore
    if gitignore_path.exists() {
        if config.ui.interactive {
            println!("  {} .gitignore already exists, skipping", "ℹ️".blue());
        }
        return Ok(());
    }
    
    let content = get_gitignore_content(template.unwrap_or(&GitignoreTemplate::Default))?;
    fs::write(&gitignore_path, content)?;
    
    let template_name = template
        .map(|t| format!("{:?}", t).to_lowercase())
        .unwrap_or_else(|| "default".to_string());
    
    println!("  {} Created .gitignore with {} template", "📝".green(), template_name.cyan());
    
    Ok(())
}

/// Get .gitignore content based on template
fn get_gitignore_content(template: &GitignoreTemplate) -> Result<String> {
    let content = match template {
        GitignoreTemplate::Rust => include_str!("../templates/rust.gitignore"),
        GitignoreTemplate::Node => include_str!("../templates/node.gitignore"),
        GitignoreTemplate::Python => include_str!("../templates/python.gitignore"),
        GitignoreTemplate::Go => include_str!("../templates/go.gitignore"),
        GitignoreTemplate::Java => include_str!("../templates/java.gitignore"),
        GitignoreTemplate::Default => include_str!("../templates/default.gitignore"),
    };
    
    Ok(content.to_string())
}

/// Create initial repository structure and files
fn create_initial_structure(path: &Path, args: &InitArgs, config: &Config) -> Result<()> {
    if args.bare {
        // Bare repositories don't need working directory structure
        return Ok(());
    }
    
    // Create README.md if it doesn't exist
    create_readme_file(path, config)?;
    
    // Create basic directory structure for certain templates
    if let Some(ref template) = args.template {
        create_template_structure(path, template, config)?;
    }
    
    Ok(())
}

/// Create README.md file
fn create_readme_file(path: &Path, config: &Config) -> Result<()> {
    let readme_path = path.join("README.md");
    
    if readme_path.exists() {
        return Ok(());
    }
    
    let project_name = path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Project");
    
    let readme_content = format!(r#"# {}

A new project initialized with rgit.

## Getting Started

This repository was created with [rgit](https://github.com/yourusername/rgit), a superior Git CLI written in Rust.

## Usage

Add your project description and usage instructions here.

## Contributing

1. Fork the repository
2. Create a feature branch (`rgit checkout -b feature/amazing-feature`)
3. Commit your changes (`rgit commit -m 'Add amazing feature'`)
4. Push to the branch (`rgit push origin feature/amazing-feature`)
5. Open a Pull Request

## License

Add your license information here.
"#, project_name);
    
    fs::write(&readme_path, readme_content)?;
    
    if config.ui.interactive {
        println!("  {} Created README.md", "📖".green());
    }
    
    Ok(())
}

/// Create template-specific directory structure
fn create_template_structure(path: &Path, template: &GitignoreTemplate, config: &Config) -> Result<()> {
    match template {
        GitignoreTemplate::Rust => {
            create_rust_structure(path, config)?;
        }
        GitignoreTemplate::Node => {
            create_node_structure(path, config)?;
        }
        GitignoreTemplate::Python => {
            create_python_structure(path, config)?;
        }
        GitignoreTemplate::Go => {
            create_go_structure(path, config)?;
        }
        GitignoreTemplate::Java => {
            create_java_structure(path, config)?;
        }
        GitignoreTemplate::Default => {
            // No specific structure for default template
        }
    }
    
    Ok(())
}

/// Create Rust project structure
fn create_rust_structure(path: &Path, config: &Config) -> Result<()> {
    // Create src directory
    let src_dir = path.join("src");
    if !src_dir.exists() {
        fs::create_dir(&src_dir)?;
        
        // Create main.rs
        let main_rs = src_dir.join("main.rs");
        fs::write(&main_rs, r#"fn main() {
    println!("Hello, world!");
}
"#)?;
        
        if config.ui.interactive {
            println!("  {} Created Rust project structure", "🦀".yellow());
        }
    }
    
    // Create Cargo.toml
    let cargo_toml = path.join("Cargo.toml");
    if !cargo_toml.exists() {
        let project_name = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("my-project");
        
        let cargo_content = format!(r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
"#, project_name);
        
        fs::write(&cargo_toml, cargo_content)?;
    }
    
    Ok(())
}

/// Create Node.js project structure
fn create_node_structure(path: &Path, config: &Config) -> Result<()> {
    // Create package.json
    let package_json = path.join("package.json");
    if !package_json.exists() {
        let project_name = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("my-project");
        
        let package_content = format!(r#"{{
  "name": "{}",
  "version": "1.0.0",
  "description": "",
  "main": "index.js",
  "scripts": {{
    "test": "echo \"Error: no test specified\" && exit 1"
  }},
  "keywords": [],
  "author": "",
  "license": "ISC"
}}
"#, project_name);
        
        fs::write(&package_json, package_content)?;
        
        if config.ui.interactive {
            println!("  {} Created Node.js project structure", "📦".green());
        }
    }
    
    // Create index.js
    let index_js = path.join("index.js");
    if !index_js.exists() {
        fs::write(&index_js, r#"console.log('Hello, world!');
"#)?;
    }
    
    Ok(())
}

/// Create Python project structure
fn create_python_structure(path: &Path, config: &Config) -> Result<()> {
    // Create main.py
    let main_py = path.join("main.py");
    if !main_py.exists() {
        fs::write(&main_py, r#"#!/usr/bin/env python3

def main():
    print("Hello, world!")

if __name__ == "__main__":
    main()
"#)?;
        
        if config.ui.interactive {
            println!("  {} Created Python project structure", "🐍".blue());
        }
    }
    
    // Create requirements.txt
    let requirements_txt = path.join("requirements.txt");
    if !requirements_txt.exists() {
        fs::write(&requirements_txt, "# Add your dependencies here\n")?;
    }
    
    Ok(())
}

/// Create Go project structure
fn create_go_structure(path: &Path, config: &Config) -> Result<()> {
    // Create main.go
    let main_go = path.join("main.go");
    if !main_go.exists() {
        let project_name = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("main");
        
        let main_content = format!(r#"package main

import "fmt"

func main() {{
    fmt.Println("Hello, world!")
}}
"#);
        
        fs::write(&main_go, main_content)?;
        
        if config.ui.interactive {
            println!("  {} Created Go project structure", "🔵".cyan());
        }
    }
    
    // Create go.mod
    let go_mod = path.join("go.mod");
    if !go_mod.exists() {
        let project_name = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("my-project");
        
        let mod_content = format!(r#"module {}

go 1.21
"#, project_name);
        
        fs::write(&go_mod, mod_content)?;
    }
    
    Ok(())
}

/// Create Java project structure
fn create_java_structure(path: &Path, config: &Config) -> Result<()> {
    // Create basic Java directory structure
    let src_main_java = path.join("src").join("main").join("java");
    if !src_main_java.exists() {
        fs::create_dir_all(&src_main_java)?;
        
        // Create Main.java
        let main_java = src_main_java.join("Main.java");
        fs::write(&main_java, r#"public class Main {
    public static void main(String[] args) {
        System.out.println("Hello, world!");
    }
}
"#)?;
        
        if config.ui.interactive {
            println!("  {} Created Java project structure", "☕".yellow());
        }
    }
    
    // Create test directory
    let src_test_java = path.join("src").join("test").join("java");
    if !src_test_java.exists() {
        fs::create_dir_all(&src_test_java)?;
    }
    
    Ok(())
}

/// Show initialization success message and next steps
fn show_init_success(path: &Path, args: &InitArgs, config: &Config) -> Result<()> {
    println!();
    println!("{} Repository initialized successfully!", "🎉".green().bold());
    
    if !config.ui.interactive {
        return Ok(());
    }
    
    println!("\n{} Next steps:", "💡".blue().bold());
    
    // Change directory if not current directory
    if path != &std::env::current_dir().unwrap_or_default() {
        println!("  • {} - Navigate to your repository", 
                format!("cd {}", path.display()).cyan());
    }
    
    if !args.bare {
        // Standard repository next steps
        if path.join("README.md").exists() || path.join("main.rs").exists() || 
           path.join("index.js").exists() || path.join("main.py").exists() {
            println!("  • {} - Add files to staging area", "rgit add .".cyan());
            println!("  • {} - Make your first commit", "rgit commit -m \"Initial commit\"".cyan());
        } else {
            println!("  • {} - Create your project files", "Create files and directories".cyan());
            println!("  • {} - Add files when ready", "rgit add <files>".cyan());
        }
        
        println!("  • {} - Add a remote repository", "rgit remote add origin <url>".cyan());
        println!("  • {} - Push to remote", "rgit push -u origin main".cyan());
    } else {
        // Bare repository next steps
        println!("  • {} - Clone this repository to start working", "git clone <path>".cyan());
        println!("  • {} - Configure as remote for existing repository", "rgit remote add origin <path>".cyan());
    }
    
    println!("  • {} - Check repository status", "rgit status".cyan());
    println!("  • {} - Get help with commands", "rgit --help".cyan());
    
    // Template-specific next steps
    if let Some(ref template) = args.template {
        show_template_next_steps(template)?;
    }
    
    println!();
    Ok(())
}

/// Show template-specific next steps
fn show_template_next_steps(template: &GitignoreTemplate) -> Result<()> {
    println!("\n{} Template-specific tips:", "📚".blue().bold());
    
    match template {
        GitignoreTemplate::Rust => {
            println!("  • {} - Build your project", "cargo build".cyan());
            println!("  • {} - Run your project", "cargo run".cyan());
            println!("  • {} - Add dependencies in Cargo.toml", "Edit Cargo.toml".cyan());
        }
        GitignoreTemplate::Node => {
            println!("  • {} - Install dependencies", "npm install".cyan());
            println!("  • {} - Run your project", "node index.js".cyan());
            println!("  • {} - Add dependencies", "npm install <package>".cyan());
        }
        GitignoreTemplate::Python => {
            println!("  • {} - Create virtual environment", "python -m venv venv".cyan());
            println!("  • {} - Activate virtual environment", "source venv/bin/activate".cyan());
            println!("  • {} - Install dependencies", "pip install -r requirements.txt".cyan());
        }
        GitignoreTemplate::Go => {
            println!("  • {} - Build your project", "go build".cyan());
            println!("  • {} - Run your project", "go run main.go".cyan());
            println!("  • {} - Add dependencies", "go get <package>".cyan());
        }
        GitignoreTemplate::Java => {
            println!("  • {} - Compile your project", "javac src/main/java/Main.java".cyan());
            println!("  • {} - Run your project", "java -cp src/main/java Main".cyan());
            println!("  • {} - Consider using Maven or Gradle", "Build tools".cyan());
        }
        GitignoreTemplate::Default => {
            // No specific tips for default template
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_get_target_path() {
        let args = InitArgs {
            path: Some(PathBuf::from("test-repo")),
            no_ignore: false,
            template: None,
            bare: false,
            initial_branch: None,
        };
        
        let path = get_target_path(&args).unwrap();
        assert!(path.to_string_lossy().contains("test-repo"));
    }

    #[test]
    fn test_is_directory_empty() {
        let temp_dir = TempDir::new().unwrap();
        
        // Empty directory
        assert!(is_directory_empty(temp_dir.path()).unwrap());
        
        // Create a file
        fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
        assert!(!is_directory_empty(temp_dir.path()).unwrap());
    }

    #[test]
    fn test_get_gitignore_content() {
        let content = get_gitignore_content(&GitignoreTemplate::Rust).unwrap();
        assert!(content.contains("target/"));
        assert!(content.contains("Cargo.lock"));
        
        let node_content = get_gitignore_content(&GitignoreTemplate::Node).unwrap();
        assert!(node_content.contains("node_modules/"));
        assert!(node_content.contains("package-lock.json"));
    }

    #[tokio::test]
    async fn test_create_repository() {
        let temp_dir = TempDir::new().unwrap();
        let args = InitArgs {
            path: None,
            no_ignore: false,
            template: None,
            bare: false,
            initial_branch: None,
        };
        
        let repo = create_repository(temp_dir.path(), &args).unwrap();
        assert!(!repo.is_bare());
        assert!(temp_dir.path().join(".git").exists());
    }

    #[tokio::test]
    async fn test_create_bare_repository() {
        let temp_dir = TempDir::new().unwrap();
        let args = InitArgs {
            path: None,
            no_ignore: false,
            template: None,
            bare: true,
            initial_branch: None,
        };
        
        let repo = create_repository(temp_dir.path(), &args).unwrap();
        assert!(repo.is_bare());
    }

    #[test]
    fn test_create_rust_structure() {
        let temp_dir = TempDir::new().unwrap();
        let config = Config::default();
        
        create_rust_structure(temp_dir.path(), &config).unwrap();
        
        assert!(temp_dir.path().join("src").exists());
        assert!(temp_dir.path().join("src/main.rs").exists());
        assert!(temp_dir.path().join("Cargo.toml").exists());
        
        let main_content = fs::read_to_string(temp_dir.path().join("src/main.rs")).unwrap();
        assert!(main_content.contains("Hello, world!"));
    }

    #[test]
    fn test_create_node_structure() {
        let temp_dir = TempDir::new().unwrap();
        let config = Config::default();
        
        create_node_structure(temp_dir.path(), &config).unwrap();
        
        assert!(temp_dir.path().join("package.json").exists());
        assert!(temp_dir.path().join("index.js").exists());
        
        let package_content = fs::read_to_string(temp_dir.path().join("package.json")).unwrap();
        assert!(package_content.contains("\"name\""));
    }
}