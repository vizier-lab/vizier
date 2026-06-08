use std::sync::Arc;

use anyhow::Result;

use crate::{
    agents::hook::VizierSessionHook,
    schema::{
        SessionHistoryContent, VizierRequest, VizierResponse, VizierResponseContent, VizierSession,
    },
    storage::{VizierStorage, history::HistoryStorage},
};

#[derive(Clone)]
pub struct HistoryHook {
    storage: Arc<VizierStorage>,
    session: VizierSession,
}

impl HistoryHook {
    pub fn new(db: Arc<VizierStorage>, session: VizierSession) -> Self {
        Self {
            storage: db,
            session,
        }
    }
}

#[async_trait::async_trait]
impl VizierSessionHook for HistoryHook {
    async fn on_request(&self, req: VizierRequest) -> Result<VizierRequest> {
        self.storage
            .save_session_history(
                self.session.clone(),
                SessionHistoryContent::Request(req.clone()),
            )
            .await?;

        Ok(req)
    }

    async fn on_response(&self, res: VizierResponse) -> Result<VizierResponse> {
        if matches!(
            &res.content,
            VizierResponseContent::Message { .. } | VizierResponseContent::AudioReply(..)
        ) {
            self.storage
                .save_session_history(
                    self.session.clone(),
                    SessionHistoryContent::Response(res.clone()),
                )
                .await?;
        }

        Ok(res)
    }
}
