use regex::Regex;

pub mod discord;
pub mod markdown;
pub mod ollama;
pub mod python;
pub mod tar;

pub fn remove_think_tags(text: &str) -> String {
    let re = Regex::new(r"(.*\n)*</think>\n?").unwrap();
    let text = re.replace_all(text, "").trim().to_string();

    text
}

pub fn agent_workspace(workspace: &String, agent_id: &String) -> String {
    format!("{workspace}/agents/{agent_id}")
}

pub fn format_thinking(name: &String, args: &serde_json::Value) -> String {
    let title = match &*name.clone() {
        "think" => "is thinking:".to_string(),
        _ => format!("use {}", &name),
    };

    let content = match &*name.clone() {
        "think" => format!("\n> {}", args["thought"].as_str().unwrap()),
        "python_interpreter" => format!("```python\n{}\n```", args["script"].as_str().unwrap()),
        _ => format!(
            "```js\n{}\n```",
            serde_json::to_string_pretty(&args).unwrap()
        ),
    };

    format!("{} {}", title, content)
}
