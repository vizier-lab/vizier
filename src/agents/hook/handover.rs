use anyhow::Result;

use crate::{
    agents::hook::VizierSessionHook,
    schema::{VizierResponse, VizierResponseContent, VizierSession},
};

#[derive(Debug, Clone)]
pub struct HandoverSenderHook {
    response_tx: flume::Sender<VizierResponse>,
    session: VizierSession,
}

impl HandoverSenderHook {
    pub fn new(response_tx: flume::Sender<VizierResponse>, session: VizierSession) -> Self {
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
                timestamp: chrono::Utc::now(),
                content: VizierResponseContent::Checkpoint {
                    handover,
                },
                attachments: vec![],
            })
            .await;

        Ok(())
    }
}
