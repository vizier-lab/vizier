# Data model: Prune old tool outputs (060)

No table, column or stored format changes. Everything below is in memory.

## `Message::ToolResult.record`

| Field | Type | Meaning |
|---|---|---|
| `call_id` | `String` | unchanged |
| `content` | `Vec<Part>` | unchanged |
| `record` | `Option<String>` | the `session_history.uid` of this result once it is saved; `None` before a save or when the save failed. Never sent to a provider. `#[serde(default, skip_serializing_if = "Option::is_none")]` |

Set by `history_entries_to_messages` (replay, resume) and by `prompt_with` from
`StepRecorder::record` (live). Kept through checkpoint rebuilds because it is part of the message.

## Pruning constants (`schema/prune.rs`)

| Name | Value | Meaning |
|---|---|---|
| `HIGH_TOKENS` | 60,000 | a prune starts once the eligible unpruned total passes this |
| `LOW_TOKENS` | 30,000 | a prune stops once the total is at or under this |
| `MIN_BYTES` | 2,048 | a smaller result is never pruned |

## `PruneStats`

| Field | Type | Meaning |
|---|---|---|
| `pruned` | `usize` | results replaced by placeholders in this view |
| `tokens_removed` | `u64` | estimated tokens of those results minus their placeholders |

## Placeholder (derived, never stored)

`[output pruned (<size>) - use recall_history("<record>") to recall]` replaces the result's whole
content as one text part. `call_id` and `record` are kept.

## States of a result in the model's view

`whole` → `pruned`. It is one-way within a conversation's view. A compaction drops results from
before it altogether; they stay `whole` in storage and recallable.
