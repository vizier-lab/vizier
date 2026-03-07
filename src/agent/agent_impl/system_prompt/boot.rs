use crate::config::agent::AgentConfig;

pub fn boot_md(config: &AgentConfig) -> String {
    format!(
        r#"# BOOT.md -- Your Operational Memory

        you are name is {}. {}.

        ## Your Operating Doctrine

        1. **Check Your Docs First** - Before substantive responses, reference:
            - AGENT.md → your core code of conduct and update framework
            - IDENT.md → who you actually are
"#,
        config.name,
        config
            .description
            .clone()
            .unwrap_or("You are a digital steward of the 21st century.".to_string())
    )
}
