use anyhow::Result;

use crate::schema::{VizierSession, session_file::SessionFileRecord};

/// How many numbered names `add_session_file` tries before it gives up.
pub const MAX_NUMBERED_NAMES: u32 = 1000;

/// `report.pdf` → `report (2).pdf`; a name with no extension, or a dot file, gets the number
/// at the end (`notes (2)`, `.env (2)`).
pub fn numbered_name(name: &str, n: u32) -> String {
    match name.rfind('.') {
        Some(dot) if dot > 0 => format!("{} ({n}){}", &name[..dot], &name[dot..]),
        _ => format!("{name} ({n})"),
    }
}

#[async_trait::async_trait]
pub trait SessionFileStorage {
    /// Save a file under `filename`, **replacing** whatever the conversation had under that
    /// name. For tools that write a file by a name the agent chose (their descriptions promise
    /// the replace).
    async fn save_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
        mime_type: &str,
        size: u64,
        file_id: &str,
    ) -> Result<SessionFileRecord>;

    /// Save a file that arrived (a person's attachment, a file a tool or script returned)
    /// **keeping** any other file of that name: when `filename` is taken by a different upload
    /// it is saved as `name (2).ext`, `name (3).ext`, …. Saving the same upload again under its
    /// name updates it in place. The returned record carries the name used.
    ///
    /// This default is not atomic; the SQL backend overrides it with one transaction.
    async fn add_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
        mime_type: &str,
        size: u64,
        file_id: &str,
    ) -> Result<SessionFileRecord> {
        let mut name = filename.to_string();
        for n in 2..=MAX_NUMBERED_NAMES + 1 {
            match self.get_session_file(session, &name).await? {
                Some(existing) if existing.file_id != file_id => name = numbered_name(filename, n),
                _ => return self.save_session_file(session, &name, mime_type, size, file_id).await,
            }
        }
        anyhow::bail!("too many files named '{filename}' in this conversation")
    }

    async fn list_session_files(&self, session: &VizierSession) -> Result<Vec<SessionFileRecord>>;

    async fn get_session_file(
        &self,
        session: &VizierSession,
        filename: &str,
    ) -> Result<Option<SessionFileRecord>>;

    async fn delete_session_file(&self, session: &VizierSession, filename: &str) -> Result<()>;

    /// Delete every record of one conversation (`specs/035-add-artifacts`).
    async fn delete_session_files(&self, session: &VizierSession) -> Result<usize>;

    /// Every session file an agent has, across conversations, for agent deletion.
    async fn list_agent_files(&self, agent_id: &str) -> Result<Vec<SessionFileRecord>>;

    /// How many session-file records reference an upload blob (a blob can be shared by a memory
    /// read re-saving the same `file_id`). Zero means the blob can be removed.
    async fn session_file_reference_count(&self, file_id: &str) -> Result<usize>;

    /// Sets who a saved file came from and the history entry it arrived with. Files a tool
    /// makes are labelled with the agent id when saved; a person's attachment is relabelled
    /// here once the request is saved. A backend that doesn't track this does nothing.
    async fn label_session_file(
        &self,
        _session: &VizierSession,
        _filename: &str,
        _sender: Option<&str>,
        _history_uid: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }

    /// Saves and relabels, as they happen, for whoever shows the session live.
    fn subscribe_session_files(
        &self,
    ) -> Option<tokio::sync::broadcast::Receiver<(VizierSession, SessionFileRecord)>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::numbered_name;

    #[test]
    fn the_number_goes_before_the_extension() {
        assert_eq!(numbered_name("report.pdf", 2), "report (2).pdf");
        assert_eq!(numbered_name("archive.tar.gz", 3), "archive.tar (3).gz");
        assert_eq!(numbered_name("notes", 2), "notes (2)");
        assert_eq!(numbered_name(".env", 2), ".env (2)");
    }
}
