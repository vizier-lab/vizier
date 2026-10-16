# Quickstart: Integrations Section

**Feature**: `016-integrations-section` | **Date**: 2026-10-16

## Build and static checks

```sh
just install
cargo build
cargo test     # integration storage round-trip + access helper, kind validate/redact/merge_secrets,
               # 016 migration (merge / no-merge / duplicate token / idempotent), compose() reuses unchanged McpServer Arcs,
               # every_tool_is_in_its_category (unchanged table)
cargo clippy
cd webui && npm run typecheck
```

## Fixtures

- **Agents.** Run `dp-a` and `dp-b` on the `dummyplug` provider. The dummyplug protocol is in `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`.
- **MCP server.** `fs` runs `npx -y @modelcontextprotocol/server-filesystem /tmp`. It needs `node` but no network once cached. Use any local stdio MCP server if `node` isn't available.
- **Reading an agent's tools** (code mode). Send `dp-*` the message `{"tool":"list_tool_functions","arguments":{"category":"<cat>"}}`:
  - `<cat>` = `web` shows Brave;
  - `<cat>` = `fs` shows the MCP server;
  - `<cat>` = `discord` / `telegram` shows the bot tools.

  Use `{"tool":"list_tool_functions","arguments":{}}` to list every category.
- **Users.** `admin` is the system role. `alice` and `bob` have a role with `owned_agents:edit` and `owned_agents:integrations:manage`. `alice` owns `dp-b`; `bob` owns a third dummyplug agent, `dp-c`.

## 1. Upgrade migration (US4, FR-022–FR-025, SC-004)

**Setup**: on `master`, create these agents through the WebUI or API, then run `just shutdown` and check out this branch.

| Agent | Old config |
|---|---|
| `old-1` | Brave on, key `K1`, safesearch on; MCP `fs` (above); Discord token `T1` (a dummy string is fine) |
| `old-2` | Brave on, key `K1`, safesearch on; MCP `fs`, an identical definition |
| `old-3` | Brave on, key `K2`; MCP `fs` with a **different** `args` |

There must also be a role holding `agents:mcp_config` but no edit permission.

Start this branch with `just run`.

| Check | Expect |
|---|---|
| startup log | one `info` per migrated integration; a `warn` that `old-3`'s `fs` was renamed `fs-old-3` |
| `GET /api/v1/agents/old-1/integrations?family=addon` | one Brave item, attached, `also_used_by` = `old-2`; `api_key` redacted `••••…` |
| same for `old-3` | a *different* Brave item (its own key), not used by others |
| `GET …/old-1/integrations?family=mcp` | `fs`, attached, `also_used_by` = `old-2` |
| `GET …/old-3/integrations?family=mcp` | `fs-old-3`, attached |
| `GET …/old-1/integrations?family=channel` | one Discord item, attached; status `failed` (dummy token rejected) **while `old-1` is `running`** in `GET /agents/health` |
| `GET /api/v1/agents/old-1` | no `discord_token`, `tools.brave_search` or `tools.mcp_servers` |
| roles | `agents:mcp_config` gone everywhere; the role that had it now has `owned_agents:integrations:manage` |
| restart again | no migration log lines and no duplicate integrations (FR-024) |

## 2. MCP live attach, edit and detach without a restart (US1, FR-014–FR-018, SC-001–SC-003)

All steps use dummyplug on `dp-a`/`dp-b`. Before the steps, note `GET /agents/health` (`state`, and `failures` = 0) for both agents.

| # | Action | Send to agent | Expect |
|---|---|---|---|
| 1 | `POST /agents/dp-a/integrations` `{kind:"mcp", name:"fs", settings:{transport:"local", command:"npx", args:[…]}}` | — | `201`, attached to `dp-a` |
| 2 | — | `dp-a`: `list_tool_functions {}` | categories include `fs` (the next turn after step 1) |
| 3 | — | `dp-b`: `list_tool_functions {}` | no `fs` |
| 4 | `PUT /agents/dp-b/integrations/{fs}/attachment {attached:true}` | `dp-b`: `list_tool_functions {"category":"fs"}` | lists `fs_*` functions |
| 5 | Start a long turn on `dp-a`: `{"tool":"execute_python","arguments":{"intent":"wait","code":"import time\ntime.sleep(15)"}}`, and during it `PUT /integrations/{fs}` with a changed `args` | — | the long turn **completes normally** with `ok: true`; health shows no restart and no new failure (G1, G2) |
| 6 | — | `dp-a`: `list_tool_functions {"category":"fs"}` | still listed (now using the edited server) |
| 7 | `PUT /agents/dp-b/integrations/{fs}/attachment {attached:false}` | `dp-b`: `list_tool_functions {}` | no `fs`; `dp-a` unaffected |
| 8 | Edit `fs` to a command that doesn't exist | `dp-a`: `context` | the turn succeeds without `fs` tools; `GET …/dp-a/integrations?family=mcp` shows `unreachable` with an error |
| 9 | — | `dp-a`: `context`, twice in a row with no integration change | the system prompt section is identical both times (G4) |

## 3. Addons (US3, FR-011–FR-013, FR-011a/b)

| # | Action | Expect |
|---|---|---|
| 1 | `GET /integrations/kinds?family=addon` | one item `brave_search` with `fields` |
| 2 | As admin, `POST` a Brave item with `settings:{safesearch:true}` (no key) on `dp-b`, detach it, then `PUT /agents/dp-a/integrations/{it}/attachment {attached:true}` | `409 incomplete, missing ["api_key"]` |
| 3 | `POST /agents/dp-a/integrations {kind:"brave_search", name:"brave", settings:{api_key:"dummy", safesearch:true}}` | `201` |
| 4 | `dp-a`: `list_tool_functions {"category":"web"}` | Brave web and news search functions listed; `dp-b` doesn't have them |
| 5 | `dp-a`: run the Brave web search via `execute_python` | the tool fails with an auth error *for that call only*; the item's status becomes `failed` with that error (needs network; marked) |
| 6 | `PUT /integrations/{brave}` with only `{settings:{safesearch:false}}` | `api_key` kept (redaction round-trip) |

## 4. Channels (US2, FR-007–FR-010) — needs real bot tokens (marked)

| # | Action | Expect |
|---|---|---|
| 1 | Create a Telegram connection for `dp-a` with a real token | within 30 s it shows `running`; the bot answers `tools` with the four code-mode tools (dummyplug through Telegram) |
| 2 | Replace its token with an invalid one | `failed` with Telegram's reason; `dp-a` still answers in the WebUI; health `state: running` |
| 3 | Restore the token | `running` again within 30 s (SC-002) |
| 4 | Move the connection to `dp-b` (`attachment` on `dp-b`) | the bot now answers as `dp-b`; `dp-a` unaffected |
| 5 | `PUT /agents/dp-b` (change `thinking_depth`) | the bot keeps answering throughout; no reconnect in the log (G6) |
| 6 | Create a second connection with the same token | `409 token_in_use` |
| 7 | `dp-b`: `list_tool_functions {"category":"telegram"}` | three Telegram functions; `dp-a` has none |

Offline variant of steps 2 and 6: with a dummy token, step 1 ends in `failed`, and steps 2 and 6 still hold.

## 5. Permissions (FR-005, FR-005a, FR-005b)

| As | Action | Expect |
|---|---|---|
| `alice` | `GET /agents/dp-a/integrations?family=mcp` (`dp-a` not hers, not shared to her) | `404` |
| `alice` | `POST /agents/dp-b/integrations` (MCP `mine`) | `201`, `scope: owned` |
| `admin` | `GET /agents/dp-a/integrations?family=mcp` | lists alice's `mine` (all-scope sees everything), `attached: false` |
| `bob` (owned scope, owns `dp-c`) | `GET /agents/dp-c/integrations?family=mcp` | does **not** list alice's `mine` (another owned-scope user's) |
| `alice` | attach a `shared` integration to `dp-b` | `200` |
| `alice` | `PUT /integrations/{shared one}` | `403` |
| `alice` | attach anything to `dp-a` | `403` |
| any | `PUT /agents/dp-a` with `"discord_token": "x"` | `422 moved_to_integrations` (FR-021) |

## 6. WebUI (FR-001, FR-002, FR-002a, FR-011a, FR-019, SC-006)

- **Sidebar order:** the agent menus are Home through Dreams. **Integrations** has Channels, Addons and MCP. **Misc** has Usage and Config. Everything is disabled with no agent selected.
- **Agent switcher:** switching agents on Integrations › MCP reloads the list for the new agent.
- **Addons:** a Brave tile shows *Installed* or *Get*. Install on an agent without a key opens the key form first. Uninstall affects only this agent.
- **Shared MCP edit:** shows the also-used-by warning before saving.
- **Config:** has no token, Brave or MCP sections. `/:agentId/settings` redirects to `/:agentId/config`.
- **Role editor:** shows the two integration permissions and no "Configure Agent MCP".
