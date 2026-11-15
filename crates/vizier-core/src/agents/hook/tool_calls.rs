use anyhow::Result;
use serde_json::Value;

use crate::{
    agents::hook::VizierSessionHook,
    sandbox::ExecutionReport,
    schema::{VizierResponse, VizierResponseContent, VizierSession},
};

#[derive(Debug, Clone)]
pub struct ToolCallsHook {
    response_tx: crate::transport::ResponseSender,
    session: VizierSession,
}

impl ToolCallsHook {
    pub fn new(response_tx: crate::transport::ResponseSender, session: VizierSession) -> Self {
        Self {
            response_tx,
            session,
        }
    }
}

#[async_trait::async_trait]
impl VizierSessionHook for ToolCallsHook {
    async fn on_tool_call(&self, function_name: String, args: String) -> Result<(String, String)> {
        if function_name != "think" {
            let args_json: serde_json::Value = serde_json::from_str::<Value>(&args)?;
            let _ = self
                .response_tx
                .send_async(VizierResponse {
                    timestamp: crate::time::now(),
                    content: VizierResponseContent::ToolChoice {
                        name: function_name.clone(),
                        args: args_json,
                    },
                    attachments: vec![],
                    ..Default::default()
                })
                .await;
        }

        Ok((function_name, args))
    }

    /// A script's own tool calls are not streamed. The report the script's run returns is
    /// what the agent reads; the individual calls are the script's business, and one
    /// `ToolChoice` frame per iteration of a loop is noise in the transcript.
    async fn on_nested_tool_call(
        &self,
        function_name: String,
        args: String,
    ) -> Result<(String, String)> {
        Ok((function_name, args))
    }

    async fn on_tool_response(&self, res: VizierResponse) -> Result<VizierResponse> {
        // Only execute_python's report is forwarded live. Identified by shape: nested
        // calls from a script pass through this hook too, so "last tool name seen"
        // would point at the last nested tool, not at execute_python.
        if let VizierResponseContent::ToolResponse { response } = &res.content
            && ExecutionReport::looks_like(response)
        {
            let _ = self.response_tx.send_async(res.clone()).await;
        }

        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::hook::{handover::HandoverSenderHook, thinking::ThinkingHook};

    /// A detached script outlives the turn that launched it, so its hooks publish into a
    /// reply whose reader is gone (spec 058 research D3). Every hook must shrug that off.
    #[tokio::test]
    async fn hooks_tolerate_a_turn_that_has_ended() {
        let (tx, stream) = crate::transport::standalone_reply_channel().await;
        drop(stream);
        let session = VizierSession("a".into(), crate::schema::VizierChannelId::System, None);

        let calls = ToolCallsHook::new(tx.clone(), session.clone());
        assert!(calls.on_tool_call("shell_exec".into(), "{}".into()).await.is_ok());
        let report = serde_json::to_value(ExecutionReport::failed(
            crate::sandbox::ExecutionErrorKind::Script,
            "x",
            "",
            None,
        ))
        .unwrap();
        let res = VizierResponse {
            content: VizierResponseContent::ToolResponse { response: report },
            ..Default::default()
        };
        assert!(calls.on_tool_response(res).await.is_ok());

        let thinking = ThinkingHook::new(tx.clone(), session.clone());
        assert!(thinking.on_tool_call("think".into(), r#"{"thought":"t"}"#.into()).await.is_ok());

        let handover = HandoverSenderHook::new(tx, session);
        assert!(handover.on_handover(Some("h".into())).await.is_ok());
    }
}
