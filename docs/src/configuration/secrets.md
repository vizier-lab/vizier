# Agent Secrets

An agent can keep credentials — API tokens, webhook secrets, passwords — in its own **secret store**. It reads them from its Python scripts and never prints them into the conversation. People manage the store in the WebUI or over the HTTP API; values are masked everywhere after saving.

Secrets belong to exactly one agent. The same name on two agents is two independent values, and one agent cannot read another's, not even to learn that the name exists.

## How agents use them

An agent can save a secret (for example after you tell it a token) and read it back inside the scripts it runs, to call an API on your behalf. The value is returned to the script only and is hidden from the model: it shows as `[redacted]` in the conversation. Secrets are available in ordinary, scheduled and background turns, never in the dream cycle, because a dream's transcript is summarised into memory and reflection never needs a credential. An invalid name or value is refused with a message and nothing is written.

## What the model can and cannot see

A secret is handed to the script only. The result the model sees is redacted:

- the script's `stdout`, `result` and traceback are scrubbed;
- the arguments of the calls that read or set a secret are scrubbed in the execution report;
- the redaction is exact-string replacement with `[redacted]`, applied to the whole text.

Once a value has been retrieved during a turn, any later occurrence of it in that turn's tool output is redacted too. Values shorter than eight bytes are not armed for redaction (a short string would corrupt ordinary prose), which is why the minimum stored value is eight bytes.

**The set input is not covered.** The script that sets a secret contains the value, and the agent's request, session history and debug log contain that script. Setting a secret from an agent is therefore a deliberate act with the plaintext in the transcript; setting it from the WebUI or the API is the way to keep it out of agent history entirely. Values set by a person never enter a request, and a later retrieval still only shows `[redacted]`.

Redaction is a display and transcript guarantee, not a sandbox: the script itself holds the real value and can send it anywhere the agent can reach.

## WebUI

**Secrets** is in the agent menu (the agent card at the top of its column), under **Running it**, for people who may manage the selected agent's secrets. The page is a searchable, paginated table of each name with its server-computed mask and when and by whom it last changed, with **Recent changes** beside it (the newest 20 create/replace/delete events, with actor and time but never a value or hash).

- **Add secret** asks for a name and a value. The value field is a password field and is never shown again.
- **Replace** writes a new value to the same name; the old one is not fetched or displayed.
- **Delete** asks for confirmation and names the secret. The agent's next retrieval returns `not_found`.

There is no reveal, export, share or expiry action. A `403` renders a permission notice rather than an empty page; hiding the nav item is not the protection — every route checks the permission.

## Limits

| Item | Rule |
|------|------|
| Name | 1–128 bytes, ASCII letters, digits, `.`, `_`, `-`; case-sensitive |
| Value | 8–65,536 UTF-8 bytes, stored exactly as supplied (no trimming or normalisation) |
| Change log | newest first; 20 by default, at most 100 per request |

## HTTP API

All routes require a bearer token. A **secret manager** is a system role, a holder of `all_agents:secret`, or a holder of `owned_agents:secret` who owns the agent. There is no read-only secret permission.

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/agents/{id}/secrets` | Names, masks and last change; `?q=`, `?sort=name\|updated`, and `?limit=` for a page with `total` and `counts` |
| `PUT` | `/api/v1/agents/{id}/secrets/{name}` | Create (`201`) or replace (`200`); body is `{ "value": "…" }` |
| `DELETE` | `/api/v1/agents/{id}/secrets/{name}` | Delete; `404` when the name does not exist |
| `GET` | `/api/v1/agents/{id}/secrets/events?limit=20` | Change log |

No response contains a value. `GET` and `PUT` return a mask produced by the same rule integrations use (`••••` and the last four characters, only when the value is longer than eight). That mask is a display convention, not a partial reveal you can reconstruct.

## Rotation, deletion and storage

Replacing a value takes effect for the very next retrieval. Nothing is cached, no agent restarts and no running turn is aborted. Deleting is immediate and permanent: the row and a `deleted` event are written in one transaction, and the agent's next lookup finds nothing. Setting the name again creates a fresh secret (`created`), with a new `created_at`.

Every write records an event (who — a person or the agent — what, and when) in `agent_secret_event`; the event holds no value. Deleting an agent removes both its secrets and its events.

Values are stored as authenticated ciphertext in the embedded SQLite database. They are encrypted at rest with the workspace key and decrypted only in memory for their consumer; the WebUI and HTTP API still mask them after decryption. **There is no expiry and no version history.** The key is separate from the database and must be backed up: see [Encryption at Rest](./encryption.md) for key sources, backup/restore and what to do if the key is lost.
