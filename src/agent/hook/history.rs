use anyhow::Result;

use crate::{
    agent::hook::VizierSessionHook,
    schema::{SessionHistoryContent, VizierRequest, VizierResponse, VizierSession},
    storage::{VizierStorage, history::HistoryStorage},
};

#[derive(Clone)]
pub struct HistoryHook {
    storage: VizierStorage,
    session: VizierSession,
}

impl HistoryHook {
    pub fn new(db: VizierStorage, session: VizierSession) -> Self {
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
        if let VizierResponse::Message {
            content,
            stats: stats,
        } = res.clone()
        {
            self.storage
                .save_session_history(
                    self.session.clone(),
                    SessionHistoryContent::Response(content, stats),
                )
                .await?;
        }

        Ok(res)
    }
}
