# Contract: HTTP routes

These live under `/api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic}`, the same prefix as history and detached scripts. They are authorized in `service::ui_pages` with the rule that already guards reading that conversation's history (`can_view_agent` plus the conversation's owner). A page outside the conversation answers `404`.

| Method | Path | Response |
|---|---|---|
| GET | `/pages/{page_id}` | `{page_id, title, latest_version, state, state_seq, versions: [{version, size, origin, base_version, created_at, verification}]}` |
| GET | `/pages/{page_id}/versions/{version}` | `text/plain; charset=utf-8` (the HTML source; it never runs as a document of the WebUI origin), `nosniff`, the frame CSP, `Cache-Control: private, max-age=31536000, immutable` |
| GET | `/pages/{page_id}/versions/{version}/frame` | `{path: "/ui-frames/{ticket}", expires_at}`: a ten-minute, credential-free ticket for that version |

- There is no create, edit or delete route. Pages are made by the agent and deleted with their conversation or agent (spec Assumptions).
- State is written over the socket (`contracts/verify-protocol.md`).
- Each route gets a vector in `tests/conformance/groups/` and an OpenAPI entry.

## The frame route (public)

| Method | Path | Response |
|---|---|---|
| GET | `/api/v1/ui-frames/{ticket}` | The frame document (`text/html`), built by `vizier-http/src/ui_frame.rs`. It carries the `FRAME_CSP` header (`sandbox allow-scripts`, `connect-src 'none'`, `https` media) plus `no-store` and `no-referrer`. A forged, expired or gone ticket gets `404`. |

- **No auth middleware.** The ticket is the proof, and it is minted only after the read check above. It carries no credential of the person, because the page's script can read its own URL.
- **Nothing is fetched by the frame.** The kit's stylesheet, runtime, elements (only when the page has a `<vz-` tag) and libraries (only when the page imports them) are inlined from the embedded WebUI build, read through `ui-kit/manifest.json`.
- **Desktop.** The app loads it through the `vizier-asset` scheme, which forwards `content-security-policy` and `cache-control`; the app's `frame-src` allows that scheme.

## Static kit files

`/ui-kit/*` in the WebUI build is cached for good (`manifest.json` excepted) and sent with `Access-Control-Allow-Origin: *`. The frame no longer loads them by URL; the header is kept so a downloaded standalone page or a later bridge may.
