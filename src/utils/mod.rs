use regex::Regex;
use std::path::PathBuf;

pub mod discord;
pub mod markdown;
pub mod ollama;
pub mod tar;
pub mod telegram;

pub fn remove_think_tags(text: &str) -> String {
    let re = Regex::new(r"(.*\n)*</think>\n?").unwrap();
    let text = re.replace_all(text, "").trim().to_string();

    text
}

/// Build a filesystem path in a cross-platform way using PathBuf.
/// 
/// This ensures compatibility on Windows (which uses backslashes) and Unix systems.
/// 
/// # Example
/// ```
/// let path = build_path("/home/user", &["projects", "vizier"]);
/// // Returns: /home/user/projects/vizier on Unix, \home\user\projects\vizier on Windows
/// ```
pub fn build_path(base: &str, components: &[&str]) -> PathBuf {
    let mut path = PathBuf::from(base);
    for component in components {
        path.push(component);
    }
    path
}

/// Build a glob pattern path in a cross-platform way.
/// 
/// Glob patterns require forward slashes even on Windows, so this function
/// constructs the path using PathBuf for safety, then converts to string with forward slashes.
/// 
/// # Example
/// ```
/// let glob_pattern = build_glob_path("/home/user", &["projects", "**", "*.md"]);
/// // Returns: /home/user/projects/**/*.md (with forward slashes on all platforms)
/// ```
pub fn build_glob_path(base: &str, components: &[&str]) -> String {
    let mut path = PathBuf::from(base);
    for component in components {
        path.push(component);
    }
    
    // Convert to string using forward slashes (required for glob patterns)
    path.to_string_lossy()
        .to_string()
        .replace('\\', "/")
}

/// Get the workspace directory for a specific agent.
/// 
/// # Example
/// ```
/// let workspace = agent_workspace("/home/user/.vizier", "my_agent");
/// // Returns: /home/user/.vizier/agents/my_agent
/// ```
pub fn agent_workspace(workspace: &str, agent_id: &str) -> PathBuf {
    build_path(workspace, &["agents", agent_id])
}

pub fn format_thinking(name: &String, args: &serde_json::Value) -> String {
    let title = match &*name.clone() {
        "think" => "is thinking:".to_string(),
        "memory_read" => "memory:".to_string(),
        "memory_write" => "memory:".to_string(),
        _ => format!("use {}", &name),
    };

    let content = match &*name.clone() {
        "think" => format!("\n> {}", args["thought"].as_str().unwrap()),
        "python_interpreter" => format!("```python\n{}\n```", args["script"].as_str().unwrap()),
        "memory_read" => format!("searching for '{}'", args["query"].as_str().unwrap()),
        "memory_write" => format!("writing '{}'", args["title"].as_str().unwrap()),
        _ => format!(
            "```js\n{}\n```",
            serde_json::to_string_pretty(&args).unwrap()
        ),
    };

    format!("{} {}", title, content)
}
