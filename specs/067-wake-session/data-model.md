# Data Model: Waking an agent in any conversation

No table is added. A wake is a call, not a record (research D11), so everything here is a value
type that travels on an existing request or sits inside an existing stored row.

## 1. The new request content kind

```rust
/// A turn the agent started in this conversation itself (spec 067).
///
/// It runs like `Chat`: the conversation's history is in view, the reply is saved into it and
/// shown there. It is not a person's message, so the question tools are not offered and the
/// frontmatter says so (FR-008, FR-009, FR-021).
///
/// The string is the instruction. The framing — that the agent started this turn itself, that
/// it is not a person's message, and that nobody is waiting on the answer here — is a constant
/// the `Display` arm adds (research D4).
Woken(String),
```

Added to the content enum:

```rust
pub enum VizierRequestContent {
    // … the existing kinds …
    /// A turn the agent started in this conversation itself (spec 067 FR-006, FR-008, FR-009).
    Woken(String),
}
```

The kind is placed in the **history arm** of `agents/process.rs::handle_request`, beside `Chat`,
`QuestionAnswer`, `SubagentTask` and `SubagentReply` — the kinds that start a turn with the
conversation's history in view (research D3). It is **not** in the `Unattended` arm, whose turns
run with an empty history.

### What the woken turn carries on the request

| Field | Value | Why |
|---|---|---|
| `content` | `Woken(instruction)` | the work |
| `user` | the calling agent's id | the requester — the agent that started this turn |
| `background_depth` | `ctx.background_depth + 1` | the hop count (FR-020, research D8) |
| `timestamp` | now | `sent_at` in the frontmatter |
| `metadata` | `{}` | nothing to carry |

`consultation_chain` is **not** touched: the woken agent is the caller, so there is no second
agent to put in the chain (research D8). `scheduled_task` stays `None`: a wake is not a scheduled
run, and the field is what turns the frontmatter's `sender` into `scheduler` (research D4).

### The woken turn's frontmatter

```yaml
sender: <the calling agent's id>
via: wake
sent_at: <when the request was sent>
metadata: {}
```

Produced by a new arm in `VizierRequest::generate_frontmatter` (`schema/request.rs:525`), beside the
scheduler's arm and the consultation's arm.

## 2. The conversation reference

A conversation is addressed by two values, both of which already exist:

| Value | Type | Meaning |
|---|---|---|
| the channel | `VizierChannelId` | which conversation. Carried in the tool as its **slug** |
| the topic | `Option<TopicId>` | which conversation within a channel that has topics |

**No agent.** The conversation that is woken always belongs to the agent making the call, and the
system supplies it (research D9), so the address never names an agent. That is also why the
address can be a channel slug and a topic: the agent is not a dimension of the address at all.

The channel slug is `VizierChannelId::to_slug()`:

| Channel | Slug | Topic |
|---|---|---|
| a WebUI conversation | `http__<user>__<channel>` | yes |
| a project chat | `project__<user>__<project id>` | yes |
| a Discord channel | `discord__<channel id>` | no |
| a Telegram chat | `telegram__<chat id>` | no |

`"current"` names the calling turn's own conversation and needs neither of the other two.

### The wakeable set

`wakeable_channel(slug) -> Option<VizierChannelId>` accepts exactly the four kinds above. Every
other slug is refused, which is how FR-014 and FR-015 are enforced:

| Refused | Why |
|---|---|
| `task__…` | a scheduled run's own conversation — the machinery owns it (FR-015) |
| `SUBAGENT` | a worker's conversation — fenced by the hiring session (FR-015) |
| `consultation__…` | a consultation's conversation — fenced by the consultee (FR-015) |
| `DREAM__…` | a dream cycle's conversation (FR-015) |
| `SYSTEM` | the system's own channel (FR-015) |
| `inter_agent__[…]` | the legacy delegation channel (FR-015) |
| `workspace__…` | removed by spec 064 (FR-015) |

## 3. The tools

### `list_sessions`

```rust
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListSessionsArgs {
    /// Only conversations on this channel slug (e.g. "http__ana__vizier-webui").
    #[serde(default)]
    pub channel: Option<String>,
    /// Only conversations whose title contains this.
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionEntry {
    /// The channel slug, as `VizierChannelId::to_slug()` renders it. This is what a wake takes.
    pub channel: String,
    /// The topic, for the channels that have topics.
    pub topic_id: Option<String>,
    /// The conversation's title.
    pub title: String,
    /// Whether a turn is running in it.
    pub is_thinking: bool,
}
```

The source is `storage.get_session_list(agent_id, None)` — every conversation of the calling
agent, across every channel — filtered down to two things: the **wakeable** channels, and the
**conversations that belong to the person this turn is working for**. No new storage method and
no new query (research D15).

A conversation appears in the listing when it has a session record — which is created the first
time a turn runs in it, or when the agent creates one empty. The listing therefore holds the
conversations the agent **has been in**, not every channel a bot is a member of; a wake decides on
its own check, so the listing is a convenience and not the authority.

### `wake_session`

```rust
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct WakeSessionArgs {
    /// The conversation to wake: "current" for the one this turn is in, or a channel slug as
    /// `list_sessions` returns it.
    pub target: String,
    /// The topic within the target channel, for the channels that have topics.
    #[serde(default)]
    pub topic_id: Option<String>,
    /// What the woken agent should do.
    pub instruction: String,
    /// Wait for the woken turn's answer and return it.
    #[serde(default)]
    pub wait: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WakeSessionResult {
    /// The conversation that was woken, as its channel slug and topic.
    pub channel: String,
    pub topic_id: Option<String>,
    /// The woken turn's answer, when the call asked to wait.
    pub reply: Option<String>,
}
```

### The refusal values

Every refusal is data the agent can read, never a failure of the call (FR-024). One shape, one
status per reason:

```json
{ "status": "<status>", "message": "<what the agent can act on>" }
```

| Status | Raised when |
|---|---|
| `unknown_channel` | the target is not a slug the system knows |
| `not_wakeable` | the target names a conversation the machinery owns (FR-015) |
| `no_owner` | the turn has no person whose access could decide (FR-018) |
| `other_person` | the target's conversation belongs to somebody else (FR-017) |
| `too_deep` | the hop limit is reached (FR-020) |
| `bad_request` | the instruction is empty, or the address is malformed |

There is no `unknown_agent` and no `in_chain`: the woken agent is always the caller, so there is
no other agent to reach and no chain to be in (research D9).

## 4. What changes on an existing type

`VizierRequestContent` gains one variant. Every exhaustive match on it — `is_interactive_turn`,
`chat_turn_prompt`, `generate_frontmatter`, the content kind's own `kind()` — becomes a compile
error until the new kind is classified (FR-021, research D5). That is the point of the
exhaustive matches.

`VizierRequest::consultation_chain` does **not** change: not its name, not its type, not its doc
comment. It keeps meaning the consulting agents above a turn, and a wake never touches it
(research D8).

## 5. What is not added

| Not added | Why |
|---|---|
| a `wake` table | a wake is a call, not a record (research D11) |
| a handle, a state machine, a stop | nothing needs to be stopped or followed up |
| a delivery-back mechanism | the answer belongs in the target conversation |
| a new depth setting | the background-work limit already is a hop limit (research D8) |
| a second chain field, or a widened chain | with one agent there is no cycle the chain would catch (research D8) |
| a new storage method | `get_session_list(agent_id, None)` already answers it (research D15) |
| an agent id on either tool | the calling agent is injected by the system (research D9) |
