# Data Model: Model Override for Chat Requests

## Entities

### Model choice

One selectable model, sourced from the current server catalogue.

| Field | Type | Rules |
|---|---|---|
| `provider` | provider ID string (`ProviderId` on the Rust side) | Required when an override is present; must parse as an existing chat provider and be currently eligible/usable. |
| `model` | full model identifier string | Required when an override is present; exact, case-sensitive match against that provider's eligible chat models. Opaque: preserve all `/` characters. |
| `label` | derived display string | `<provider-id>/<full-model-id>`; not authoritative and never parsed back into the two identifiers. |
| `provider icon` | optional logo URL/asset | Derived from provider metadata; absent icons do not make a model unavailable. |

### Conversation model preference (client-side only)

The most recent selection for subsequent prompts in one conversation. It is not server-authoritative and is not a database entity.

| Field | Type | Rules |
|---|---|---|
| `user_id` | string | Part of the local preference key, preventing one signed-in user from inheriting another's selection in the same browser. |
| `agent_id` | string | Part of the key. |
| `channel_id` | string | Part of the key; separates ordinary and project conversations. |
| `topic_id` | string | Part of the key; separates conversations for the same agent. |
| `choice` | model choice or Agent default (`null`) | Saved on selection. If the choice ceases to be available, retain/display it as unavailable and refuse to submit it until the user chooses a valid option or Agent default. |

The browser preference is keyed by `(user_id, agent_id, channel_id, topic_id)`. Browser storage is best-effort via existing guarded helpers. It is not synchronized across browsers/devices.

### Request model override

An optional, per-request choice. This is the authoritative value for that turn and is carried on the existing user-chat request, not in agent configuration.

| Field | Type | Rules |
|---|---|---|
| `model_override` | optional `{ provider: string, model: string }` | Absent means use the configured agent model. Present means authorize and validate; any failure rejects the turn and must never be converted to absence or replaced by another choice. |

The Rust request field is defaulted and optional for existing clients/serialized history. REST chat and WebUI topic WebSocket/desktop chat use the same value contract. In the topic path it is accepted only for user `chat` and `audio_chat` requests on the WebUI channel; other request kinds/channels that supply it are rejected. Discord/Telegram and non-user work omit it.

## Relationships

- A conversation preference belongs to one browser-local user + agent + channel + topic tuple.
- Each sent request snapshots zero or one model choice; changes after submission do not mutate it.
- A model choice references one provider and one complete model ID from the current eligible chat catalogue.
- An overridden request constructs one turn-local model. That model owns completion calls, usage recording and response model stats for that turn; the running agent's configured model remains unchanged.

## Validation and authorization

1. An absent override follows the current configured-model path and does not require `model_override`.
2. A present override requires the exact unscoped `model_override` permission (system role bypass follows the existing `access::require` behavior). Agent-view and project-channel access remain separate checks.
3. The selected provider must be a valid, eligible chat provider with a usable connection, including supported local providers.
4. The selected model must exactly match a currently eligible model for that provider's chat capability. Do not split or normalize model IDs.
5. The normal resolver must successfully resolve provider configuration/credentials. A failure at validation or execution is an error, never a default-model fallback.
6. An unauthorized or unavailable choice is not sent to model execution. Client-provided model information is never trusted as authorization or as proof of availability.

## Lifecycle

`Agent default` (`null`) → user picks a valid choice → preference is stored locally → each subsequent chat/audio-chat submission snapshots the choice into its request → server rechecks authorization and availability → per-turn model is built and used. A changed choice affects later submissions only. A request queued behind another turn or resumed after restart retains its original request field. If a saved choice goes stale, the UI marks it unavailable and requires an explicit valid choice or reset to Agent default; server-side stale submissions are rejected.
