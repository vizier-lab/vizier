//! `dummyplug`: an offline test provider. It needs no keys and makes no network calls, and
//! replies to chat messages with random lorem ipsum.
//!
//! See `specs/008-dummyplug-provider/` for the full chat protocol.

use anyhow::Result;
use rand::RngExt;
use rig_core::{
    OneOrMany,
    completion::{ToolDefinition, Usage},
    message::{AssistantContent, Message},
};

use super::VizierModelTrait;
use crate::schema::AgentConfig;

pub struct DummyplugModel {
    context_window: Option<u64>,
}

impl DummyplugModel {
    pub fn new(agent_config: &AgentConfig) -> Self {
        Self {
            context_window: agent_config.context_window,
        }
    }
}

#[async_trait::async_trait]
impl VizierModelTrait for DummyplugModel {
    async fn completion(
        &self,
        _message: Message,
        _history: Vec<Message>,
        _tools: Vec<ToolDefinition>,
    ) -> Result<(Option<String>, OneOrMany<AssistantContent>, Usage)> {
        Ok((
            None,
            OneOrMany::one(AssistantContent::text(lorem_ipsum())),
            Usage::new(),
        ))
    }

    fn context_window(&self) -> Option<u64> {
        self.context_window
    }
}

const WORDS: &[&str] = &[
    "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit", "sed", "do",
    "eiusmod", "tempor", "incididunt", "ut", "labore", "et", "dolore", "magna", "aliqua", "enim",
    "ad", "minim", "veniam", "quis", "nostrud", "exercitation", "ullamco", "laboris", "nisi",
    "aliquip", "ex", "ea", "commodo", "consequat", "duis", "aute", "irure", "in",
    "reprehenderit", "voluptate", "velit", "esse", "cillum", "fugiat", "nulla", "pariatur",
    "excepteur", "sint", "occaecat", "cupidatat", "non", "proident", "sunt", "culpa", "qui",
    "officia", "deserunt", "mollit", "anim", "id", "est", "laborum",
];

/// 1–3 paragraphs of 2–5 sentences, each 6–14 words, capitalized and ending in `.`.
fn lorem_ipsum() -> String {
    let mut rng = rand::rng();
    let paragraphs = rng.random_range(1..=3);
    (0..paragraphs)
        .map(|_| {
            let sentences = rng.random_range(2..=5);
            (0..sentences)
                .map(|_| {
                    let words = rng.random_range(6..=14);
                    let mut sentence = (0..words)
                        .map(|_| WORDS[rng.random_range(0..WORDS.len())])
                        .collect::<Vec<_>>()
                        .join(" ");
                    if let Some(first) = sentence.get_mut(0..1) {
                        first.make_ascii_uppercase();
                    }
                    sentence.push('.');
                    sentence
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_config(context_window: Option<u64>) -> AgentConfig {
        AgentConfig {
            name: "tester".into(),
            owner_id: None,
            shared_to: vec![],
            system_prompt: None,
            description: None,
            provider: crate::config::provider::ProviderVariant::dummyplug,
            model: "dummyplug".into(),
            thinking_depth: 5,
            checkpoint_threshold: 0.8,
            tools: Default::default(),
            silent_read_initiative_chance: 0.0,
            max_tokens: None,
            context_window,
            include_documents: None,
            prompt_timeout: Default::default(),
            documents: vec![],
            heartbeat_interval: Default::default(),
            core: None,
            dream_enabled: false,
            dream_schedule: None,
            dream_provider: None,
            dream_model: None,
            discord_token: None,
            telegram_token: None,
            avatar_url: None,
            embedding: None,
            indexer: None,
        }
    }

    #[test]
    fn lorem_ipsum_is_non_empty_and_ends_with_a_period() {
        let text = lorem_ipsum();
        assert!(!text.is_empty());
        assert!(text.ends_with('.'));
    }

    #[test]
    fn lorem_ipsum_varies_between_calls() {
        let texts = (0..5).map(|_| lorem_ipsum()).collect::<Vec<_>>();
        assert!(texts.iter().any(|t| t != &texts[0]));
    }

    #[tokio::test]
    async fn prose_gets_one_text_reply_and_zero_usage() {
        let model = DummyplugModel::new(&agent_config(None));
        let (id, content, usage) = model
            .completion(Message::user("hello"), vec![], vec![])
            .await
            .unwrap();
        assert!(id.is_none());
        assert_eq!(content.len(), 1);
        assert!(matches!(content.first(), AssistantContent::Text(_)));
        assert_eq!(usage, Usage::new());
    }

    #[test]
    fn context_window_passes_through_the_config() {
        assert_eq!(
            DummyplugModel::new(&agent_config(Some(1234))).context_window(),
            Some(1234)
        );
        assert_eq!(DummyplugModel::new(&agent_config(None)).context_window(), None);
    }
}
