# Quickstart: Office Addon

**Feature**: `036-office-addon`

This checks the feature against a running binary and the standalone `dummyplug` server (spec `030-standalone-dummyplug-server`, merged in vizier-lab/vizier#103). dummyplug is no longer built into Vizier: it is a separate Node.js OpenAI-compatible server in the `dummyplug/` submodule, and an agent with provider `dummyplug` reaches it over HTTP at `DUMMYPLUG_URL` (default `http://127.0.0.1:9998/v1`). The chat protocol is unchanged: send `tools`, then a tool name for a sample request, then the JSON request to run it (see `specs/030-standalone-dummyplug-server/contracts/` and the legacy `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`). Agent tools are called from `execute_python` (code mode). No step needs a live model provider or a model key. This branch already contains #103 (merged from `master`); the `dummyplug/` submodule must be checked out (`just install`).

Start dummyplug first, in its own terminal (needs Node.js 22; `just install` pulls the submodule and its packages):

```sh
just install
just dummyplug
curl -s http://127.0.0.1:9998/healthz
```

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

Expected: `dp-old` has exactly one `office` integration and one `created` event; the log shows the backfill ran once and `migration.office_default_seeded` is set.

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

Expected: `office_docx_read_as_md`, `office_docx_write_from_md`, `office_xlsx_read_as_cells`, `office_xlsx_read_as_csv`, `office_xlsx_write_from_sheets`.

## 2. Write a document, deliver it (US2, FR-003/007)

```json
{"tool":"execute_python","arguments":{"code":"r = office_docx_write_from_md(filename='report', title='Q4 Report', markdown='# Summary\n\nSome **bold** and *italic* text.\n\n- one\n  - nested\n- two\n\n| Item | Qty |\n|---|---|\n| Pens | 12 |\n\nSee [docs](https://example.com).')\nprint(r)"}}
```

Expected: result contains `status: ok`, `filename: "report.docx"`, a non-zero `size`. The conversation's files list (`GET $API/agents/dp-a/channel/<channel>/topic/<topic>/files`, or the WebUI files panel) shows `report.docx` with media type `application/vnd.openxmlformats-officedocument.wordprocessingml.document`. Then:

```json
{"tool":"send_attachment","arguments":{"filename":"report.docx"}}
```

Expected: the attachment is delivered (the reply carries it). Download it from the file URL and open it in Word/LibreOffice: heading, bold/italic, nested list, table and link are present, no repair prompt (SC-004).

## 3. Read it back with full fidelity (US1, SC-001/002)

```json
{"tool":"execute_python","arguments":{"code":"r = office_docx_read_as_md(filename='report.docx')\nprint(r['markdown'])\nprint(r['truncated'])"}}
```

Expected: Markdown containing `# Summary`, `**bold**`, `*italic*`, the nested list, the pipe table with `| Pens | 12 |`, and `[docs](https://example.com)`; `truncated` is `False`; the text `[table content]` appears nowhere.

## 4. Workbook write and typed read (US2/US1, FR-004/005)

```json
{"tool":"execute_python","arguments":{"code":"r = office_xlsx_write_from_sheets(filename='budget', sheets=[{'name':'Budget','rows':[['Item','Qty','Due'],['Pens',12,{'date':'2026-11-01'}],['Done',True,None]]},{'name':'Notes','rows':[['=1+1','+x','@y']]}])\nprint(r)\nprint(office_xlsx_read_as_cells(filename='budget.xlsx'))"}}
```

Expected: write returns `status ok`, `filename: "budget.xlsx"`. The read returns both sheets; `12` is a number, `True` is a boolean, the due cell is `{"date": "2026-11-01"}`, the empty cell is `null`; the three strings on `Notes` come back as the literal text `=1+1`, `+x`, `@y` (FR-028 — never formulas). Range and sheet filters:

```json
{"tool":"execute_python","arguments":{"code":"print(office_xlsx_read_as_cells(filename='budget.xlsx', sheet='Budget', range='A1:B2'))"}}
```

Expected: only that range of the `Budget` sheet, with `start: "A1"`. Single cell and sheet information:

```json
{"tool":"execute_python","arguments":{"code":"print(office_xlsx_read_as_cells(filename='budget.xlsx', sheet='Budget', range='B2'))\nprint(office_xlsx_read_as_cells(filename='budget.xlsx', info=True))"}}
```

Expected: the first result is one row with one cell, `12`, and `start: "B2"`. Formula cells need a fixture, because the write tool never writes formulas (FR-028): attach `formulas.xlsx` (one formula with a saved result, one with none) and read it twice:

```json
{"tool":"execute_python","arguments":{"code":"print(office_xlsx_read_as_cells(filename='formulas.xlsx'))\nprint(office_xlsx_read_as_cells(filename='formulas.xlsx', formulas=True))"}}
```

Expected: the first shows the saved result for the first formula and `null` for the second, and a `note` naming the second cell. The second shows `{"value": <saved result>, "formula": "=…"}` for the first and `{"value": null, "formula": "=…"}` for the second. This tool never calculates: the saved result is whatever the file held, even if it no longer matches the formula. The CSV tool previews the whole workbook, and is the one that can calculate (IronCalc, research R10):

```json
{"tool":"execute_python","arguments":{"code":"print(office_xlsx_read_as_csv(filename='formulas.xlsx'))\nprint(office_xlsx_read_as_csv(filename='formulas.xlsx', executed_formula=True))\nprint(office_xlsx_read_as_csv(filename='budget.xlsx'))"}}
```

Expected: the first lists every sheet as CSV text with `used_range`, `start` and `formula_cells`, the formula cells written as `=…` text. The second writes those cells as calculated results: the `Data` sheet is `Item,Amount` / `a,17` / `b,25` / `sum,42` / `label,big` / `double,34` / `broken,#DIV/0!` (`double` had no saved result in the file, so the first read-as-cells showed `null` for it; here it is `34`) and `Summary` is `Total from Data,42`, with no `note` because every formula could be calculated. A `note` appears when a cell cannot be calculated (an unknown function, a circular reference, a link to another file): it names the cell and the cell shows its saved value. With `formulas-minimal.xlsx` (the same sheets without `styles.xml`, which the engine refuses) the CSV is the saved values (`double` is empty) and the `note` says the formulas were not calculated because the engine could not read the file. A legacy `.xls` is never calculated and its `note` says so. The third (`budget.xlsx`, no formulas) returns both sheets, `Item,Qty,Due` on the first line of `Budget`, the date as `2026-11-01`, and an empty `formula_cells`. Back to the `budget.xlsx` read: the information-only result lists `Budget` and `Notes` with `used_range` and `dimensions` (`A1:C3`, 3 rows by 3 columns for `Budget`) and no `rows`.

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

Expected: no document text. The result is the message saying Word and Excel files are read with `office_docx_read_as_md` / `office_xlsx_read_as_cells` and that the Office addon must be installed. The `office` category no longer lists (`list_tool_functions` with category `office` reports it unknown). Repeat with a legacy `.xls` fixture (`src/agents/tools/office/fixtures/legacy.xls`): same message. Then `read_document_file` a plain text file and a PDF in the same conversation: both read exactly as before this feature.

## 6a. Turning the addon off and on (US4, FR-029)

First reinstall the addon on `dp-a` (it was removed in step 5): `POST $API/agents/dp-a/integrations` with `{"kind":"office","name":"Office","settings":{}}`, and keep its id as `OID`. Then use the addon's switch on `dp-a`'s Addons page, or the same call it makes:

```sh
curl -s "${auth[@]}" -X PUT "$API/integrations/$OID" -d '{"enabled":false}' | jq .data.enabled   # false
```

Then in the conversation: `list_tool_functions` with category `office` reports it unknown, and `read_document_file` on `report.docx` returns the message saying the Office addon must be installed or turned on. Restart vizier and update the agent: `GET $API/agents/dp-a/integrations` still shows one `office` entry with `enabled: false`, and the log shows no backfill. Turn it back on:

```sh
curl -s "${auth[@]}" -X PUT "$API/integrations/$OID" -d '{"enabled":true}' | jq .data.enabled   # true
```

Expected: the five `office_*` functions are listed again, with no agent restart and no running turn aborted.

## 6b. Removal sticks (US4, FR-017)

With the addon removed from `dp-a` in step 5:

1. Restart vizier (`cargo run -- shutdown --config …` then run again). Check `GET $API/agents/dp-a/integrations`: no `office` entry.
2. Update the agent (`PUT $API/agents` with the same config, e.g. change `description`). Check again: still no `office`.
3. Restart once more; still none. The log shows the backfill did **not** run (flag present).
4. Reinstall by hand (`POST $API/agents/dp-a/integrations` with `{"kind":"office","name":"Office","settings":{}}`) → 201; a second identical POST → `409 addon_installed`.

## 7. Existing-name behaviour (FR-027)

Upload `mine.docx` to the conversation as a person (WebUI attach). Then:

```json
{"tool":"execute_python","arguments":{"code":"print(office_docx_write_from_md(filename='mine.docx', markdown='# overwrite?'))"}}
```

Expected: `status: ok`, and `mine.docx` now holds the agent's content (the upload is replaced, as `image_generate` would do). Write `report.docx` again with different markdown: also `status: ok` and the file holds the new content. Then check the tool descriptions (`describe_tool_function` for `office_docx_write_from_md` and `office_xlsx_write_from_sheets`): both say a same-named file is replaced and tell the agent to pick a new name for a version of an existing file.

## 8. Failures are data, limits are signalled (FR-009/010)

```json
{"tool":"execute_python","arguments":{"code":"print(office_docx_read_as_md(filename='nope.docx'))\nprint(office_xlsx_read_as_cells(filename='budget.xlsx', sheet='Missing'))\nprint(office_xlsx_read_as_cells(filename='report.docx'))"}}
```

Expected: three `status: error` results (missing file; unknown sheet listing `Budget`, `Notes`; wrong format naming `office_docx_read_as_md`), and the turn continues. To see truncation, write a sheet of 2 000 rows and read it: `truncated: true`, `dimensions.rows == 2000`, 1 000 rows returned.

## 9. Automated checks to run alongside

```sh
cargo test office            # per-format round-trips, fixtures, kind, seed/backfill, write-path rule
cargo test every_addon_tool  # namespacing + category tables
cargo clippy
cross build --target x86_64-unknown-linux-musl   # a static target from Cross.toml still builds (Principle IV)
```
