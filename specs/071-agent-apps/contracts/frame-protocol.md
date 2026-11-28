# Contract: page ↔ host protocol additions

This extends spec 063's `window.vizier` runtime (`webui/app/kit/runtime.ts`) and its host (`SandboxedPage.tsx`, `lib/uiPage/host.ts`). Everything in 063 is unchanged. The new abilities appear in `vizier.abilities` **only in app frames** (boot data `app: {id, abilities}`), so a page in a reply sees exactly what it sees today.

Messages keep the `{vz: 1, type, ...}` envelope. Requests use the existing `ask()` helper: frame → host `{type, id, ...}`, host → frame `{type: "reply", id, ok, value | error}`.

## Page API

```ts
vizier.has('db')                                   // true in app frames
await vizier.db.query(sql, params?)                // → {columns, rows}
await vizier.db.exec(sql, params?)                 // → {changes}
const off = vizier.db.onChange((tables) => ...)    // after any write by page, function or driver

await vizier.fn.call(name, input)                  // → output; rejects with {status, reason}

await vizier.chat.send(text, {files?: [fileId]})   // only right after a press or key (gesture rule)

const f = await vizier.files.pick({accept?})       // opens the OS file picker; → {id, name, size, type}, uploaded to the thread
await vizier.files.list()                          // → app files + thread uploads
await vizier.files.keep(id)                        // thread upload → app file
await vizier.files.share(id)                       // app file → thread attachment

vizier.tools.handle(name, async (input) => output) // answer a manifest tool
```

## Messages

| Direction | `type` | Fields | Host action |
|---|---|---|---|
| frame → host | `db.query` / `db.exec` | `sql, params` | `POST /apps/{id}/db/query` |
| frame → host | `fn.call` | `name, input` | `POST /apps/{id}/functions/{name}/call` |
| frame → host | `chat.send` | `text, files` | `SendGate.check(now, focused)` (the 10 s interaction window and 1 s spacing, already in `host.ts`), then `POST /apps/{id}/chat`. A refusal replies `ok:false, error:{status:"no_gesture"\|"rate_limited"\|"no_driver"}` |
| frame → host | `files.pick` | `accept` | The host can't open a picker for the frame. **The frame's own `<input type=file>` stays the picker**: the page calls `vizier.files.upload(blob, name)`, the runtime sends `files.upload{name, type, bytes: ArrayBuffer}` (transferred) and the host posts it |
| frame → host | `files.list` / `files.keep` / `files.share` | `id?` | The matching route |
| frame → host | `tools.ready` | `names: [..]` | Records which manifest tools have handlers |
| host → frame | `db.changed` | `tables` | Relayed from the socket frame `app_data_changed` |
| host → frame | `tool.call` | `id, name, input` | A manifest tool call from the driver's turn |
| frame → host | `tool.result` | `id, ok, value \| error` | Returned to `ClientTools` as the call's outcome |

`vizier.files.pick()` in the API above is a helper in the runtime: it creates a hidden `<input type=file>`, clicks it from the current gesture, and then calls `upload`. The bytes cross `postMessage` as a transferred `ArrayBuffer`, because the frame has no network (`connect-src 'none'`).

## Rules

- `db.*` reject DDL with `status: "refused"`. A query over its limits rejects with `too_large` or `timeout`.
- `fn.call` for a function that is not `page_callable`, not approved or unavailable rejects with that status and no run (SC-006).
- `chat.send` outside the gesture window never reaches the server.
- A `tool.call` for a name with no handler is answered by the host at once with `{status: "unavailable"}` (Story 9, scenario 2).
- Every new message type is added to `parseFrameMessage`; unknown types are dropped, as today.
