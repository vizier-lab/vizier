# Quickstart: Office Addon

**Feature**: `032-office-addon`

This checks the feature against a running binary on the offline `dummyplug` provider (spec `008-dummyplug-provider`). The dummyplug protocol is `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`: send `tools`, then a tool name for a sample request, then the JSON request to run it. Agent tools are called from `execute_python` (code mode). Everything here is offline; no step needs a live provider.

Use a scratch workspace:

```sh
rm -rf /tmp/vizier-office
cargo run -- run --workspace /tmp/vizier-office --port 9999 2>&1 | tee /tmp/vizier-office.log
```

In another shell (create the admin on first run, then log in):

```sh
curl -s localhost:9999/api/v1/auth/setup -H 'content-type: application/json' \
  -d '{"username":"admin","password":"admin"}'
TOKEN=$(curl -s localhost:9999/api/v1/auth/login -H 'content-type: application/json' \
  -d '{"username":"admin","password":"admin"}' | jq -r .data.token)
API=localhost:9999/api/v1
auth=(-H "authorization: bearer $TOKEN" -H 'content-type: application/json')
```

## 0. Upgrade path first (US3, FR-016 — run BEFORE the new agent exists)

To prove the one-time backfill, create an agent on the **old** binary (or insert one with the addon-less state): start `master`, create `dp-old` (below), stop it, then start this branch against the same workspace.

```sh
curl -s "${auth[@]}" "$API/agents" -d '{"agent_id":"dp-old","name":"Dp Old","description":"pre-existing","provider":"dummyplug","model":"dummyplug"}' | jq .data.agent_id
```

After starting the new binary once:

```sh
curl -s "${auth[@]}" "$API/agents/dp-old/integrations" | jq '[.data[] | select(.kind=="office") | .name]'   # ["Office"]
curl -s "${auth[@]}" "$API/agents/dp-old/integrations/events" | jq '[.data[] | select(.action=="created")] | length'  # 1
```

Expected: `dp-old` has exactly one `office` integration and one `created` event; the log shows the backfill ran once and `office.default_seeded` is set.

## 1. New agent has the tools without a restart (US3, FR-015)

```sh
curl -s "${auth[@]}" "$API/agents" -d '{"agent_id":"dp-a","name":"Dp A","description":"office quickstart","provider":"dummyplug","model":"dummyplug"}' | jq .data.agent_id
curl -s "${auth[@]}" "$API/agents/dp-a/integrations" | jq '[.data[] | select(.kind=="office")] | length'   # 1
```

Open a conversation with `dp-a` (WebUI, or the HTTP/WS chat) and send:

```text
tools
```

Expected: the reply lists the sandbox tools only (code mode); then send the JSON below to list the category:

```json
{"tool":"list_tool_functions","arguments":{"category":"office"}}
```

Expected: `office_docx_read`, `office_docx_write`, `office_xlsx_read`, `office_xlsx_write`.

## 2. Write a document, deliver it (US2, FR-003/007)

```json
{"tool":"execute_python","arguments":{"code":"r = office_docx_write(filename='report', title='Q4 Report', markdown='# Summary\n\nSome **bold** and *italic* text.\n\n- one\n  - nested\n- two\n\n| Item | Qty |\n|---|---|\n| Pens | 12 |\n\nSee [docs](https://example.com).')\nprint(r)"}}
```

Expected: result contains `status: ok`, `filename: "report.docx"`, a non-zero `size`. The conversation's files list (`GET $API/agents/dp-a/channel/<channel>/topic/<topic>/files`, or the WebUI files panel) shows `report.docx` with media type `application/vnd.openxmlformats-officedocument.wordprocessingml.document`. Then:

```json
{"tool":"send_attachment","arguments":{"filename":"report.docx"}}
```

Expected: the attachment is delivered (the reply carries it). Download it from the file URL and open it in Word/LibreOffice: heading, bold/italic, nested list, table and link are present, no repair prompt (SC-004).

## 3. Read it back with full fidelity (US1, SC-001/002)

```json
{"tool":"execute_python","arguments":{"code":"r = office_docx_read(filename='report.docx')\nprint(r['markdown'])\nprint(r['truncated'])"}}
```

Expected: Markdown containing `# Summary`, `**bold**`, `*italic*`, the nested list, the pipe table with `| Pens | 12 |`, and `[docs](https://example.com)`; `truncated` is `False`; the text `[table content]` appears nowhere.

## 4. Workbook write and typed read (US2/US1, FR-004/005)

```json
{"tool":"execute_python","arguments":{"code":"r = office_xlsx_write(filename='budget', sheets=[{'name':'Budget','rows':[['Item','Qty','Due'],['Pens',12,{'date':'2026-11-01'}],['Done',True,None]]},{'name':'Notes','rows':[['=1+1','+x','@y']]}])\nprint(r)\nprint(office_xlsx_read(filename='budget.xlsx'))"}}
```

Expected: write returns `status ok`, `filename: "budget.xlsx"`. The read returns both sheets; `12` is a number, `True` is a boolean, the due cell is `{"date": "2026-11-01"}`, the empty cell is `null`; the three strings on `Notes` come back as the literal text `=1+1`, `+x`, `@y` (FR-028 — never formulas). Range and sheet filters:

```json
{"tool":"execute_python","arguments":{"code":"print(office_xlsx_read(filename='budget.xlsx', sheet='Budget', range='A1:B2'))"}}
```

Expected: only that range of the `Budget` sheet, with `start: "A1"`. Single cell and sheet information:

```json
{"tool":"execute_python","arguments":{"code":"print(office_xlsx_read(filename='budget.xlsx', sheet='Budget', range='B2'))\nprint(office_xlsx_read(filename='budget.xlsx', info=True))"}}
```

Expected: the first result is one row with one cell, `12`, and `start: "B2"`. The second lists `Budget` and `Notes` with `used_range` and `dimensions` (`A1:C3`, 3 rows by 3 columns for `Budget`) and no `rows`.

## 5. Generic reader no longer reads Word and Excel (US5, FR-012/013)

Delete the addon from `dp-a` (this is also US4's removal step):

```sh
OID=$(curl -s "${auth[@]}" "$API/agents/dp-a/integrations" | jq -r '.data[] | select(.kind=="office") | .id')
curl -s "${auth[@]}" -X DELETE "$API/agents/dp-a/integrations/$OID" | jq .
```

Then, in the conversation:

```json
{"tool":"execute_python","arguments":{"code":"print(read_document_file(filename='report.docx'))"}}
```

Expected: no document text. The result is the message saying Word and Excel files are read with `office_docx_read` / `office_xlsx_read` and that the Office addon must be installed. The `office` category no longer lists (`list_tool_functions` with category `office` reports it unknown). Repeat with a legacy `.xls` fixture (`tests/fixtures/office/legacy.xls`): same message. Then `read_document_file` a plain text file and a PDF in the same conversation: both read exactly as before this feature.

## 6a. Turning the addon off and on (US4, FR-029)

First reinstall the addon on `dp-a` (it was removed in step 5): `POST $API/agents/dp-a/integrations` with `{"kind":"office","name":"Office","settings":{}}`, and keep its id as `OID`. Then use the addon's switch on `dp-a`'s Addons page, or the same call it makes:

```sh
curl -s "${auth[@]}" -X PUT "$API/integrations/$OID" -d '{"enabled":false}' | jq .data.enabled   # false
```

Then in the conversation: `list_tool_functions` with category `office` reports it unknown, and `read_document_file` on `report.docx` returns the message saying the Office addon must be installed or turned on. Restart vizier and update the agent: `GET $API/agents/dp-a/integrations` still shows one `office` entry with `enabled: false`, and the log shows no backfill. Turn it back on:

```sh
curl -s "${auth[@]}" -X PUT "$API/integrations/$OID" -d '{"enabled":true}' | jq .data.enabled   # true
```

Expected: the four `office_*` functions are listed again, with no agent restart and no running turn aborted.

## 6b. Removal sticks (US4, FR-017)

With the addon removed from `dp-a` in step 5:

1. Restart vizier (`cargo run -- shutdown --config …` then run again). Check `GET $API/agents/dp-a/integrations`: no `office` entry.
2. Update the agent (`PUT $API/agents` with the same config, e.g. change `description`). Check again: still no `office`.
3. Restart once more; still none. The log shows the backfill did **not** run (flag present).
4. Reinstall by hand (`POST $API/agents/dp-a/integrations` with `{"kind":"office","name":"Office","settings":{}}`) → 201; a second identical POST → `409 addon_installed`.

## 7. Existing-name behaviour (FR-027)

Upload `mine.docx` to the conversation as a person (WebUI attach). Then:

```json
{"tool":"execute_python","arguments":{"code":"print(office_docx_write(filename='mine.docx', markdown='# overwrite?'))"}}
```

Expected: `status: ok`, and `mine.docx` now holds the agent's content (the upload is replaced, as `image_generate` would do). Write `report.docx` again with different markdown: also `status: ok` and the file holds the new content. Then check the tool descriptions (`describe_tool_function` for `office_docx_write` and `office_xlsx_write`): both say a same-named file is replaced and tell the agent to pick a new name for a version of an existing file.

## 8. Failures are data, limits are signalled (FR-009/010)

```json
{"tool":"execute_python","arguments":{"code":"print(office_docx_read(filename='nope.docx'))\nprint(office_xlsx_read(filename='budget.xlsx', sheet='Missing'))\nprint(office_xlsx_read(filename='report.docx'))"}}
```

Expected: three `status: error` results (missing file; unknown sheet listing `Budget`, `Notes`; wrong format naming `office_docx_read`), and the turn continues. To see truncation, write a sheet of 2 000 rows and read it: `truncated: true`, `dimensions.rows == 2000`, 1 000 rows returned.

## 9. Automated checks to run alongside

```sh
cargo test office            # per-format round-trips, fixtures, kind, seed/backfill, write-path rule
cargo test every_addon_tool  # namespacing + category tables
cargo clippy
cross build --target x86_64-unknown-linux-musl   # a static target from Cross.toml still builds (Principle IV)
```
