use anyhow::Result;

use crate::{
    agent::hook::VizierSessionHook,
    database::VizierDatabases,
    schema::{SessionHistoryContent, VizierRequest, VizierResponse, VizierSession},
};

#[derive(Debug, Clone)]
pub struct HistoryHook {
    db: VizierDatabases,
    session: VizierSession,
}

impl HistoryHook {
    pub fn new(db: VizierDatabases, session: VizierSession) -> Self {
        Self { db, session }
    }
}

#[async_trait::async_trait]
impl VizierSessionHook for HistoryHook {
    async fn on_request(&self, req: VizierRequest) -> Result<VizierRequest> {
        self.db
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
            self.db
                .save_session_history(
                    self.session.clone(),
                    SessionHistoryContent::Response(content, stats),
                )
                .await?;
        }

        Ok(res)
    }
}
