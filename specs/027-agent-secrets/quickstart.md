# Quickstart: Agent Secrets (dummyplug e2e)

**Feature**: `027-agent-secrets`

This checks the feature against a running binary with agents on the offline `dummyplug` provider, per the constitution's e2e gate. The dummyplug protocol is `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`. All agent tool calls go through `execute_python` (code mode), and dummyplug runs one tool per turn.

The value used in steps 2–5 is deliberately a test value. It is **expected** to be visible in the `execute_python` script that supplies it (the set input); the checks below are about the stored value, later retrievals, reports and logs.

## 0. Setup

1. Start vizier and capture the log:

   ```sh
   just run 2>&1 | tee /tmp/vizier-secrets.log
   ```

2. Add this tiny echo server so a script can prove it received the real value. It records the request line and `Authorization` header to `/tmp/vizier-echo.log`:

   ```sh
   cat >/tmp/vizier-echo.py <<'PY'
   from http.server import BaseHTTPRequestHandler, HTTPServer

   class Handler(BaseHTTPRequestHandler):
       def do_GET(self):
           print(f"{self.path} {self.headers.get('Authorization', '')}", flush=True)
           self.send_response(200)
           self.end_headers()
           self.wfile.write(b"ok")

       def log_message(self, *args):
           pass

   HTTPServer(("127.0.0.1", 8765), Handler).serve_forever()
   PY
   python3 -u /tmp/vizier-echo.py >/tmp/vizier-echo.log 2>&1 &
   ```

3. Create two dummyplug agents in the WebUI, `dp-a` and `dp-b`, each with **HTTP client** enabled (`tools.http_client: true` in `POST /api/v1/agents`). Open a topic with each. Agent `dp-a` is the one used below; `dp-b` is for isolation.

4. Check the category is offered. In `dp-a`, send:

   ```json
   {"tool":"list_tool_functions","arguments":{"category":"secrets"}}
   ```

   Expect exactly `set_secret` and `get_secret`. Sending `tools` still shows only the four sandbox tools (`think`, `execute_python`, `list_tool_functions`, `describe_tool_function`).

## 1. Agent sets and gets (US1, FR-001–FR-008)

In `dp-a`, send:

```json
{"tool":"execute_python","arguments":{"code":"print(set_secret(name='AGENT_TOKEN', value='agent-token-abc123'))"}}
```

Expect a result with `created` and `AGENT_TOKEN`. The nested `set_secret` arguments in the saved report show `[redacted]` for the value. The outer script contains the value; that is the excluded set input.

In a later message (a separate turn), send:

```json
{"tool":"execute_python","arguments":{"code":"result = get_secret(name='AGENT_TOKEN')\nprint(result)\nprobe = http_client(url='http://127.0.0.1:8765/probe/a', method='GET', headers={'Authorization': 'Bearer ' + result['value']})\nprint(probe['status'])"}}
```

Expect:

- the report's printed result has `\"status\": \"found\"` (or Python's `'found'`) and `\"value\": \"[redacted]\"`;
- `200`;
- `/tmp/vizier-echo.log` has `AUTH=Bearer agent-token-abc123` or `Bearer agent-token-abc123` on the `/probe/a` line — the script received the exact value;
- the saved history and the WebUI show `[redacted]`, never the token.

The echo server's log is outside vizier and is the proof of the value; it is not part of the non-disclosure surface.

Missing name:

```json
{"tool":"execute_python","arguments":{"code":"print(get_secret(name='NO_SUCH_TOKEN'))"}}
```

Expect `not_found` naming `NO_SUCH_TOKEN`; nothing is created.

## 2. Restart and persistence (US1, FR-004, SC-007)

1. `just shutdown`, then `just run` again (keep the echo server running).
2. In `dp-a`, repeat the get script from step 1.
3. Expect `200`, `[redacted]` in the report, and a new echo log line with the same token.

## 3. Replace and delete (US3, FR-011, FR-017, SC-005)

1. Open `dp-a`'s **Secrets** page, choose **Replace** for `AGENT_TOKEN`, and save `rotated-token-xyz789`. The page must show only the new masked value.
2. In the agent, repeat the get script. Expect `200` and an echo log line with `rotated-token-xyz789`, with no restart or aborted turn.
3. Replace it again from a script with `set_secret(name='AGENT_TOKEN', value='script-token-456')`; the next get must return `script-token-456`.
4. Delete `AGENT_TOKEN` from the page after the confirmation dialog, and confirm the row disappears.
5. In the agent, call get again. Expect `not_found`, no echo log line, and no turn error.
6. Set `AGENT_TOKEN` again. It is created fresh (`created`, not `replaced`), and the old values are never returned.
7. Delete a name that does not exist via `DELETE /api/v1/agents/dp-a/secrets/NO_SUCH_TOKEN`; expect `404` and no change.

## 4. WebUI and permissions (US2, FR-013–FR-018, SC-008)

1. As a `superadmin`, open `dp-a`'s Secrets page. Create `UI_TOKEN` = `ui-only-token-24680`, reload, and confirm it is listed with a masked value and the actor; the raw value is not in the page source, network response or another tab.
2. Open the role editor. The **Secrets** group must show both `all_agents:secret` ("Manage every agent's secrets") and `owned_agents:secret` ("Manage secrets of agents they own"). Grant and revoke them like the other permissions.
3. Create a user with a role holding `all_agents:secret`. They can open either agent's page.
4. Create a user with `owned_agents:secret` who owns `dp-a` but not `dp-b`. They can manage `dp-a`; `dp-b` is not offered, and a direct request for `dp-b` returns `403`.
5. Create a user with neither permission. The Secrets nav item is absent. A direct call is refused:

   ```sh
   TOKEN=$(curl -s localhost:9999/api/v1/auth/login \
     -H 'content-type: application/json' \
     -d '{"username":"no-secret","password":"…"}' | jq -r .data.token)
   curl -i localhost:9999/api/v1/agents/dp-a/secrets \
     -H "Authorization: Bearer $TOKEN"
   ```

   Expect `403` and no names. Repeat with a `PUT`; expect `403` and the stored value unchanged.
6. With a permitted page open, revoke that user's secret permission. The next request (reload or direct) must return `403`; the next write must change nothing.
7. The `superadmin` role still works for every agent.

## 5. Cross-agent isolation (US4, FR-001, FR-009, SC-004)

1. In `dp-b`, set the same name to a different value:

   ```json
   {"tool":"execute_python","arguments":{"code":"print(set_secret(name='AGENT_TOKEN', value='agent-b-token-456'))"}}
   ```

2. Get `AGENT_TOKEN` in `dp-b` and send it to `http://127.0.0.1:8765/probe/b`. The echo log must show `agent-b-token-456` on `/probe/b`.
3. Get it in `dp-a` and send it to `/probe/a`. The echo log must show `agent-token-abc123` (or the current A value) on `/probe/a`, never B's.
4. In `dp-a`, ask for a name that exists only on B (for example `B_ONLY_TOKEN`). Expect `not_found`, with no hint that B has it.

## 6. Change records (US4, FR-019, FR-021)

1. On `dp-a`'s Secrets page, open **Recent changes** (or call `GET /api/v1/agents/dp-a/secrets/events?limit=20`).
2. Expect one row each for the agent's create, the WebUI replace, the script replace and the delete, newest first, with the action, name, actor name and time.
3. No row contains the value, a mask or a hash. A change made by `dp-a` shows the agent as the actor; a WebUI change shows the person.
4. Set `limit=1` and expect one row; set `limit=500` and expect at most 100 (the cap).

## 7. Logs and telemetry (US4, FR-020, SC-003)

1. In the running `RUST_LOG=debug` server, retrieve `UI_TOKEN` (set only through the WebUI, so its value was never in an agent request):

   ```json
   {"tool":"execute_python","arguments":{"code":"print(get_secret(name='UI_TOKEN'))"}}
   ```

2. Search `/tmp/vizier-secrets.log` for `ui-only-token-24680`: expect no match. The report shows `[redacted]`.
3. Search the saved history (`sqlite3 "${VIZIER_DATA_DIR:-$HOME/.vizier}/vizier.db"` or the WebUI) for the value: expect no match outside the Set input of the WebUI form, which is not stored in history.
4. For telemetry, run with `OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true` and `just run-otel`, retrieve the same secret, and inspect the `execute_tool` span for `execute_python` in Jaeger. Its captured arguments/result must show `[redacted]`, not the value.

## 8. Deletion and restart cleanup (FR-005, SC-006)

1. Delete `dp-b` from the WebUI. Check:

   ```sh
   sqlite3 "${VIZIER_DATA_DIR:-$HOME/.vizier}/vizier.db" \
     "select count(*) from agent_secret where agent_id='dp-b';
      select count(*) from agent_secret_event where agent_id='dp-b';"
   ```

   Expect `0` and `0`.
2. Delete `dp-a` as well and check both tables are empty for it.
3. The test value may still be in `/tmp/vizier-echo.log` (the echo server's own file); delete that file when done.

## 9. Dream cycle (FR-010)

`cargo test` asserts that neither `set_secret` nor `get_secret` is in `VizierTools::DREAM_TOOL_NAMES`, and that `dream_tools` does not offer them. With dummyplug the dream prompt produces no tool calls, so a live dream check is optional: grant a real provider, run the dream cycle, and confirm the dream never calls either tool (or that `agent_secret_event` gains no row for the dream session).

## 10. Final checks

```sh
cargo test
cargo clippy
cd webui && npm run typecheck
```

All must pass. The `secrets` category is reserved: an MCP server keyed `secrets` is exposed as `secrets_mcp` by the existing catalogue rule.
