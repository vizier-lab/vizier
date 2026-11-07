use anyhow::Result;
use serde_json::Value;

use crate::{
    agents::hook::VizierSessionHook,
    schema::{VizierResponse, VizierResponseContent, VizierSession},
};

#[derive(Debug, Clone)]
pub struct ThinkingHook {
    response_tx: crate::transport::ResponseSender,
    session: VizierSession,
}

impl ThinkingHook {
    pub fn new(response_tx: crate::transport::ResponseSender, session: VizierSession) -> Self {
        Self {
            response_tx,
            session,
        }
    }
}

#[async_trait::async_trait]
impl VizierSessionHook for ThinkingHook {
    async fn on_tool_call(&self, function_name: String, args: String) -> Result<(String, String)> {
        if function_name == "think"
            && let Some(thought) = thought_from_args(&args)
        {
            let _ = self
                .response_tx
                .send_async(VizierResponse {
                    timestamp: crate::time::now(),
                    content: VizierResponseContent::Thinking(thought),
                    attachments: vec![],
                    ..Default::default()
                })
                .await;
        }

        Ok((function_name, args))
    }

    /// A script calling `think` is as script-internal as any other nested call, so it does
    /// not stream either.
    async fn on_nested_tool_call(
        &self,
        function_name: String,
        args: String,
    ) -> Result<(String, String)> {
        Ok((function_name, args))
    }
}

fn thought_from_args(args: &str) -> Option<String> {
    serde_json::from_str::<Value>(args)
        .ok()?
        .get("thought")?
        .as_str()
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_thinking_arguments_have_no_streamed_thought() {
        assert_eq!(
            thought_from_args(r#"{"thought":"checking the evidence"}"#).as_deref(),
            Some("checking the evidence")
        );
        for args in [
            r#"{"intent":"missing thought"}"#,
            r#"{"thought":42}"#,
            r#""raw model arguments""#,
            "{",
        ] {
            assert_eq!(thought_from_args(args), None, "{args}");
        }
    }
}
