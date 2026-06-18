use regex::Regex;
use std::path::PathBuf;

pub mod discord;
pub mod logo;
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
    path.to_string_lossy().to_string().replace('\\', "/")
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
        "memory_list" => "📚 Listing memories:".to_string(),
        "memory_detail" => "🔎 Memory detail:".to_string(),
        "memory_follow" => "🔗 Following links:".to_string(),
        "memory_graph" => "📊 Knowledge graph:".to_string(),
        "memory_delete" => "🗑️ Deleting memory:".to_string(),
        "list_task" => "📋 Listing tasks:".to_string(),
        "delete_task" => "🗑️ Deleting task:".to_string(),
        "get_task_detail" => "📋 Task detail:".to_string(),
        "update_skill" => "✏️ Updating skill:".to_string(),
        "delete_skill" => "🗑️ Deleting skill:".to_string(),
        "list_skills" => "📚 Listing skills:".to_string(),
        "read_skill_resource" => "📖 Reading skill resource:".to_string(),
        "execute_skill_resource" => "▶️ Executing skill resource:".to_string(),
        "list_session_files" => "📂 Listing session files".to_string(),
        "read_document_file" => "📄 Reading document:".to_string(),
        "send_attachment" => "📎 Sending attachment:".to_string(),
        "fetch" => "🌐 Fetching:".to_string(),
        "http_client" => format!("🔗 HTTP {}:", args["method"].as_str().unwrap_or("GET")),
        "web_search" => "🌐 Searching web:".to_string(),
        "news_search" => "📰 Finding news:".to_string(),
        "shell_exec" => "🖥️ Running shell:".to_string(),
        "tts_generate" => "🔊 Generating speech:".to_string(),
        "stt_transcribe" => "🎤 Transcribing:".to_string(),
        "image_generate" => "🎨 Generating image:".to_string(),
        "read_image_file" => "🖼️ Reading image:".to_string(),
        "write_dream_journal" => "💤 Writing dream journal:".to_string(),
        "read_dream_journal" => "💤 Reading dream journal:".to_string(),
        "schedule_one_time_task" => "⏰ Scheduling task:".to_string(),
        "schedule_cron_task" => "🔄 Setting up recurring task:".to_string(),
        "consult_agent" => "🤝 Consulting agent:".to_string(),
        "delegate_agent" => "👤 Delegating task:".to_string(),
        "paralel_subtasks" => "⚡ Running parallel tasks".to_string(),
        "create_skill" => "🎯 Creating skill:".to_string(),
        "WRITE_SOUL" => "📝 Updating notes".to_string(),
        "WRITE_IDENTITY" => "🪪 Updating identity".to_string(),
        "WRITE_HEARTBEAT" => "💗 Updating heartbeat".to_string(),
        "READ_HEARTBEAT" => "💗 Reading heartbeat".to_string(),
        "discord_send_message" => "💬 Discord message".to_string(),
        "discord_react_message" => "👍 Discord reaction".to_string(),
        "discord_get_message_by_id" => "📩 Discord message".to_string(),
        "telegram_send_message" => "✈️ Telegram message".to_string(),
        "telegram_react_message" => "👍 Telegram reaction".to_string(),
        "telegram_get_message_by_id" => "📩 Telegram message".to_string(),
        "webui_send_message" => "💬 WebUI message:".to_string(),
        "webui_list_topics" => "📋 WebUI topics:".to_string(),
        _ if name.starts_with("mcp_") => "🔌 MCP tool:".to_string(),
        _ => format!("use {}", &name),
    };

    let content = match &*name.clone() {
        "think" => args["thought"]
            .as_str()
            .unwrap()
            .split('\n')
            .map(|line| format!("> {}", line))
            .collect::<Vec<_>>()
            .join("\n"),
        "python_interpreter" => {
            format!("```python\n{}\n```", args["script"].as_str().unwrap())
        }
        "shell_exec" => format!("```bash\n{}\n```", args["commands"].as_str().unwrap()),
        "memory_read" => format!("searching for '{}'", args["query"].as_str().unwrap()),
        "memory_write" => format!("writing '{}'", args["title"].as_str().unwrap()),
        "memory_list" => {
            let limit = args["limit"].as_u64().unwrap_or(50);
            let offset = args["offset"].as_u64().unwrap_or(0);
            format!("showing {} starting at {}", limit, offset)
        }
        "memory_detail" | "memory_delete" => {
            format!("'{}'", args["slug"].as_str().unwrap_or("?"))
        }
        "memory_follow" => {
            let slug = args["slug"].as_str().unwrap_or("?");
            let depth = args["depth"].as_u64().unwrap_or(1);
            format!("'{}' depth: {}", slug, depth)
        }
        "memory_graph" => {
            if let Some(tags) = args["tags"].as_array() {
                if !tags.is_empty() {
                    let tag_strs: Vec<&str> = tags.iter().filter_map(|t| t.as_str()).collect();
                    format!("tags: [{}]", tag_strs.join(", "))
                } else {
                    "all memories".to_string()
                }
            } else {
                "all memories".to_string()
            }
        }
        "list_task" => {
            if let Some(active) = args["is_active"].as_bool() {
                format!("filter: active={}", active)
            } else {
                "all tasks".to_string()
            }
        }
        "delete_task" | "get_task_detail" => {
            format!("'{}'", args["slug"].as_str().unwrap_or("?"))
        }
        "update_skill" => format!("'{}'", args["slug"].as_str().unwrap_or("?")),
        "delete_skill" => format!("'{}'", args["slug"].as_str().unwrap_or("?")),
        "list_skills" => {
            if let Some(kw) = args["keyword"].as_str() {
                format!("keyword: '{}'", kw)
            } else {
                "all skills".to_string()
            }
        }
        "read_skill_resource" => {
            format!(
                "'{}/{}'",
                args["slug"].as_str().unwrap_or("?"),
                args["path"].as_str().unwrap_or("?")
            )
        }
        "execute_skill_resource" => {
            format!(
                "'{}/{}'",
                args["slug"].as_str().unwrap_or("?"),
                args["path"].as_str().unwrap_or("?")
            )
        }
        "read_document_file" => {
            format!("'{}'", args["filename"].as_str().unwrap_or("?"))
        }
        "send_attachment" => {
            format!("'{}'", args["filename"].as_str().unwrap_or("?"))
        }
        "fetch" => format!("'{}'", args["url"].as_str().unwrap_or("?")),
        "http_client" => format!(
            "'{}'",
            args["url"].as_str().unwrap_or("?")
        ),
        "web_search" | "news_search" => {
            format!("'{}'", args["query"].as_str().unwrap_or("?"))
        }
        "tts_generate" => {
            let text = args["text"].as_str().unwrap_or("");
            if text.len() > 60 {
                format!("'{}...'", &text[..60])
            } else {
                format!("'{}'", text)
            }
        }
        "stt_transcribe" => {
            format!("'{}'", args["filename"].as_str().unwrap_or("?"))
        }
        "image_generate" => {
            let prompt = args["prompt"].as_str().unwrap_or("");
            if prompt.len() > 60 {
                format!("'{}...'", &prompt[..60])
            } else {
                format!("'{}'", prompt)
            }
        }
        "read_image_file" => {
            format!("'{}'", args["filename"].as_str().unwrap_or("?"))
        }
        "write_dream_journal" => {
            format!("'{}'", args["stage"].as_str().unwrap_or("?"))
        }
        "read_dream_journal" => {
            let parts = vec![
                args["stage"].as_str().map(|s| format!("stage: {}", s)),
                args["cycle_id"]
                    .as_str()
                    .map(|s| format!("cycle: {}", s)),
                args["limit"]
                    .as_u64()
                    .map(|l| format!("limit: {}", l)),
            ];
            let filters: Vec<String> = parts.into_iter().flatten().collect();
            if filters.is_empty() {
                "recent entries".to_string()
            } else {
                filters.join(", ")
            }
        }
        "schedule_one_time_task" | "schedule_cron_task" => {
            format!("'{}'", args["title"].as_str().unwrap_or("?"))
        }
        "consult_agent" | "delegate_agent" => {
            format!(
                "agent {} about '{}'",
                args["agent_id"].as_str().unwrap_or("?"),
                args["prompt"].as_str().unwrap_or("")
            )
        }
        "create_skill" => format!("'{}'", args["name"].as_str().unwrap_or("?")),
        "webui_send_message" => {
            format!("@{}", args["username"].as_str().unwrap_or("?"))
        }
        "webui_list_topics" => {
            format!("@{}", args["username"].as_str().unwrap_or("?"))
        }
        _ if name.starts_with("mcp_") => {
            let parts = name.strip_prefix("mcp_").unwrap_or(name);
            let segments: Vec<&str> = parts.splitn(2, "__").collect();
            if segments.len() == 2 {
                format!("{} ({})", segments[1], segments[0])
            } else {
                name.to_string()
            }
        }
        _ => format!(
            "```js\n{}\n```",
            serde_json::to_string_pretty(&args).unwrap()
        ),
    };

    if title.ends_with(':') || title.ends_with('.') {
        format!("{} {}", title, content)
    } else {
        format!("{}\n{}", title, content)
    }
}

pub fn get_mime_type(filename: &str) -> String {
    mime_guess::from_path(filename)
        .first_or_text_plain()
        .to_string()
}
