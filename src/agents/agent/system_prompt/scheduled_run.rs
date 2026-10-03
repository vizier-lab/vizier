/// What the agent is told when a task fires.
///
/// **Constant by design.** No task name, no requester, no timestamp — those are in the
/// request frontmatter. A constant string is what keeps the prompt prefix reusable between
/// runs of a session, and it is also why nothing here can be got wrong per-run.
///
/// **Appended last**, after CORE and documents: last is where situational context belongs,
/// and it leaves the boot/CORE prefix byte-identical to an interactive turn's. The content,
/// not the ordering, is what keeps the framing from touching the agent's character — it
/// describes the situation and says nothing about tone or persona. A warm agent writes a
/// warm report.
///
/// **Selected on the session's channel**, never on the request content kind: the dream
/// cycle sends `VizierRequestContent::Unattended` too, while carrying its own framing in
/// `EXTRACTION_PROMPT`. See `VizierSession::is_scheduled_task()`.
pub fn scheduled_run_md() -> String {
    SCHEDULED_RUN_MD.to_string()
}

const SCHEDULED_RUN_MD: &str = r#"# SCHEDULED RUN

This turn is a scheduled task run, not a conversation. Nobody is reading as you
work, and nothing you write here reaches a person until they open the task later.

- **No questions.** Anything you ask goes unanswered and the run just ends. Where
  something is ambiguous, take the most reasonable reading, say which assumption
  you made, and continue.
- **Act, don't offer.** Use your tools to do the work now. "I can do X if you'd
  like" is a dead end here.
- **To reach someone, send to them.** If the task calls for telling or asking a
  person something, use a tool that delivers to them. Writing it in this turn
  does not.
- **Your last message is the report.** It is what a person sees when they open
  the task. Lead with the outcome, then what you did, and anything needing their
  attention — a failure, a blocker, a judgement call you had to make. No
  greeting, no sign-off, no offer of further help."#;

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::schema::{DreamStage, VizierChannelId, VizierSession};

    fn session(channel: VizierChannelId) -> VizierSession {
        VizierSession("agent-1".to_string(), channel, None)
    }

    fn task_channel() -> VizierChannelId {
        VizierChannelId::Task(
            "daily-report".to_string(),
            Utc.timestamp_opt(1_700_000_000, 0).single().unwrap(),
        )
    }

    /// The framing reaches a scheduled task run.
    #[test]
    fn a_scheduled_task_session_is_framed() {
        assert!(session(task_channel()).is_scheduled_task());
    }

    /// **The trap this whole module exists to avoid.** The dream cycle dispatches its work
    /// as `VizierRequestContent::Unattended`, exactly as the scheduler does, while carrying
    /// its own framing in `EXTRACTION_PROMPT`. Selecting the framing on the request content
    /// kind would therefore double-frame every dream, handing the agent two briefs that
    /// disagree. The channel is what distinguishes them; the content kind does not.
    #[test]
    fn a_dream_session_is_not_framed() {
        let inner = session(VizierChannelId::HTTP(
            "someone".to_string(),
            "webui".to_string(),
        ));
        let dream = session(VizierChannelId::Dream(
            Box::new(inner),
            DreamStage::Extraction,
        ));
        assert!(!dream.is_scheduled_task());

        // Including a dream *of* a task session, which nests a `Task` channel inside — the
        // framing question is about the outer channel, not anything reachable within it.
        let dream_of_task = session(VizierChannelId::Dream(
            Box::new(session(task_channel())),
            DreamStage::Consolidation,
        ));
        assert!(!dream_of_task.is_scheduled_task());
    }

    /// Interactive turns match nothing here and are untouched.
    #[test]
    fn interactive_sessions_are_not_framed() {
        for channel in [
            VizierChannelId::HTTP("someone".to_string(), "webui".to_string()),
            VizierChannelId::DiscordChanel(42),
            VizierChannelId::TelegramChannel(42),
            VizierChannelId::System,
            VizierChannelId::Subagent,
            VizierChannelId::InterAgent(vec!["agent-2".to_string()]),
        ] {
            assert!(
                !session(channel.clone()).is_scheduled_task(),
                "{channel:?} must not be framed as a scheduled run"
            );
        }
    }

    /// FR-033: constant per run, so the prompt prefix stays reusable between runs.
    #[test]
    fn the_framing_is_byte_identical_across_calls() {
        assert_eq!(scheduled_run_md(), scheduled_run_md());
    }

    /// The section carries no task name, requester or timestamp — those live in the request
    /// frontmatter, and putting them here is what would make the prefix vary per run.
    #[test]
    fn the_framing_carries_nothing_per_run() {
        let md = scheduled_run_md();
        assert!(md.starts_with("# SCHEDULED RUN"));
        for leak in ["daily-report", "agent-1", "2023", "sender", "requested_by"] {
            assert!(
                !md.contains(leak),
                "the framing must stay constant, but it mentions {leak:?}"
            );
        }
    }
}
