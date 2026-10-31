//! The built-in `shell` addon (the addons redesign): run commands on this machine in a working directory.
//! Docker shells are not offered here yet; the environment is a plain local shell.

use std::{collections::HashMap, sync::Arc};

use serde_json::{Value, json};

use crate::{
    agents::{
        shell::VizierShell,
        tools::{VizierToolSet, shell::ShellExec},
    },
    config::shell::LocalShellConfig,
    integrations::kind::{
        AgentToolContext, Contribution, FieldType, IntegrationKind, KindField, KindListing,
        string_setting,
    },
    schema::{Integration, IntegrationFamily},
};

pub const ID: &str = "shell";
/// The name of the integration created for an agent.
pub const DEFAULT_NAME: &str = "Shell";

pub struct ShellKind;

impl IntegrationKind for ShellKind {
    fn id(&self) -> &'static str {
        ID
    }

    fn family(&self) -> IntegrationFamily {
        IntegrationFamily::Addon
    }

    fn listing(&self) -> KindListing {
        KindListing {
            name: "Shell",
            description: "Run commands in a working directory on the machine running Vizier",
            icon: "shell",
            fields: vec![
                KindField::new("path", "Working directory", FieldType::String).required(),
                KindField::new("env", "Environment", FieldType::Secret),
            ],
        }
    }

    fn validate(&self, _name: &str, settings: &Value) -> Result<(), String> {
        if string_setting(settings, "path").is_none() {
            return Err("a working directory is required".into());
        }
        if let Some(env) = settings.get("env").and_then(|v| v.as_str()) {
            parse_env(env)?;
        }
        Ok(())
    }

    fn contribute(&self, integration: &Integration, _ctx: &AgentToolContext) -> Contribution {
        let Some(path) = string_setting(&integration.settings, "path") else {
            return Contribution::default();
        };
        let env = integration
            .settings
            .get("env")
            .and_then(|v| v.as_str())
            .and_then(|text| parse_env(text).ok())
            .filter(|env| !env.is_empty());
        let shell = VizierShell::local(LocalShellConfig {
            path: path.to_string(),
            env,
        });
        Contribution {
            native: VizierToolSet::new().tool(ShellExec(Arc::new(shell))),
            mcp: None,
            addon: Some(self.id()),
        }
    }
}

/// `KEY=value` lines, as the form shows them. Blank lines and `#` comments are skipped; a line
/// without `=` is an error rather than a silent drop.
fn parse_env(text: &str) -> Result<HashMap<String, String>, String> {
    let mut env = HashMap::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("line {} is not KEY=value", index + 1));
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(format!("line {} has no name before =", index + 1));
        }
        env.insert(key.to_string(), value.trim().to_string());
    }
    Ok(env)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn it_needs_a_directory_and_reads_env_lines() {
        assert!(ShellKind.listing().fields[0].required);
        assert!(ShellKind.validate("shell", &json!({})).is_err());
        assert!(
            ShellKind
                .validate("shell", &json!({ "path": "/tmp" }))
                .is_ok()
        );
        assert!(
            ShellKind
                .validate("shell", &json!({ "path": "/tmp", "env": "A=1\nB=2" }))
                .is_ok()
        );
        assert!(
            ShellKind
                .validate("shell", &json!({ "path": "/tmp", "env": "A=1\nnope" }))
                .is_err()
        );
        assert_eq!(
            parse_env("A=1\n# note\n\nB= 2 ").unwrap(),
            HashMap::from([("A".into(), "1".into()), ("B".into(), "2".into())])
        );
    }
}
