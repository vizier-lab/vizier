# Quickstart: Agent Self-Managed Integrations

These are end-to-end checks against a running binary, per the constitution's dummyplug gate. Every agent step uses the dummyplug protocol (`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`). Agent tools are called from code mode:

```json
{"tool":"execute_python","arguments":{"intent":"…","code":"print(create_integration(kind='mcp', name='fs', settings={…}))"}}
```

The tables below abbreviate that as `create_integration {…}`.

## Build and static checks

```sh
cargo clippy && cargo test
cd webui && npm run typecheck
```

Unit tests cover:
- `access.rs`: person and agent rules, legacy rows;
- the grant storage round-trip;
- `every_tool_is_in_its_category` with the new `Integrations` category;
- `category_summaries` (no `integrations` category without a grant).

## Fixtures

- **Agents**: `dp-a` and `dp-b`, both on `dummyplug`. `alice` owns `dp-a`.
- **Users**:
  - `admin`: system role.
  - `alice`: `owned_agents:edit` and `owned_agents:integrations:manage`.
  - `carol`: `all_agents:edit` only, so she can edit agents but has no integration permission.
- **MCP server**: a local stdio MCP server that is known to work (e.g. `npx -y @modelcontextprotocol/server-everything`).
- Before each section, record `GET /agents/health` for `dp-a` (`failures` = 0, no restart).

## 1. Grants and their permission (US1, FR-001–FR-006, SC-002, SC-006)

| # | Action | Send to agent | Expect |
|---|---|---|---|
| 1 | — | `dp-a`: `list_tool_functions {}` | no `integrations` category (SC-006) |
| 2 | as `carol`: `PUT /agents/dp-a/integrations/grants {"mcp":true}` | — | `403`, `GET …/grants` still all off |
| 3 | as `carol`: `PUT /agents/dp-a` with a changed description | — | `200`, grants unchanged |
| 4 | as `carol`: `PUT /agents/dp-a` with `"self_management":{"mcp":true}` | — | `422 use_grants_endpoint` |
| 5 | as `alice` on `dp-b` (not hers): `PUT …/dp-b/integrations/grants {"mcp":true}` | — | `403` |
| 6 | as `alice`: `PUT /agents/dp-a/integrations/grants {"mcp":true}` | — | `200`, `mcp.on`, `granted_by: alice`; health shows no restart |
| 7 | — | `dp-a`: `list_tool_functions {"category":"integrations"}` | the five tools; descriptions name only MCP servers |
| 8 | Start a long turn on `dp-a` (`time.sleep(15)`) and switch `addon` on during it | — | the turn completes `ok: true` (FR-006) |

## 2. MCP self-management (US2, FR-007–FR-012, FR-017, FR-019, SC-001, SC-003, SC-004)

| # | Action | Send to agent | Expect |
|---|---|---|---|
| 1 | — | `dp-a`: `create_integration {kind:"mcp", name:"everything", settings:{transport:"local", command:"npx", args:["-y","@modelcontextprotocol/server-everything"]}}` | the result has `note: available from your next turn` |
| 2 | — | `dp-a`: `list_tool_functions {"category":"everything"}` | lists `everything_*` (next turn) |
| 3 | `GET /agents/dp-a/integrations?family=mcp` | — | the row has `creator {agent:"dp-a"}`; `dp-b` doesn't list it |
| 4 | — | `dp-a`: `create_integration` with the same name again | error `name_in_use_on_agent` |
| 5 | — | `dp-a`: `update_integration {id, enabled:false}` | next turn: no `everything` category |
| 6 | — | `dp-a`: `list_integrations {}` with a server that has an `env` secret | the secret is redacted (`••••…`) |
| 7 | — | `dp-a`: `delete_integration {id}` | gone from the page |
| 8 | `GET /agents/dp-a/integrations/events` | — | `created`, `edited`, `disabled`, `deleted`, all with actor `{agent:"dp-a"}` and a session; no secret values in `fields` |
| 9 | as `alice`: switch `mcp` off | `dp-a`: `list_integrations {"family":"mcp"}` | error `managing mcp is not enabled…`; nothing changes (SC-003) |
| 10 | — | `dp-a`: `delete_integration {id of a dp-b integration}` | `no integration '<id>'` |
| 11 | — | throughout | `GET /agents/health` shows no restart or new failure for `dp-a` or `dp-b` (SC-004) |

## 3. Addons (US3)

| # | Action | Send to agent | Expect |
|---|---|---|---|
| 1 | grant `addon` | `dp-a`: `list_integration_kinds {"family":"addon"}` | `brave_search`, `installed:false` |
| 2 | — | `dp-a`: `create_integration {kind:"brave_search", name:"brave", settings:{api_key:"test-key-123456"}}` | created; next turn `list_tool_functions {"category":"brave_search"}` lists `brave_search_web` / `_news` |
| 3 | — | the same create again | `addon_installed` with the existing id |
| 4 | — | `delete_integration {id}` | the category is gone the next turn |

## 4. Channels (US4) — needs a real Telegram bot token (live, marked)

| # | Action | Send to agent | Expect |
|---|---|---|---|
| 1 | grant `channel` | `dp-a` (via WebUI): `create_integration {kind:"telegram", name:"tg", settings:{token:"<real>"}}` | Integrations › Channels shows `running` within 30 s; the bot answers `tools` |
| 2 | — | from the Telegram chat: `update_integration {id, enabled:false}` | the result carries `warning`; the bot stops answering; the WebUI shows it disabled |

## 5. Per-agent model and legacy rows (FR-020–FR-023)

| # | Action | Expect |
|---|---|---|
| 1 | `PUT /agents/dp-a/integrations/{iid}/attachment {attached:true}` | `410 attach_removed` |
| 2 | Seed a legacy row (with SQL, insert a second `integration_attachment` for an existing MCP server onto `dp-b`) and restart | both agents keep its tools; on both pages the row shows `legacy_linked`; as `alice` it is read-only |
| 3 | `dp-a` (grant on): `list_integrations {}` | the legacy row is absent |
| 4 | as `admin`: `DELETE /agents/dp-a/integrations/{legacy}` | unlinked from `dp-a` only; an `unlinked` event; `dp-b` still has it |
| 5 | Delete agent `dp-b` | its integrations, grants and events are gone |

## 6. WebUI

1. As `carol`, open `dp-a` › Misc › Config › Tools: the Self-management checkboxes are disabled with the permission notice.
2. As `alice`: tick Addons and save. Reload, and it is still ticked. The MCP warning is always visible.
3. On Integrations › MCP, an agent-made row shows "added by 🤖 dp-a". Its Activity tab lists the events from §2.
4. On Integrations › Addons there is no configuration picker. Uninstall asks for confirmation and deletes the configuration.
