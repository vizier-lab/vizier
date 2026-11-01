# Contract — Channel controls: Discord and Telegram

**Feature**: `042-agent-asking-question`

A channel presents the same question set with its own native controls (FR-022). Core never
talks to Discord or Telegram: each reader implements `QuestionSurface` and registers it for its
platform while it runs (research R4), so adding a channel means implementing the trait, not
editing core.

```rust
#[async_trait::async_trait]
pub trait QuestionSurface: Send + Sync {
    fn platform(&self) -> ChannelPlatform;
    /// Post the set. Returning `Err` fails the tool call and keeps the turn running (FR-010).
    async fn present(&self, set: &QuestionSetRecord) -> Result<Controls>;
    /// The set closed: remove, disable, or edit the controls into a record (FR-024).
    async fn close(&self, set: &QuestionSetRecord, controls: &Controls) -> Result<()>;
    /// A short refusal for a stale press: ephemeral where the platform allows it.
    async fn refuse(&self, set_id: &str, who: &str, reason: Refusal) -> Result<()>;
}
```

A platform control carries **only** an 8-character `short_id` plus a question index and an
option index or action. Everything else is resolved from `question_set`, which is why a press
still works after a restart (FR-025) and why the identifiers fit the platform's byte budget.

## 1. Identifiers

```
<short>.<question>.<action>      e.g.  7f3a91c2.db.pick
<short>.<question>.o<index>      e.g.  7f3a91c2.features.o2
<short>.submit | <short>.dismiss | <short>.text
```

| Platform | Field | Limit | Worst case |
|---|---|---|---|
| Discord | `custom_id` | 100 bytes | `7f3a91c2.features.o25` = 22 ✅ |
| Telegram | `callback_data` | 64 bytes | `7f3a91c2.features.o25` = 22 ✅ |

Never JSON, never a prompt or label, never a value: the platform sees only indices.

## 2. Discord

| Question type | Control | Layout |
|---|---|---|
| `choice` ≤ 5 options | one action row of `Button`s | 5 buttons / row |
| `choice` 6–25 options | `SelectMenu` (`StringSelect`) | 25 options / select |
| `multi_choice` ≤ 25 | `SelectMenu` with `min_values`/`max_values` | 25 options |
| `confirm` | two `Button`s (`yes` / `no`) | — |
| `text` | a `Button` that opens a `Modal` with one `TextInput` | 5 inputs / modal, 4 000 chars each |

- **Acknowledgement first** (FR-023, SC-013): a press is answered with
  `InteractionResponseType::DeferredUpdateMessage` before any storage read, using
  `InteractionResponseDataBuilder` from `twilight_util` (already a dependency; the gateway
  already subscribes `INTERACTION_CREATE`, `channels/discord/mod.rs:105-111`, `:172-174`).
- `multi_choice` and `text` need a submit step: the select/modal records its own question's
  answer and edits the message to show it; the last required question submits the set.
- After the set closes, the message is edited into a record of the answers with the action rows
  removed. A press on a removed/closed control is answered ephemerally and starts no turn.
- No new permission and no privileged intent: components, modals and `interaction` responses
  are all in the existing scope (FR-031).

## 3. Telegram

| Question type | Control | Note |
|---|---|---|
| `choice` | one row of `InlineKeyboardButton`s per option | one press answers it |
| `multi_choice` | toggle rows, `☐`/`☑`, plus a `Submit` row | the press edits the keyboard |
| `confirm` | two buttons | — |
| `text` | reply to the question message; the reply body is the answer | FR-027's documented text form |

- `AllowedUpdate::CallbackQuery` is added to the poller's `allowed_updates`
  (`channels/telegram/mod.rs:172-176`) and `UpdateKind::CallbackQuery` to `handle_update`
  (`:212-230`); every callback is answered with `answer_callback_query` before any work.
- One message per question when a set has more than one question, each carrying the same
  `short_id`; the set submits once every required question is answered (FR-029 — split, never
  merge or drop).
- After the set closes, each message is edited to its answers and its keyboard removed.

## 4. The text form (FR-027)

Used when a set cannot be expressed natively: more than 25 options on one question, more
questions than the platform's message budget comfortably carries, or a free-text question where
the platform has no form (Telegram outside a reply flow). It is one message, one submission:

```text
Project setup (set 7f3a91c2)

1. name — Project name  (text)
2. db — Database  (one of: postgres, sqlite)  *
3. features — Features  (any of: auth, ci)

Reply to this message with one line per question:
  name: …
  db: …
  features: …
```

Parsing rules: `key: value` per line, key = the question id; `*` marks required; an unknown
key, a missing required key, a value outside the options, or an over-limit text refuses the
whole submission with the reason and keeps the set open. The parser lives in
`questions/text_form.rs` and is unit-tested independently of any platform, so every surface
shares one grammar.

## 5. What a channel must never do

- Never answer a set that is not `pending` in the conversation it came from (the atomic claim
  decides; a loser gets `refuse`).
- Never accept an answer from someone who cannot post in the conversation (the platform
  decides who may press, and `answerer` records the platform identity).
- Never render an answer as markup, a command, or a mention that could ping someone: answer
  text is escaped/plain in the record (FR-059).
- Never post a sensitive-input dialog: `ask_sensitive_input` is not offered on a channel at all
  (FR-046).
