# 016 Integrations: end-to-end results

Run on 2026-10-16 against `target/debug/vizier run` (fresh workspace, config-less mode) with two
dummyplug agents `dp-a` and `dp-b`. MCP fixture: a small offline stdio MCP server (Python,
tools `read_file`, `echo`, `sleep`) standing in for `server-filesystem`, so no network is needed.
Tools were observed with `list_tool_functions` through dummyplug's JSON path.

## §1 Upgrade without migration

Not run against a `master` deployment. Covered instead by the unit test
`records_saved_with_the_moved_integration_fields_still_load` (an old row loads and drops the moved
keys when saved) and by `POST /agents` with `discord_token` + `tools.mcp_servers` →
`422 {"error":"moved_to_integrations","fields":["discord_token","tools.mcp_servers"]}`.

## §2 MCP live attach, edit, detach (US1)

| # | Result |
|---|---|
| 1 | `201`, attached to `dp-a` |
| 2 | `dp-a` lists category `fs` (`fs_echo`, `fs_read_file`) on the next turn |
| 3 | `dp-b` has no `fs` |
| 4 | after attaching, `dp-b` lists `fs_*` |
| 5 | a 12 s turn (`fs_sleep` from `execute_python`) with `PUT /integrations/{fs}` 4 s in **completed `ok: true` on the old server (`v1`)**; health: both agents `running`, `failures 0`, no restart in the log (G1, G2) |
| 6 | the next turn's `fs_read_file` answered from the edited server (`v2`) |
| 7 | detached from `dp-b`: no `fs` there; `dp-a` unaffected |
| 8 | broken command: turns succeed without `fs` tools; the list shows `unreachable` with `connect failed: No such file or directory` |
| 9 | `context` twice: identical. The system prompt itself is not observable through dummyplug; G4 is covered by the unit test `identical_inputs_compose_identical_categories` |

## §3 Addons (US3)

| # | Result |
|---|---|
| 1 | one kind `brave_search` with `api_key` (secret, required) and `safesearch` (bool) |
| 2 | attaching a key-less configuration: `409 {"error":"incomplete","missing":["api_key"]}` |
| 3 | `201`, settings come back as `{"api_key":"••••3456",…}` |
| 4 | `dp-a`: `brave_search` → `brave_search_news`, `brave_search_web`; `dp-b`: no such category. (`web` is absent on `dp-a` only because its fetch/HTTP tools are off.) |
| 4a | second MCP `fs` onto `dp-a`: `409 name_in_use_on_agent`, naming the attached one |
| 4b | second Brave configuration installed on `dp-a` replaced the first; tool names unchanged |
| 5 | (network) a call with an invalid key fails for that call only; status becomes `failed: Brave Search rejected the API key (422 …)`. Brave answers a bad key with `422 SUBSCRIPTION_TOKEN_INVALID`, not 401/403, so that is recognised too |
| 6 | `PUT` with only `{settings:{safesearch:false}}` kept the key (checked in the database) |

## §4 Channels (US2), offline variant

| # | Result |
|---|---|
| 1 | Telegram with a dummy token → `failed: telegram rejected the bot token`; `dp-a` stays `running` and answers in the API |
| 2 | replacing the token restarts only that reader; still `failed` |
| 4 | moving to `dp-b`: `affected_agents ["dp-a","dp-b"]`; health shows the reader under `dp-b` |
| 5 | `PUT /agents/dp-b`: reader start count unchanged (3 → 3), no reconnect (G6) |
| 6 | same token on another connection: `409 token_in_use`; second Telegram on the same agent: `409 platform_in_use_on_agent` |
| 7 | `dp-b` lists the three `telegram_*` functions; `dp-a` none |

Steps needing a real bot token (running state, answering as the bot) were not run.

## §5 Permissions

Users `alice` and `bob` with `owned_agents:view/edit`, `all_agents:create`,
`owned_agents:integrations:manage`; `alice` owns `dp-al`, `bob` owns `dp-c`.

| As | Action | Result |
|---|---|---|
| alice | `GET /agents/dp-a/integrations` | `404` |
| alice | `POST` MCP `mine` on `dp-al` | `201`, `scope: owned`, `can_edit: true` |
| admin | list on `dp-a` | sees `mine` (`attached: false`) |
| bob | list on `dp-c` | does not see `mine` |
| alice | attach shared `fs` to `dp-al` | `200` |
| alice | `PUT`/`DELETE` shared `fs` | `403` |
| alice | attach to `dp-a` | `404` (not `403` as the quickstart says: the contract answers `404` for an agent the caller can't view) |
| any | `PUT /agents/…` with `discord_token` | `422 moved_to_integrations` |

## §6 WebUI (Chrome)

- Sidebar: Home … Dreams, then **Integrations** (Channels, Addons, MCP), then **Misc** (Usage, Config); headings collapse to dividers.
- MCP: table with status (Unreachable + error), also-used-by and switches; the edit panel shows the shared-edit warning.
- Addons: Brave tile with its embedded icon and *Installed*; the panel shows the configuration picker, redacted key with Replace, and "also installed on".
- Channels: the failed Telegram card with its error, Replace token, Disable, Move to agent…, and a Connect card offering only Discord.
- `/dp-a/settings` redirects to `/dp-a/config`, titled "Config"; no token, Brave or MCP sections remain.
- `npm run typecheck` passes.

## Regression (014)

`tools` lists exactly the four code-mode tools (`describe_tool_function`, `execute_python`,
`list_tool_functions`, `think`); scripts call native, addon and MCP functions as before.
