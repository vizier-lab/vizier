//! Shared helpers for `#[cfg(test)]` modules. Not compiled into the binary.

use std::sync::{Mutex, MutexGuard};

use crate::schema::AgentConfig;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Serialize tests that read or mutate process environment variables: cargo runs tests in
/// parallel threads, and one test's `set_var` is every other test's environment.
pub fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A minimal, valid dummyplug agent config, so tests that need a stored agent can create one.
pub fn agent_config(id: &str) -> AgentConfig {
    AgentConfig {
        name: format!("Agent {id}"),
        owner_id: None,
        shared_to: vec![],
        system_prompt: None,
        description: None,
        provider: crate::config::provider::ProviderId::DUMMYPLUG,
        model: "dummyplug".into(),
        thinking_depth: 1,
        checkpoint_threshold: 0.8,
        tools: Default::default(),
        silent_read_initiative_chance: 0.0,
        max_tokens: None,
        context_window: None,
        include_documents: None,
        prompt_timeout: Default::default(),
        documents: vec![],
        heartbeat_interval: Default::default(),
        core: None,
        dream_enabled: false,
        dream_schedule: None,
        dream_provider: None,
        dream_model: None,
        avatar_url: None,
        embedding: None,
        indexer: None,
        chunking: Default::default(),
        auto_context: Default::default(),
        completion_retry: Default::default(),
    }
}
