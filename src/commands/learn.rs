use anyhow::Result;
use colored::*;
use std::collections::HashMap;

use crate::cli::LearnArgs;
use crate::config::Config;
use crate::error::RgitError;
use crate::interactive::{InteractivePrompt, TableDisplay};

/// Execute the learn command - interactive Git tutorials
pub async fn execute(args: &LearnArgs, config: &Config) -> Result<()> {
    if !config.is_interactive() {
        return Err(RgitError::NonInteractiveEnvironment.into());
    }

    println!("{} {} Interactive Git Learning", "🎓".blue(), "rgit".cyan().bold());
    println!("{}", "Welcome to the rgit learning experience!".green());
    println!();

    let tutorial_manager = TutorialManager::new();

    if let Some(ref topic) = args.topic {
        // Run specific tutorial
        tutorial_manager.run_tutorial_by_name(topic, config).await
    } else {
        // Show tutorial menu
        tutorial_manager.run_tutorial_menu(config).await
    }
}

/// Tutorial management system
struct TutorialManager {
    tutorials: HashMap<String, Tutorial>,
}

impl TutorialManager {
    fn new() -> Self {
        let mut tutorials = HashMap::new();
        
        // Register all available tutorials
        tutorials.insert("basics".to_string(), Tutorial::basics());
        tutorials.insert("branching".to_string(), Tutorial::branching());
        tutorials.insert("merging".to_string(), Tutorial::merging());
        tutorials.insert("submodules".to_string(), Tutorial::submodules());
        tutorials.insert("conflicts".to_string(), Tutorial::conflicts());
        tutorials.insert("remotes".to_string(), Tutorial::remotes());
        tutorials.insert("advanced".to_string(), Tutorial::advanced());
        tutorials.insert("workflow".to_string(), Tutorial::workflow());
        tutorials.insert("troubleshooting".to_string(), Tutorial::troubleshooting());

        Self { tutorials }
    }

    /// Run tutorial selection menu
    async fn run_tutorial_menu(&self, config: &Config) -> Result<()> {
        loop {
            self.show_tutorial_overview()?;
            
            let options: Vec<String> = self.tutorials.values()
                .map(|t| format!("{} - {}", t.title, t.description))
                .collect();
            
            let mut menu_options = options;
            menu_options.push("Exit tutorials".to_string());

            let selection = InteractivePrompt::new()
                .with_message("Choose a tutorial to learn about")
                .with_options(&menu_options)
                .fuzzy_search()
                .select()?;

            if selection == menu_options.len() - 1 {
                println!("\n{} Happy learning! 🎉", "✅".green());
                break;
            }

            // Get tutorial by index
            let tutorial_keys: Vec<_> = self.tutorials.keys().collect();
            if let Some(tutorial_key) = tutorial_keys.get(selection) {
                if let Some(tutorial) = self.tutorials.get(*tutorial_key) {
                    self.run_tutorial(tutorial, config).await?;
                }
            }

            // Ask if user wants to continue
            println!();
            if !InteractivePrompt::new()
                .with_message("Would you like to try another tutorial?")
                .confirm()? {
                break;
            }
        }

        Ok(())
    }

    /// Run tutorial by name
    async fn run_tutorial_by_name(&self, name: &str, config: &Config) -> Result<()> {
        let tutorial_key = self.find_tutorial_key(name);
        
        if let Some(tutorial) = tutorial_key.and_then(|key| self.tutorials.get(key)) {
            self.run_tutorial(tutorial, config).await
        } else {
            println!("{} Tutorial '{}' not found", "❌".red(), name.red());
            self.suggest_similar_tutorials(name)?;
            Ok(())
        }
    }

    /// Find tutorial key by partial name match
    fn find_tutorial_key(&self, name: &str) -> Option<&String> {
        let name_lower = name.to_lowercase();
        
        // Exact match first
        if let Some(key) = self.tutorials.keys().find(|k| k.to_lowercase() == name_lower) {
            return Some(key);
        }
        
        // Partial match
        self.tutorials.keys().find(|k| k.to_lowercase().contains(&name_lower))
    }

    /// Show tutorial overview
    fn show_tutorial_overview(&self) -> Result<()> {
        println!("{} Available Tutorials:", "📚".blue().bold());
        println!();

        let mut table = TableDisplay::new()
            .with_headers(vec![
                "Tutorial".to_string(),
                "Level".to_string(),
                "Duration".to_string(),
                "Description".to_string(),
            ]);

        for tutorial in self.tutorials.values() {
            let level_colored = match tutorial.level {
                TutorialLevel::Beginner => "Beginner".green().to_string(),
                TutorialLevel::Intermediate => "Intermediate".yellow().to_string(),
                TutorialLevel::Advanced => "Advanced".red().to_string(),
            };

            table.add_row(vec![
                tutorial.title.clone(),
                level_colored,
                tutorial.duration.clone(),
                tutorial.description.clone(),
            ]);
        }

        table.display();
        println!();

        Ok(())
    }

    /// Run a specific tutorial
    async fn run_tutorial(&self, tutorial: &Tutorial, config: &Config) -> Result<()> {
        println!("\n{} {}", "🎯".blue().bold(), tutorial.title.cyan().bold());
        println!("{}", tutorial.description.dimmed());
        println!("⏱️  Duration: {} | 📊 Level: {:?}", tutorial.duration, tutorial.level);
        println!();

        if !InteractivePrompt::new()
            .with_message("Ready to start this tutorial?")
            .confirm()? {
            return Ok(());
        }

        // Run tutorial sections
        for (i, section) in tutorial.sections.iter().enumerate() {
            println!("\n{} Section {}: {}", 
                    "📖".blue(), 
                    i + 1, 
                    section.title.cyan().bold());
            println!("{}", "─".repeat(50).dimmed());
            
            self.run_tutorial_section(section, config).await?;
            
            // Check if user wants to continue to next section
            if i < tutorial.sections.len() - 1 {
                if !InteractivePrompt::new()
                    .with_message("Continue to next section?")
                    .confirm()? {
                    break;
                }
            }
        }

        // Tutorial completion
        self.show_tutorial_completion(tutorial)?;

        Ok(())
    }

    /// Run a tutorial section
    async fn run_tutorial_section(&self, section: &TutorialSection, _config: &Config) -> Result<()> {
        // Show explanation
        for line in &section.explanation {
            println!("{}", line);
        }
        println!();

        // Show examples
        if !section.examples.is_empty() {
            println!("{} Examples:", "💡".yellow().bold());
            for example in &section.examples {
                println!("  {} {}", "•".green(), example.cyan());
            }
            println!();
        }

        // Interactive exercises
        if !section.exercises.is_empty() {
            println!("{} Try it yourself:", "🏃".green().bold());
            
            for (i, exercise) in section.exercises.iter().enumerate() {
                println!("\n{} Exercise {}: {}", "📝".blue(), i + 1, exercise.description);
                
                if !exercise.command.is_empty() {
                    println!("   Try running: {}", exercise.command.cyan().bold());
                }
                
                if !exercise.hint.is_empty() {
                    if InteractivePrompt::new()
                        .with_message("Need a hint?")
                        .confirm()? {
                        println!("   💡 {}", exercise.hint.yellow());
                    }
                }

                InteractivePrompt::new()
                    .with_message("Press Enter when you've completed this exercise")
                    .with_options(&["Continue".to_string()])
                    .select()?;
            }
        }

        // Quiz questions
        if !section.quiz.is_empty() {
            println!("\n{} Quick Quiz:", "🧠".purple().bold());
            let mut correct_answers = 0;
            
            for (i, question) in section.quiz.iter().enumerate() {
                println!("\n{} Question {}: {}", "❓".blue(), i + 1, question.question);
                
                let answer = InteractivePrompt::new()
                    .with_message("Your answer")
                    .with_options(&question.options)
                    .select()?;

                if answer == question.correct_answer {
                    println!("   {} Correct!", "✅".green());
                    correct_answers += 1;
                } else {
                    println!("   {} Not quite. {}", "❌".red(), question.explanation);
                }
            }
            
            let percentage = (correct_answers * 100) / section.quiz.len();
            println!("\n📊 Quiz Score: {}/{} ({}%)", 
                    correct_answers, 
                    section.quiz.len(), 
                    percentage);
        }

        Ok(())
    }

    /// Show tutorial completion
    fn show_tutorial_completion(&self, tutorial: &Tutorial) -> Result<()> {
        println!("\n{} Tutorial Complete! 🎉", "🏆".yellow().bold());
        println!("You've successfully completed: {}", tutorial.title.cyan().bold());
        
        if !tutorial.next_steps.is_empty() {
            println!("\n{} Next Steps:", "🚀".blue().bold());
            for step in &tutorial.next_steps {
                println!("  • {}", step.green());
            }
        }

        if !tutorial.related_tutorials.is_empty() {
            println!("\n{} Related Tutorials:", "🔗".blue().bold());
            for related in &tutorial.related_tutorials {
                println!("  • {}", related.cyan());
            }
        }

        Ok(())
    }

    /// Suggest similar tutorials when one is not found
    fn suggest_similar_tutorials(&self, query: &str) -> Result<()> {
        let similar: Vec<_> = self.tutorials.keys()
            .filter(|k| k.contains(&query.to_lowercase()) || 
                       levenshtein_distance(k, query) <= 2)
            .collect();

        if !similar.is_empty() {
            println!("\n{} Did you mean:", "💡".blue());
            for suggestion in similar {
                println!("  • {}", suggestion.cyan());
            }
        }

        println!("\n{} Use {} to see all available tutorials", 
                "ℹ️".blue(), 
                "rgit learn".cyan());

        Ok(())
    }
}

// =============================================================================
// Tutorial Data Structures
// =============================================================================

#[derive(Debug)]
struct Tutorial {
    title: String,
    description: String,
    level: TutorialLevel,
    duration: String,
    sections: Vec<TutorialSection>,
    next_steps: Vec<String>,
    related_tutorials: Vec<String>,
}

#[derive(Debug)]
enum TutorialLevel {
    Beginner,
    Intermediate,
    Advanced,
}

#[derive(Debug)]
struct TutorialSection {
    title: String,
    explanation: Vec<String>,
    examples: Vec<String>,
    exercises: Vec<Exercise>,
    quiz: Vec<QuizQuestion>,
}

#[derive(Debug)]
struct Exercise {
    description: String,
    command: String,
    hint: String,
}

#[derive(Debug)]
struct QuizQuestion {
    question: String,
    options: Vec<String>,
    correct_answer: usize,
    explanation: String,
}

// =============================================================================
// Tutorial Implementations
// =============================================================================

impl Tutorial {
    fn basics() -> Self {
        Self {
            title: "Git Basics".to_string(),
            description: "Learn fundamental Git concepts and commands".to_string(),
            level: TutorialLevel::Beginner,
            duration: "15 minutes".to_string(),
            sections: vec![
                TutorialSection {
                    title: "What is Git?".to_string(),
                    explanation: vec![
                        "Git is a distributed version control system that helps you:".to_string(),
                        "• Track changes in your files over time".to_string(),
                        "• Collaborate with others on the same project".to_string(),
                        "• Maintain a complete history of your project".to_string(),
                        "• Work on different features simultaneously".to_string(),
                    ],
                    examples: vec![
                        "Think of Git like a time machine for your code".to_string(),
                        "Every 'commit' is a snapshot you can return to".to_string(),
                    ],
                    exercises: vec![
                        Exercise {
                            description: "Check your Git version".to_string(),
                            command: "git --version".to_string(),
                            hint: "This shows which version of Git you have installed".to_string(),
                        }
                    ],
                    quiz: vec![
                        QuizQuestion {
                            question: "What is the main purpose of Git?".to_string(),
                            options: vec![
                                "To compile code".to_string(),
                                "To track changes in files".to_string(),
                                "To run programs".to_string(),
                                "To design websites".to_string(),
                            ],
                            correct_answer: 1,
                            explanation: "Git is primarily a version control system for tracking changes".to_string(),
                        }
                    ],
                },
                TutorialSection {
                    title: "Basic Workflow".to_string(),
                    explanation: vec![
                        "The basic Git workflow follows these steps:".to_string(),
                        "1. Make changes to your files".to_string(),
                        "2. Stage the changes you want to commit".to_string(),
                        "3. Commit the staged changes with a message".to_string(),
                        "4. Push commits to share with others".to_string(),
                    ],
                    examples: vec![
                        "rgit add file.txt        # Stage changes".to_string(),
                        "rgit commit -m 'message' # Create commit".to_string(),
                        "rgit push                # Share changes".to_string(),
                    ],
                    exercises: vec![
                        Exercise {
                            description: "Create a new file and add it to Git".to_string(),
                            command: "echo 'Hello Git' > test.txt && rgit add test.txt".to_string(),
                            hint: "This creates a file and stages it for commit".to_string(),
                        }
                    ],
                    quiz: vec![],
                },
            ],
            next_steps: vec![
                "Try the 'Branching' tutorial to learn about parallel development".to_string(),
                "Practice with 'rgit status' to see your repository state".to_string(),
            ],
            related_tutorials: vec![
                "branching".to_string(),
                "workflow".to_string(),
            ],
        }
    }

    fn branching() -> Self {
        Self {
            title: "Branching & Merging".to_string(),
            description: "Master Git branches for parallel development".to_string(),
            level: TutorialLevel::Intermediate,
            duration: "20 minutes".to_string(),
            sections: vec![
                TutorialSection {
                    title: "Understanding Branches".to_string(),
                    explanation: vec![
                        "Branches allow you to work on different features simultaneously:".to_string(),
                        "• Each branch is an independent line of development".to_string(),
                        "• You can switch between branches easily".to_string(),
                        "• Changes in one branch don't affect others".to_string(),
                        "• Branches can be merged when features are complete".to_string(),
                    ],
                    examples: vec![
                        "rgit branch feature/login    # Create new branch".to_string(),
                        "rgit checkout feature/login  # Switch to branch".to_string(),
                        "rgit checkout -b feature/ui  # Create and switch".to_string(),
                    ],
                    exercises: vec![
                        Exercise {
                            description: "List all branches in your repository".to_string(),
                            command: "rgit branch".to_string(),
                            hint: "The current branch is marked with an asterisk (*)".to_string(),
                        }
                    ],
                    quiz: vec![
                        QuizQuestion {
                            question: "What happens when you create a new branch?".to_string(),
                            options: vec![
                                "All files are deleted".to_string(),
                                "A copy of the current state is created".to_string(),
                                "The repository is reset".to_string(),
                                "Nothing happens".to_string(),
                            ],
                            correct_answer: 1,
                            explanation: "A new branch creates an independent line of development from the current state".to_string(),
                        }
                    ],
                },
            ],
            next_steps: vec![
                "Learn about 'Merging' to combine branch changes".to_string(),
                "Practice creating feature branches for new work".to_string(),
            ],
            related_tutorials: vec![
                "merging".to_string(),
                "workflow".to_string(),
            ],
        }
    }

    fn submodules() -> Self {
        Self {
            title: "Submodules".to_string(),
            description: "Learn to manage Git submodules effectively".to_string(),
            level: TutorialLevel::Advanced,
            duration: "25 minutes".to_string(),
            sections: vec![
                TutorialSection {
                    title: "What are Submodules?".to_string(),
                    explanation: vec![
                        "Submodules let you include other Git repositories in your project:".to_string(),
                        "• Keep external dependencies as separate repositories".to_string(),
                        "• Pin to specific versions of dependencies".to_string(),
                        "• Maintain clean separation between your code and libraries".to_string(),
                        "• Share common code between multiple projects".to_string(),
                    ],
                    examples: vec![
                        "rgit submodule add https://github.com/user/lib.git libs/mylib".to_string(),
                        "rgit submodule init     # Initialize submodules".to_string(),
                        "rgit submodule update   # Update to specified commits".to_string(),
                    ],
                    exercises: vec![
                        Exercise {
                            description: "Check current submodule status".to_string(),
                            command: "rgit submodule status".to_string(),
                            hint: "This shows the status of all submodules in your repository".to_string(),
                        }
                    ],
                    quiz: vec![],
                },
            ],
            next_steps: vec![
                "Practice adding a real submodule to a project".to_string(),
                "Learn about submodule workflows in teams".to_string(),
            ],
            related_tutorials: vec![
                "advanced".to_string(),
                "troubleshooting".to_string(),
            ],
        }
    }

    // Simplified implementations for other tutorials
    fn merging() -> Self {
        Self {
            title: "Merging Strategies".to_string(),
            description: "Learn different ways to combine branch changes".to_string(),
            level: TutorialLevel::Intermediate,
            duration: "18 minutes".to_string(),
            sections: vec![],
            next_steps: vec![],
            related_tutorials: vec!["branching".to_string(), "conflicts".to_string()],
        }
    }

    fn conflicts() -> Self {
        Self {
            title: "Conflict Resolution".to_string(),
            description: "Master resolving merge conflicts like a pro".to_string(),
            level: TutorialLevel::Intermediate,
            duration: "22 minutes".to_string(),
            sections: vec![],
            next_steps: vec![],
            related_tutorials: vec!["merging".to_string(), "troubleshooting".to_string()],
        }
    }

    fn remotes() -> Self {
        Self {
            title: "Remote Repositories".to_string(),
            description: "Work with remote Git repositories and collaboration".to_string(),
            level: TutorialLevel::Intermediate,
            duration: "20 minutes".to_string(),
            sections: vec![],
            next_steps: vec![],
            related_tutorials: vec!["workflow".to_string()],
        }
    }

    fn advanced() -> Self {
        Self {
            title: "Advanced Git".to_string(),
            description: "Advanced Git techniques and power-user features".to_string(),
            level: TutorialLevel::Advanced,
            duration: "30 minutes".to_string(),
            sections: vec![],
            next_steps: vec![],
            related_tutorials: vec!["submodules".to_string(), "troubleshooting".to_string()],
        }
    }

    fn workflow() -> Self {
        Self {
            title: "Git Workflows".to_string(),
            description: "Learn popular Git workflows for teams".to_string(),
            level: TutorialLevel::Intermediate,
            duration: "25 minutes".to_string(),
            sections: vec![],
            next_steps: vec![],
            related_tutorials: vec!["branching".to_string(), "remotes".to_string()],
        }
    }

    fn troubleshooting() -> Self {
        Self {
            title: "Troubleshooting".to_string(),
            description: "Fix common Git problems and mistakes".to_string(),
            level: TutorialLevel::Advanced,
            duration: "20 minutes".to_string(),
            sections: vec![],
            next_steps: vec![],
            related_tutorials: vec!["advanced".to_string(), "conflicts".to_string()],
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Calculate Levenshtein distance between two strings
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if s1.chars().nth(i - 1) == s2.chars().nth(j - 1) { 0 } else { 1 };
            matrix[i][j] = std::cmp::min(
                std::cmp::min(matrix[i - 1][j] + 1, matrix[i][j - 1] + 1),
                matrix[i - 1][j - 1] + cost,
            );
        }
    }

    matrix[len1][len2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tutorial_manager_creation() {
        let manager = TutorialManager::new();
        assert!(!manager.tutorials.is_empty());
        assert!(manager.tutorials.contains_key("basics"));
        assert!(manager.tutorials.contains_key("branching"));
    }

    #[test]
    fn test_find_tutorial_key() {
        let manager = TutorialManager::new();
        
        // Exact match
        assert_eq!(manager.find_tutorial_key("basics"), Some(&"basics".to_string()));
        
        // Partial match
        assert_eq!(manager.find_tutorial_key("branch"), Some(&"branching".to_string()));
        
        // No match
        assert_eq!(manager.find_tutorial_key("nonexistent"), None);
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
        assert_eq!(levenshtein_distance("hello", "hallo"), 1);
        assert_eq!(levenshtein_distance("git", "got"), 1);
        assert_eq!(levenshtein_distance("branch", "ranch"), 1);
    }

    #[test]
    fn test_tutorial_structure() {
        let basics = Tutorial::basics();
        assert_eq!(basics.title, "Git Basics");
        assert!(!basics.sections.is_empty());
        assert!(matches!(basics.level, TutorialLevel::Beginner));
    }

    #[tokio::test]
    async fn test_tutorial_manager_invalid_tutorial() {
        let manager = TutorialManager::new();
        let config = Config::minimal();
        
        // This would normally be interactive, but we're testing the logic
        let result = manager.run_tutorial_by_name("invalid", &config).await;
        assert!(result.is_ok()); // Should handle gracefully
    }
}