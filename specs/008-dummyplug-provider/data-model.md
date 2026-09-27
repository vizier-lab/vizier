# Data Model: Dummyplug Test Provider

**Feature**: `008-dummyplug-provider` | **Date**: 2026-09-27

Nothing is persisted: no new SQLite tables or columns, and no new stored provider entries. The only persisted change is that `AgentConfig.provider` (and `dream_provider`, `read_image_settings.provider`) can now hold the value `"dummyplug"`. That field already exists and is stored as a `ProviderVariant` string.

## Entities

### `ProviderVariant::dummyplug` (enum variant, `src/config/provider.rs`)

- Serialized as `"dummyplug"` (the enum uses `#[allow(non_camel_case_types)]` and serde's default naming).
- Has no `ProviderConfig` field, no `ProviderEntryConfig` variant, and no env var.
- Valid anywhere a chat `ProviderVariant` is accepted: the agent's main provider, the dream provider, and the read-image provider.

### `DummyplugModel` (in-memory, `src/agents/agent/model/dummyplug.rs`)

| Field | Type | Source |
|-------|------|--------|
| `context_window` | `Option<u64>` | `agent_config.context_window`, used as-is (no detection) |

This is a stateless request handler. Everything else comes from the arguments of each `completion(message, history, tools)` call. It keeps no state between calls, so it is `Send + Sync` and can be cloned freely behind `VizierModel`'s `Arc`.

### Command text (derived value)

The plain user text extracted from `message` using research R4: frontmatter, the attachment trailer, and code fences are stripped, and the result is trimmed. It is never stored.

### Tool request (wire shape, see `contracts/dummyplug-protocol.md`)

| Field | Type | Required | Rule |
|-------|------|----------|------|
| `tool` | string | yes | Must equal one `ToolDefinition.name` from the current call's `tools` |
| `arguments` | object | no (default `{}`) | Passed through unchanged as `ToolFunction.arguments` and not validated by dummyplug. The tool validates its own input (US4-3). |

Validation outcomes:

| Input | Outcome |
|-------|---------|
| Not valid JSON | Text reply: parse error, with the parser message |
| JSON, but not an object, `tool` missing or not a string, or `arguments` present but not an object | Text reply: shape error, showing the expected shape |
| Valid shape, but `tool` doesn't match any tool | Text reply: unknown tool, suggesting "tools" |
| Valid | A single `AssistantContent::ToolCall` |

### Assistant reply (output of `completion`)

Always `OneOrMany<AssistantContent>` containing exactly one item:
- `AssistantContent::Text` for the listing, sample, error, tool-result echo, and lorem ipsum replies.
- `AssistantContent::ToolCall { id: "dummyplug-<uuid>", call_id: None, function: { name, arguments }, signature: None, .. }` for a valid tool request.

`Usage` is always `Usage::new()` (all zeros), and `message_id` is always `None`.

## State transitions (one agent turn)

```text
user text ──▶ completion #1
               ├─ "tools"            → Text(listing)          → turn ends
               ├─ <tool name>        → Text(sample)           → turn ends
               ├─ "{…" invalid       → Text(error)            → turn ends
               ├─ "{…" valid         → ToolCall ─▶ agent runs tool (hooks, history, UI)
               │                                   └─▶ completion #2 with ToolResult
               │                                         → Text(result echo) → turn ends
               └─ anything else      → Text(lorem ipsum)      → turn ends
```

Each turn makes at most 2 completions, so turns always stay well under `thinking_depth`.
