use std::collections::HashMap;

use serde_json::{Value, json};

use crate::{
    agents::mcp::McpServer,
    config::tools::mcp::McpClientConfig,
    integrations::kind::{
        AgentToolContext, Contribution, FieldType, IntegrationKind, KindField, KindListing,
        string_setting,
    },
    schema::{Integration, IntegrationFamily},
};

pub struct McpKind;

/// `^[a-z0-9][a-z0-9_-]{0,47}$`: the name becomes the server key in tool names.
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    name.len() <= 48
        && (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn is_http(settings: &Value) -> bool {
    settings.get("transport").and_then(|t| t.as_str()) == Some("http")
}

/// The stored settings as the client config `McpServer` connects with.
pub fn client_config(settings: &Value) -> Option<McpClientConfig> {
    if is_http(settings) {
        return Some(McpClientConfig::Http {
            uri: string_setting(settings, "uri")?.to_string(),
        });
    }
    let args = settings
        .get("args")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let env: HashMap<String, String> = settings
        .get("env")
        .and_then(|e| e.as_object())
        .map(|e| {
            e.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    Some(McpClientConfig::Local {
        command: string_setting(settings, "command")?.to_string(),
        args,
        env: (!env.is_empty()).then_some(env),
    })
}

impl IntegrationKind for McpKind {
    fn id(&self) -> &'static str {
        "mcp"
    }

    fn family(&self) -> IntegrationFamily {
        IntegrationFamily::Mcp
    }

    fn listing(&self) -> KindListing {
        KindListing {
            name: "MCP server",
            description: "Tools from a Model Context Protocol server",
            icon: "mcp",
            fields: vec![
                KindField::new("transport", "Transport", FieldType::Select)
                    .options(vec!["local", "http"])
                    .default(json!("local")),
                KindField::new("command", "Command", FieldType::String),
                KindField::new("args", "Arguments", FieldType::StringList),
                KindField::new("env", "Environment", FieldType::SecretMap),
                KindField::new("uri", "URL", FieldType::String),
            ],
        }
    }

    fn validate(&self, name: &str, settings: &Value) -> Result<(), String> {
        if !valid_name(name) {
            return Err(
                "the name must be 1–48 lowercase letters, digits, '_' or '-', starting with a letter or digit"
                    .into(),
            );
        }
        match settings.get("transport").and_then(|t| t.as_str()) {
            None | Some("local") | Some("http") => {}
            Some(other) => return Err(format!("unknown transport '{other}'")),
        }
        if !self.missing(settings).is_empty() {
            return Err(if is_http(settings) {
                "an http server needs a uri".into()
            } else {
                "a local server needs a command".into()
            });
        }
        Ok(())
    }

    fn missing(&self, settings: &Value) -> Vec<String> {
        let key = if is_http(settings) { "uri" } else { "command" };
        match string_setting(settings, key) {
            Some(_) => vec![],
            None => vec![key.to_string()],
        }
    }

    fn contribute(&self, integration: &Integration, _ctx: &AgentToolContext) -> Contribution {
        let Some(config) = client_config(&integration.settings) else {
            return Contribution::default();
        };
        // A connection of this agent's own (per-agent isolation), even when the definition is
        // shared.
        Contribution {
            mcp: Some((integration.name.clone(), McpServer::new(integration.name.clone(), config))),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn validate_checks_the_name_and_the_command_or_uri() {
        let local = json!({"transport": "local", "command": "npx", "args": ["-y", "x"]});
        assert!(McpKind.validate("fs", &local).is_ok());
        assert!(McpKind.validate("my-server_2", &local).is_ok());
        assert!(McpKind.validate("Bad Name", &local).is_err());
        assert!(McpKind.validate("-fs", &local).is_err());
        assert!(McpKind.validate(&"a".repeat(49), &local).is_err());
        assert!(McpKind.validate("fs", &json!({"transport": "local"})).is_err());
        assert!(McpKind.validate("fs", &json!({"transport": "http"})).is_err());
        assert!(McpKind.validate("fs", &json!({"transport": "http", "uri": "http://x"})).is_ok());
    }

    #[test]
    fn every_env_value_is_redacted() {
        let stored = json!({
            "transport": "local", "command": "npx", "args": ["a"],
            "env": {"GITHUB_TOKEN": "ghp_0123456789a91c", "OTHER": "value-1234567"}
        });
        let redacted = McpKind.redact(&stored);
        assert_eq!(redacted["env"], json!({"GITHUB_TOKEN": "••••a91c", "OTHER": "••••4567"}));
        assert_eq!(redacted["command"], "npx");
    }

    #[test]
    fn merge_keeps_unchanged_env_values_and_replaces_sent_ones() {
        let stored = json!({
            "transport": "local", "command": "npx", "args": ["a"],
            "env": {"GITHUB_TOKEN": "ghp_0123456789a91c", "OTHER": "value-1234567"}
        });
        // Only args changed: env left out entirely.
        let merged = McpKind.merge_secrets(
            &stored,
            json!({"transport": "local", "command": "npx", "args": ["b"]}),
        );
        assert_eq!(merged["env"], stored["env"]);
        assert_eq!(merged["args"], json!(["b"]));

        // One value replaced, one sent back redacted, one removed by leaving it out.
        let merged = McpKind.merge_secrets(
            &stored,
            json!({"transport": "local", "command": "npx", "args": ["a"],
                   "env": {"GITHUB_TOKEN": "••••a91c", "NEW": "fresh"}}),
        );
        assert_eq!(merged["env"], json!({"GITHUB_TOKEN": "ghp_0123456789a91c", "NEW": "fresh"}));
    }

    #[test]
    fn settings_map_to_the_client_config() {
        let config = client_config(&json!({
            "transport": "local", "command": "npx", "args": ["-y"], "env": {"K": "v"}
        }));
        assert!(matches!(
            config,
            Some(McpClientConfig::Local { ref command, ref args, env: Some(ref env) })
                if command == "npx" && args == &["-y"] && env["K"] == "v"
        ));
        assert!(matches!(
            client_config(&json!({"transport": "http", "uri": "http://x"})),
            Some(McpClientConfig::Http { .. })
        ));
    }
}
