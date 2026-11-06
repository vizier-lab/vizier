use anyhow::Result;

use crate::{
    agents::hook::VizierSessionHook,
    schema::{VizierResponse, VizierResponseContent, VizierSession},
};

#[derive(Debug, Clone)]
pub struct HandoverSenderHook {
    response_tx: crate::transport::ResponseSender,
    session: VizierSession,
}

impl HandoverSenderHook {
    pub fn new(response_tx: crate::transport::ResponseSender, session: VizierSession) -> Self {
        Self {
            response_tx,
            session,
        }
    }
}

#[async_trait::async_trait]
impl VizierSessionHook for HandoverSenderHook {
    async fn on_handover(&self, handover: Option<String>) -> Result<()> {
        let _ = self
            .response_tx
            .send_async(VizierResponse {
                timestamp: crate::time::now(),
                content: VizierResponseContent::Checkpoint {
                    handover,
                },
                attachments: vec![],
                ..Default::default()
            })
            .await;

        Ok(())
    }
}
