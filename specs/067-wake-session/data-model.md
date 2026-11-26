# Data Model: Waking an agent in any conversation

No table is added. A wake is a call, not a record (research D11), so everything here is a value
type that travels on an existing request or sits inside an existing stored row.

## 1. The new request content kind

```rust
/// A turn started by another agent in this conversation (spec 067).
///
/// It runs like `Chat`: the conversation's history is in view, the reply is saved into it and
/// shown there. It is not a person's message, so the question tools are not offered and the
/// frontmatter says who asked (FR-008, FR-009, FR-023).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WokenRequest {
    /// The agent that asked for this work.
    pub from_agent: AgentId,
    /// What it asked for.
    pub instruction: String,
}
```

Added to the content enum:

```rust
pub enum VizierRequestContent {
    // … the existing kinds …
    /// A turn another agent started in this conversation (spec 067 FR-006, FR-008, FR-009).
    Woken(WokenRequest),
}
```

The kind is placed in the **history arm** of `agents/process.rs::handle_request`, beside `Chat`,
`QuestionAnswer`, `SubagentTask` and `SubagentReply` — the kinds that start a turn with the
conversation's history in view (research D3). It is **not** in the `Unattended` arm, whose turns
run with an empty history.

### What the woken turn carries on the request

| Field | Value | Why |
|---|---|---|
| `content` | `Woken(WokenRequest { from_agent, instruction })` | the work, and who asked for it |
| `user` | the waking agent's id | the requester, as `Chat` carries a person's name |
| `background_depth` | `ctx.background_depth + 1` | the hop count (FR-021, research D7) |
| `consultation_chain` | the caller's chain **plus the waking agent** | who is above this turn (FR-022, research D8) |
| `timestamp` | now | `sent_at` in the frontmatter |
| `metadata` | `{}` | nothing to carry |

`scheduled_task` stays `None`: a wake is not a scheduled run, and the field is what turns the
frontmatter's `sender` into `scheduler` (research D4).

### The woken turn's frontmatter

```yaml
sender: <the waking agent's id>
via: wake
sent_at: <when the request was sent>
metadata: {}
```

Produced by a new arm in `VizierRequest::generate_frontmatter` (`schema/request.rs:525`), beside the
scheduler's arm and the consultation's arm.

## 2. The conversation reference

A conversation is addressed by three values, all of which already exist:

| Value | Type | Meaning |
|---|---|---|
| the owning agent | `AgentId` | whose agent runs the turn. Defaults to the calling agent |
| the channel | `VizierChannelId` | which conversation. Carried in the tool as its **slug** |
| the topic | `Option<TopicId>` | which conversation within a channel that has topics |

The channel slug is `VizierChannelId::to_slug()`:

| Channel | Slug | Topic |
|---|---|---|
| a WebUI conversation | `http__<user>__<channel>` | yes |
| a project chat | `project__<user>__<project id>` | yes |
| a Discord channel | `discord__<channel id>` | no |
| a Telegram chat | `telegram__<chat id>` | no |

`"current"` names the calling turn's own conversation and needs none of the other two.

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

### Why the agent id is not inside the slug

An agent id is a user-supplied free-form string (`service/agents.rs::create` takes it verbatim and
the FS backend uses it as a file name), and a WebUI topic id is a `nanoid` of length 10 from the
`A-Za-z0-9_-` alphabet. Both may contain `__`, so packing them into one `__`-separated address
would make it ambiguous to parse (research D1).

## 3. The tools

### `list_sessions`

```rust
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListSessionsArgs {
    /// Whose conversations to list. Omit for the agent making the call.
    #[serde(default)]
    pub agent_id: Option<String>,
    /// Only conversations on this channel slug (e.g. "http__ana__vizier-webui").
    #[serde(default)]
    pub channel: Option<String>,
    /// Only conversations whose title contains this.
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionEntry {
    /// The agent that owns the conversation.
    pub agent_id: String,
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

The source is `storage.get_session_list(agent_id, None)` — every conversation of that agent, across
every channel — filtered down to the wakeable channels and to the agents this turn may reach
(research D15). No new storage method and no new query.

### `wake_session`

```rust
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct WakeSessionArgs {
    /// The conversation to wake: "current" for the one this turn is in, or a channel slug as
    /// `list_sessions` returns it.
    pub target: String,
    /// The agent that owns the target conversation. Omit for the calling agent.
    #[serde(default)]
    pub agent_id: Option<String>,
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
    pub agent_id: String,
    pub channel: String,
    pub topic_id: Option<String>,
    /// The woken turn's answer, when the call asked to wait.
    pub reply: Option<String>,
}
```

### The refusal values

Every refusal is data the agent can read, never a failure of the call (FR-026). One shape, one
status per reason:

```json
{ "status": "<status>", "message": "<what the agent can act on>" }
```

| Status | Raised when |
|---|---|
| `unknown_channel` | the target is not a slug the system knows |
| `not_wakeable` | the target names a conversation the machinery owns (FR-015) |
| `unknown_agent` | the agent does not exist, or the person may not use it — one answer for both (FR-017) |
| `no_owner` | the turn has no person whose access could decide (FR-019) |
| `other_person` | the target's conversation belongs to somebody else (FR-018) |
| `self` | the target agent is the calling agent's own other conversation is allowed; this is for the agent refusing to wake itself — see below |
| `in_chain` | the target agent is already above this turn (FR-022) |
| `too_deep` | the hop limit is reached (FR-021) |
| `bad_request` | the instruction is empty, or the address is malformed |

The `self` status is only for the case where a turn asks to wake an agent that is already the
caller: that is a consultation, and the refusal says so.

## 4. What changes on an existing type

`VizierRequest::consultation_chain` keeps its name, its type and its storage. Only its **doc
comment** changes, to say that it is the agents above this turn — consultations and wakes alike
(research D8). No stored row changes shape, and nothing reads the field that would misread it.

`VizierRequestContent` gains one variant. Every exhaustive match on it — `is_interactive_turn`,
`chat_turn_prompt`, `generate_frontmatter`, the content kind's own `kind()` — becomes a compile
error until the new kind is classified (FR-023, research D5). That is the point of the
exhaustive matches.

## 5. What is not added

| Not added | Why |
|---|---|
| a `wake` table | a wake is a call, not a record (research D11) |
| a handle, a state machine, a stop | nothing needs to be stopped or followed up |
| a delivery-back mechanism | the answer belongs in the target conversation |
| a new depth setting | the background-work limit already is a hop limit (research D7) |
| a second chain field | one chain is "the agents above this turn" (research D8) |
| a new storage method | `get_session_list(agent_id, None)` already answers it (research D15) |
