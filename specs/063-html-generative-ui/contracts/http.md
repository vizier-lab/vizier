# Contract: HTTP routes

These live under `/api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic}`, the same prefix as history and detached scripts. They are authorized in `service::ui_pages` with the rule that already guards reading that conversation's history (`can_view_agent` plus the conversation's owner). A page outside the conversation answers `404`.

| Method | Path | Response |
|---|---|---|
| GET | `/pages/{page_id}` | `{page_id, title, latest_version, state, state_seq, versions: [{version, size, origin, base_version, created_at, verification}]}` |
| GET | `/pages/{page_id}/versions/{version}` | `text/html; charset=utf-8`, the version's HTML, `Cache-Control: private, max-age=31536000, immutable` |

- There is no create, edit or delete route. Pages are made by the agent and deleted with their conversation or agent (spec Assumptions).
- State is written over the socket (`contracts/verify-protocol.md`).
- Each route gets a vector in `tests/conformance/groups/` and an OpenAPI entry.

## Static kit assets (`vizier-http` WebUI handler and the desktop asset path)

| Path | Notes |
|---|---|
| `/ui-kit/kit-{hash}.css`, `/ui-kit/kit-{hash}.js`, `/ui-kit/lib/*.js`, `/ui-kit/manifest.json` | public, immutable, `Access-Control-Allow-Origin: *` (opaque-origin frames load modules) |
