use regex::Regex;

pub mod discord;
pub mod ollama;
pub mod python;

pub fn remove_think_tags(text: &str) -> String {
    let re = Regex::new(r"(.*\n)*</think>\n?").unwrap();
    let text = re.replace_all(text, "").trim().to_string();

    text
}

pub fn agent_workspace(workspace: &String, agent_id: &String) -> String {
    format!("{workspace}/agents/{agent_id}")
}
