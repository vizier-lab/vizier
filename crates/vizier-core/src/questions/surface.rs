//! How one channel presents a set, and the per-platform registry of surfaces (research R4).
//!
//! Core never names Discord or Telegram: a channel reader implements [`QuestionSurface`] and
//! registers it while it runs, and `ask_question` resolves the surface for the conversation's
//! platform and awaits it. That is why a set whose platform post failed can fail the tool call
//! and keep the turn running (FR-010) instead of disappearing into a best-effort frame.
//!
//! The WebUI needs no surface: its presentation is the history entry plus the frame
//! `record_step` already publishes.

use std::collections::HashMap;
use std::sync::{Arc, Weak};

use parking_lot::RwLock;

use crate::questions::model::{Controls, QuestionSetRecord};
use crate::schema::ChannelPlatform;

/// Why a press or a submission was refused, in words a person reads (FR-024).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Another person, or another tab, answered it first.
    AlreadyAnswered,
    /// It was dismissed.
    Declined,
    /// The script that asked failed, so the set was taken away.
    Withdrawn,
    /// The person cannot post in this conversation.
    NotAllowed,
    /// The press carried something the set does not have — an option that was never offered, or
    /// a question that has since gone.
    UnknownChoice,
    /// An answer that broke a rule; the surface says which.
    Invalid(String),
}

impl Refusal {
    /// The one line shown for this refusal.
    pub fn message(&self) -> String {
        match self {
            Self::AlreadyAnswered => "This was already answered.".to_string(),
            Self::Declined => "This was dismissed.".to_string(),
            Self::Withdrawn => "This question was withdrawn.".to_string(),
            Self::NotAllowed => "You cannot answer here.".to_string(),
            Self::UnknownChoice => "That choice is not part of this question.".to_string(),
            Self::Invalid(reason) => reason.clone(),
        }
    }
}

/// One channel's way of showing a set (contracts/channel-controls.md).
///
/// Every method is fallible: a platform call that fails becomes an error `ask_question` hands
/// back to the agent, never a silent no-op.
#[async_trait::async_trait]
pub trait QuestionSurface: Send + Sync {
    /// The platform this surface posts to.
    fn platform(&self) -> ChannelPlatform;

    /// Post the set and say where it went.
    async fn present(&self, set: &QuestionSetRecord) -> anyhow::Result<Controls>;

    /// The set closed: strip its controls, or edit its messages into a record of the answers
    /// (FR-024). A failure is logged by the caller and changes nothing else.
    async fn close(&self, set: &QuestionSetRecord, controls: &Controls) -> anyhow::Result<()>;

    /// Tell one person, quietly, why their press did nothing.
    async fn refuse(&self, set: &QuestionSetRecord, who: &str, refusal: Refusal)
    -> anyhow::Result<()>;
}

/// Which surface posts for which conversation, by agent and platform.
///
/// A reader registers a **weak** reference, so a surface that stops with its reader stops being
/// found: there is nothing to unregister, and a stale entry cannot post through a dead bot.
#[derive(Default, Clone)]
pub struct PlatformSurfaces {
    surfaces: Arc<RwLock<HashMap<(String, ChannelPlatform), Weak<dyn QuestionSurface>>>>,
}

impl std::fmt::Debug for PlatformSurfaces {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let live = self.surfaces.read().values().filter(|weak| weak.strong_count() > 0).count();
        f.debug_struct("PlatformSurfaces").field("live", &live).finish()
    }
}

impl PlatformSurfaces {
    /// Announce `surface` for this agent while the caller keeps it alive.
    pub fn register(&self, agent_id: &str, surface: &Arc<dyn QuestionSurface>) {
        let key = (agent_id.to_string(), surface.platform());
        self.surfaces.write().insert(key, Arc::downgrade(surface));
    }

    /// The surface that posts for this agent and platform, if one is still running. A dead
    /// entry is pruned on the way out.
    pub fn surface(
        &self,
        agent_id: &str,
        platform: ChannelPlatform,
    ) -> Option<Arc<dyn QuestionSurface>> {
        let key = (agent_id.to_string(), platform);
        let weak = self.surfaces.read().get(&key).cloned()?;
        match weak.upgrade() {
            Some(surface) => Some(surface),
            None => {
                self.surfaces.write().remove(&key);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::model::QuestionState;
    use crate::schema::{TopicId, VizierChannelId, VizierSession};
    use chrono::Utc;

    struct FakeSurface {
        platform: ChannelPlatform,
    }

    #[async_trait::async_trait]
    impl QuestionSurface for FakeSurface {
        fn platform(&self) -> ChannelPlatform {
            self.platform
        }

        async fn present(&self, set: &QuestionSetRecord) -> anyhow::Result<Controls> {
            Ok(Controls {
                platform: self.platform.to_string(),
                messages: vec![crate::questions::model::ControlMessage {
                    chat_id: "1".into(),
                    message_id: set.set_id.clone(),
                    question: None,
                }],
            })
        }

        async fn close(&self, _set: &QuestionSetRecord, _controls: &Controls) -> anyhow::Result<()> {
            Ok(())
        }

        async fn refuse(
            &self,
            _set: &QuestionSetRecord,
            _who: &str,
            _refusal: Refusal,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn record() -> QuestionSetRecord {
        QuestionSetRecord {
            set_id: "s1".into(),
            short_id: "7f3a91c2".into(),
            session: VizierSession(
                "a".into(),
                VizierChannelId::DiscordChanel(1),
                Some("t".into()),
            ),
            state: QuestionState::Pending,
            set: crate::questions::model::QuestionSet {
                title: None,
                description: None,
                questions: vec![],
                submit_label: None,
                dismiss_label: None,
            },
            answers: None,
            answerer: None,
            controls: None,
            created_at: Utc::now(),
            closed_at: None,
        }
    }

    /// A registered surface is found for its own agent and platform only.
    #[test]
    fn a_surface_is_found_by_agent_and_platform() {
        let surfaces = PlatformSurfaces::default();
        let discord: Arc<dyn QuestionSurface> = Arc::new(FakeSurface {
            platform: ChannelPlatform::Discord,
        });
        surfaces.register("a", &discord);

        assert!(surfaces.surface("a", ChannelPlatform::Discord).is_some());
        assert!(surfaces.surface("a", ChannelPlatform::Telegram).is_none());
        assert!(surfaces.surface("b", ChannelPlatform::Discord).is_none());
    }

    /// When the reader that registered one stops, the surface goes with it: nothing has to
    /// remember to unregister, and a stale entry cannot post through a dead bot.
    #[test]
    fn a_surface_stops_with_its_reader() {
        let surfaces = PlatformSurfaces::default();
        let discord: Arc<dyn QuestionSurface> = Arc::new(FakeSurface {
            platform: ChannelPlatform::Discord,
        });
        surfaces.register("a", &discord);
        assert!(surfaces.surface("a", ChannelPlatform::Discord).is_some());

        drop(discord);
        assert!(surfaces.surface("a", ChannelPlatform::Discord).is_none());
        // And the dead entry is gone rather than left to be retried.
        assert_eq!(surfaces.surfaces.read().len(), 0);
    }

    /// Registering a second surface for the same agent and platform replaces the first, which is
    /// what a reconnected reader does.
    #[test]
    fn a_reconnected_reader_replaces_its_surface() {
        let surfaces = PlatformSurfaces::default();
        let first = Arc::new(FakeSurface {
            platform: ChannelPlatform::Telegram,
        });
        let first_dyn: Arc<dyn QuestionSurface> = first.clone();
        surfaces.register("a", &first_dyn);

        let second = Arc::new(FakeSurface {
            platform: ChannelPlatform::Telegram,
        });
        let second_dyn: Arc<dyn QuestionSurface> = second.clone();
        surfaces.register("a", &second_dyn);

        let found = surfaces.surface("a", ChannelPlatform::Telegram).unwrap();
        assert!(Arc::ptr_eq(&found, &second_dyn));

        // The old one going away must not take the new registration with it.
        drop(first_dyn);
        drop(first);
        assert!(surfaces.surface("a", ChannelPlatform::Telegram).is_some());
    }

    /// `present` reports where it posted, which is what lets a failure fail the tool call.
    #[tokio::test]
    async fn present_reports_where_it_posted() {
        let surface = FakeSurface {
            platform: ChannelPlatform::Discord,
        };
        let controls = surface.present(&record()).await.unwrap();
        assert_eq!(controls.platform, "discord");
        assert_eq!(controls.messages.len(), 1);
    }

    /// A refusal always says something a person can read.
    #[test]
    fn every_refusal_has_a_message() {
        for refusal in [
            Refusal::AlreadyAnswered,
            Refusal::Declined,
            Refusal::Withdrawn,
            Refusal::NotAllowed,
            Refusal::UnknownChoice,
            Refusal::Invalid("too long".into()),
        ] {
            assert!(!refusal.message().is_empty());
        }
        assert_eq!(Refusal::Invalid("x".into()).message(), "x");
    }
}
