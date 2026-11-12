# Quickstart: verifying Shared Chat Snapshots

End-to-end checks on a running binary with a `dummyplug` agent (constitution: no keys, no network). Replace `$B` with the server (`http://localhost:9999` for `just run`). `$ADMIN`, `$ALICE`, `$BOB` are bearer tokens of three people from `POST /api/v1/auth/login`; Alice owns agent `$A`. Alice's role holds `owned_agents:share_conversations`; Bob's role does not. Steps are scripted for the implementation; none has been run yet.

## 0. Setup

```sh
just run-d                                   # dev.vizier.yaml, agent "plug" on dummyplug
# make a conversation as Alice (WebUI, or the chat socket): send "tools", then a ui_render sample,
# then a plain message; note TOPIC=<topic id>.
```

## 1. Create, read, revoke (P1, US1/US5)

```sh
curl -s -X POST $B/api/v1/shares -H "Authorization: Bearer $ALICE" -H 'content-type: application/json' \
  -d @fixtures/public.json                   # → 201 {share_id, url_path}
curl -si $B/api/v1/shares/$ID               # no token → 200, Cache-Control: public, max-age=300, ETag, X-Robots-Tag: noindex
curl -si $B/api/v1/shares/$ID -H 'If-None-Match: "'$ID'"'     # → 304
curl -s -X DELETE $B/api/v1/shares/$ID -H "Authorization: Bearer $BOB"     # → 404 (not yours)
curl -s -X DELETE $B/api/v1/shares/$ID -H "Authorization: Bearer $ALICE"   # → 204
curl -si $B/api/v1/shares/$ID               # no token → 401 sign_in_required; with $ALICE → 404 share_unavailable
```

## 2. Permission and audiences (US3)

```sh
curl -s -o /dev/null -w '%{http_code}\n' -X POST $B/api/v1/shares -H "Authorization: Bearer $BOB" … # 403
# audience specific, people:[bob]; read as bob → 200 private,no-store; as carol (signed in) → 404; no token → 401
# audience signed_in: read as carol → 200; no token → 401
# audience specific with people:[] / 51 people / unknown user id → 400
```

## 3. Limits and malformed input (negative)

```sh
# 5 MiB + 1 byte body → 413 ; format_version 2 → 400 ; include_trail=false with trail parts → 422
# not JSON → 400 ; title 121 chars → 400 ; 11th create in an hour → 429 with retry_after_secs
# 61st miss from one address within a minute → 429 (FR-049)
# GET /shares/people?q=a → 400 ; q=al → at most 10 matches ; 31st search in a minute → 429 (FR-012)
# GET /api/v1/shares/short, /shares/../etc, /shares/%00, 22-char id → same 404/401 body as unknown
```

## 4. WebUI (Playwright, `webui/e2e/share.spec.ts`)

Share from the chat header → preview counts match the fixture (messages, images, removals) → Create → copy link → open in a fresh context with no storage → text, image, 3D scene (canvas present, orbit changes the frame) and tabs work; composer is absent; `sendMessage` button is inert. Export → file saved → open via `file://` with `context.setOffline(true)` → same content, zero network requests (Playwright `request` listener), zero console errors.

## 5. Hostile content (SC-007)

Fixtures in `webui/app/lib/share/fixtures/hostile/*.json` and `*.vizier.html` (script tags, `onerror`, `javascript:`, `</script><script>`, `<!--<script>`, SVG with script as `image/svg+xml` media, CSS `url(http://…)`, U+2028, giant string, 10 000 nested objects). Each opened in the viewer: `window.__pwned` stays undefined, no outbound request, a placeholder or inert text is shown.

## 6. Redaction (SC-003, SC-004)

Seed a dummyplug conversation with a tool result `sk-ant-…`, `AKIA…`, a PEM block, `/home/alice/.vizier/secret.txt`, a recalled passage. Serialize with trail off and on; the output (`JSON.stringify` of the whole document) contains none of the seeds; the preview counts match.

## 7. Existing data untouched (plan rule, T014; SC-010 for shares themselves)

Start the new binary on a copy of an existing workspace database: `shared_snapshot` tables appear, `PRAGMA integrity_check` is ok, row counts of every pre-existing table are unchanged. Start the old binary on the same file: it runs. For PostgreSQL: apply `0007` to a database at `0006`, then run the storage tests with `VIZIER_TEST_DATABASE_URL` (the variable `tests_support.rs` reads).

## 8. Load (SC-012)

`oha -z 60s -c 50 $B/api/v1/shares/$ID` against a 2 MiB public snapshot while a signed-in `GET /api/v1/agents` is timed once a second; the median of the latter stays within 10 % of its idle value.


## Results (round 3, 2026-11-12, debug binary, scratch workspace)

Run against `vizier run --port 9999` with the dummyplug server on 9998. The embedding model cannot be downloaded here; a stub (random-table ONNX `Qdrant/all-MiniLM-L6-v2-onnx` plus a word-level tokenizer, seeded in the hf-hub cache layout under `<data-dir>/.runtime/embeddings/Qdrant/all-MiniLM-L6-v2-onnx/`) lets agents be created through the API. Chat turns work; vectors are meaningless.

- §1, §2 by HTTP (`qs.py`): all pass, including no `audience` in the read body (FR-030) for the author's named person and anonymous readers; the owner's list keeps it.
- §3 by HTTP: not JSON 400, format_version 2 400, title 121 400, trail 422, 5 MiB+1 413, 10th/11th create 429 with `retry_after_secs`, miss limiter 429 before the 61st (52nd here, earlier misses counted), people q=a 400, 31st search 429, odd ids (`short`, `%00`, 22 chars, `%2e%2e%2f`) the same body as unknown. `/shares/../etc` is not a share route and gets the WebUI fallback page (200 HTML), not share data.
- Secret backstop: PEM, sk-, ghp_, AKIA, hf_, xox in a turn and a ghp_ in the title give `422 secret_detected` and the message does not contain the text; "password is stored in a vault", `sk-learn`, `max_token=2048` are accepted.
- §4 Playwright (desktop-light, `share.spec.ts`, in groups): all share.spec tests pass. `a11y.spec` Sharing cases pass in light and dark; `mobile.spec` /settings/shares passes in light and dark. `/settings/shares` (and every settings page) fails axe `color-contrast` on the group labels in the settings navigation (design system tokens; not specific to shares).
- §5: TP-40/43 fixtures that the server now refuses (dangling media key, SVG media) are asserted as refusals; TP-74 opens each hostile fixture as a file and passes. The 100-conversation set was not run.
- §6: Scenario 3 passes (seeds never reach the stored document, trail off and on).
- NOT run: §7 on a copy of a workspace database and with the old binary, PostgreSQL (none here), §8 `oha` (not installed), the SC-001 30 s timing, mobile/dark runs of the whole share.spec. T081 stays open.
