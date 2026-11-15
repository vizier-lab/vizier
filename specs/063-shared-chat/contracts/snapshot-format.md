# Contract: Snapshot format v1 and the exported file

Produced by `webui/app/lib/share/serialize.ts`; validated by `share/schema.ts` (zod, strict, the single definition: the viewer, the preview and the tests share it). Treat as hostile input on every read.

## Document

```jsonc
{
  "format_version": 1,
  "generator": "vizier-webui <version>",         // informational
  "created_at": 1790000000000,
  "title": "string ≤120",
  "source": { "agent_name": "string", "agent_avatar": "media-key?" },   // no user names, no ids
  "include_trail": false,
  "turns": [                                       // chronological
    {
      "id": "t1",
      "role": "person | agent",
      "at": 1790000000000,
      "parts": [
        { "kind": "text", "markdown": "…" },
        { "kind": "media", "key": "m1", "alt": "…" },                       // key into `media`
        { "kind": "ui", "title": "…", "spec": { /* json-render spec */ }, "state": { /* initial */ } },
        { "kind": "question", "title": "…", "items": [ /* read-only */ ] },
        { "kind": "placeholder", "reason": "oversized | missing | unsupported", "label": "…" }
      ],
      "trail": [                                  // present only when include_trail
        { "kind": "tool_call", "name": "…", "arguments": { }, "result": "…", "ok": true }
      ]
    }
  ],
  "media": { "m1": { "mime": "image/webp", "bytes": 12345, "data": "<base64>" } },
  "rehydration": null                             // reserved for the later import spec; MUST be null/absent in v1
}
```

Rules: ≤ 5,000 turns; ≤ 64 parts per turn; text ≤ 256 KiB per part; `ui.spec` ≤ `LIMITS.specBytes`; `media` ≤ 100 keys, MIME in the raster allow-list; every `media` key referenced exists and vice versa (unreferenced keys dropped on read); `spec` file references are rewritten at serialization from file *names* to `media` keys. Unknown top-level keys and unknown part kinds are dropped/placeholdered, never rendered. A `format_version` above the viewer's is "needs a newer version".

## The exported file `*.vizier.html`

```html
<!doctype html>
<html lang="en"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="robots" content="noindex">
<meta name="referrer" content="no-referrer">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline' 'unsafe-eval'; style-src 'unsafe-inline'; img-src data: blob:; worker-src blob:; connect-src 'none'; form-action 'none'; base-uri 'none'">   <!-- research R5.6, fixed by spike T004 (spike-notes.md) -->
<title>{escaped title}</title>
<style>/* inlined */</style>
</head><body>
<div id="root"></div>
<script type="application/json" id="vizier-snapshot">{escaped JSON}</script>
<script>/* inlined viewer bundle */</script>
</body></html>
```

- JSON escaping: `<`→`\u003c`, `>`→`\u003e`, `&`→`\u0026`, U+2028/9 → `\u2028`/`\u2029` (R6). Title in `<title>` is HTML-escaped.
- File name: `<slug of title, ≤60 chars, [a-z0-9-]>-<yyyymmdd>.vizier.html`; empty slug → `conversation`.
- No `<link>`, `<img src=http…>`, `@import`, `url(http…)`, fonts or CDN anywhere (checked by a test that greps the output).
- The viewer in the file and the viewer behind `/share/:id` are the same component.

## Importing a `.json` snapshot (later spec)

A bare snapshot document is valid input for the future import feature; it is not accepted anywhere in this version.
