# Contract: WebUI Model Override

**Feature**: `049-model-override` | **Specification**: [spec.md](../spec.md)

## 1. Permission

The only permission name is the exact string `model_override`. It is registered in the available role permissions and is checked on the server for every request carrying an override. No `all_agents:model_override` or `owned_agents:model_override` permission exists. A system role follows the existing permission bypass. Hiding the control is presentation only; a missing permission rejects a supplied override.

## 2. Available choices

The WebUI reuses the authenticated catalogue provider/model endpoints. It offers chat-capable eligible providers and models; media-only providers are omitted. Provider icons are optional metadata. The user-visible label is exactly:

```text
<provider-id>/<complete-model-id>
```

For example, provider `opencode` and model `deepseek/deepseek-flash` render as `opencode/deepseek/deepseek-flash`. The provider and model are separate fields; the model portion is never split on `/`.

An `Agent default` option clears the local override. If the catalogue request fails, the UI reports that choices could not be loaded. If no eligible choices exist, the selector reports that state and offers no unusable model. A saved choice no longer present is identified as unavailable; it is not silently replaced.

## 3. Preference and scope

The WebUI stores the preference locally by signed-in `user_id`, `agent_id`, `channel_id`, and `topic_id`. It survives reopening that conversation in the same browser profile. It is not synchronized to another browser/device. Selecting a new model changes only future submissions; in-flight submissions are immutable.

The selector is one dropdown in the open space immediately to the right of the existing status-strip controls, on their row below Send. The status strip keeps its existing appearance and behavior. Flexible space separates the strip and dropdown. Discord and Telegram have no override controls or request changes.

## 4. User request value

The shared Rust user-chat request has an optional, default-absent value with this shape:

```json
{
  "model_override": {
    "provider": "opencode",
    "model": "deepseek/deepseek-flash"
  }
}
```

An absent or null `model_override` means use the agent's configured model. The same value may be supplied to the existing authenticated REST chat request as well as the WebSocket/desktop topic chat. Legacy clients omitting it are unchanged. The override is accepted only on a user-originated `chat` or `audio_chat` request in the WebUI topic channel (including the desktop WebUI). Supplying it on `client_event`, `question_answer`, a command, a side chat, a project tool request, or any machine/channel request is rejected rather than ignored. The existing authenticated REST chat accepts the same optional value; it does not expose a saved selector preference. Discord and Telegram do not send it.

When sending, the WebUI snapshots the selected choice into the request; a later selector change does not alter a sent or queued request. Durable turns retain the field for resume.

## 5. Server behavior

For every supplied override, before beginning model execution, the server:

1. Resolves the requester's current permissions (including on a still-open topic connection) and requires `model_override`.
2. Preserves existing agent visibility and project-channel authorization.
3. Checks the exact provider/model pair against the current eligible chat choices and resolves it through the normal provider resolver.
4. Constructs an isolated model for this request/turn; it does not mutate the configured agent model or another session's model.

Omitting an override preserves the existing request path. An explicit invalid, unavailable, unauthorized, or unresolvable override MUST fail. No case may retry the prompt against the configured/default model silently.

## 6. Refusal and failure behavior

| Condition | Behavior |
|---|---|
| Missing permission | Reject before dispatch; topic transports send a client error with code `model_override_forbidden`; REST returns 403 using the existing error envelope. |
| Override supplied on a request kind/channel outside the supported user chat paths | Reject before dispatch; topic transport returns `model_override_unsupported`; REST returns 422. |
| Provider/model malformed, not eligible, removed, or not resolvable | Reject before dispatch where possible; topic transport sends `model_override_unavailable`; REST returns 422 using the existing error envelope. |
| Model/provider becomes unavailable after preflight | The turn ends with the existing classified provider/completion error; no fallback request is issued. |
| No eligible choices | UI communicates the empty state and presents no unavailable model as selectable. |
| No override | Existing configured model and existing error behavior are unchanged. |

Error codes above are stable, non-secret identifiers. Error messages must not disclose provider credentials or secret values.

## 7. Compatibility

- Older requests omit the new field and deserialize as no override.
- No agent configuration or persistent conversation schema changes.
- The response's existing `stats.provider` and `stats.model` identify the model actually used.
- Usage entries must identify the actual selected provider/model.
- Discord and Telegram behavior remains unchanged; they continue to use the configured agent model.
