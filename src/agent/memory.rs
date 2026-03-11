use rig::message::Message;

use crate::{agent::VizierAgent, config::agent::MemoryConfig, schema::VizierRequest};

#[derive(Debug, Clone)]
pub enum SessionMemory {
    Response(String),
    Request(VizierRequest),
}

impl SessionMemory {
    fn simple(&self) -> String {
        match self {
            Self::Request(req) => format!(
                r"
---
{}: {}
---
                ",
                req.user, req.content
            ),
            Self::Response(content) => format!("answer: {}", content),
        }
    }

    #[allow(unused)]
    fn to_string(&self) -> String {
        match self {
            Self::Request(req) => req.to_prompt().unwrap(),
            Self::Response(content) => content.into(),
        }
    }

    fn to_message(&self) -> Message {
        match self {
            Self::Request(req) => Message::user(req.to_prompt().unwrap()),
            Self::Response(content) => Message::assistant(content),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionMemories {
    messages: Vec<SessionMemory>,
    session_memory_recall_depth: usize,
    summary: Option<String>,
}

impl SessionMemories {
    pub fn new(config: MemoryConfig) -> Self {
        Self {
            messages: vec![],
            session_memory_recall_depth: config.session_memory_recall_depth,
            summary: None,
        }
    }

    pub fn push_user_message(&mut self, req: VizierRequest) {
        self.messages.push(SessionMemory::Request(req));
    }

    pub fn push_agent(&mut self, response: String) {
        self.messages.push(SessionMemory::Response(response));
    }

    pub fn recall(&self) -> Vec<SessionMemory> {
        self.messages
            .iter()
            .rev()
            .take(self.session_memory_recall_depth)
            .map(|item| item.clone())
            .collect()
    }

    pub fn recall_as_messages(&self) -> Vec<Message> {
        self.recall().iter().map(|item| item.to_message()).collect()
    }

    pub async fn try_summarize(&mut self, agent: &VizierAgent) -> anyhow::Result<()> {
        if self.messages.len() < self.session_memory_recall_depth {
            return Ok(());
        }

        let summary_prompt = format!(
            r"
            Provided below is your recent conversation. 
            Summarize and remember it on your memory. 
            make it as concise as possible, yet maintain clarity and avoid information loss as much as possible
            {}",
            self.format_messages_for_summary()
        );

        let response = agent
            .prompt(VizierRequest {
                user: "system".into(),
                content: summary_prompt,
                ..Default::default()
            })
            .await?;

        self.messages.clear();

        self.summary = Some(response);

        Ok(())
    }

    fn format_messages_for_summary(&self) -> String {
        self.messages
            .iter()
            .map(|msg| msg.simple())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn flush(&mut self) {
        self.messages.clear();
        self.summary = None;
    }
}
