# Quickstart: Encrypted Storage of Secrets at Rest

**Feature**: `031-encrypt-agent-secrets`

This checks the feature against a running binary, mostly on the offline `dummyplug` provider (spec `008-dummyplug-provider`). The dummyplug protocol is `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`; every agent tool call goes through `execute_python` (code mode), and dummyplug runs one tool per turn. Steps marked **LIVE** need a real network/provider and are not part of the offline gate; everything else is offline.

Use a scratch workspace so the test values never touch a real install:

```sh
rm -rf /tmp/vizier-encryption /tmp/vizier-encryption-restore
VIZIER_JWT_SECRET=quickstart-secret \
  cargo run -- run --workspace /tmp/vizier-encryption --port 9999 2>&1 | tee /tmp/vizier-encryption.log
```

`VIZIER_JWT_SECRET` is required in config-less mode. Creating the first agent also loads the default local embedding model; on a machine that has never run Vizier it is downloaded once into `<workspace>/.runtime/embeddings/` (or copy that directory from another workspace to stay offline).

On first run, create the admin and a dummyplug agent:

```sh
curl -s localhost:9999/api/v1/auth/setup \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"admin"}'

TOKEN=$(curl -s localhost:9999/api/v1/auth/login \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"admin"}' | jq -r .data.token)
API=localhost:9999/api/v1
auth=(-H "authorization: bearer $TOKEN" -H 'content-type: application/json')

curl -s "${auth[@]}" "$API/agents" -d '{
  "agent_id":"dp-a","name":"Dp A","description":"encryption quickstart",
  "provider":"dummyplug","model":"dummyplug"
}' | jq .data.agent_id
```

Use the WebUI chat for `dp-a` in the tool steps, or send the same JSON through the session WebSocket. The expected replies below are the dummyplug echoes.

## 1. Fresh install: a key is generated with no setup

While vizier is running (or after a clean stop), inspect the key:

```sh
stat -c '%a %n' /tmp/vizier-encryption/.runtime/encryption.key
cat /tmp/vizier-encryption/.runtime/encryption.key
```

Expect mode `600` (on Linux/macOS; Windows logs a warning instead) and a two-line file:

```text
vizier-key-v1
<base64 of 32 bytes>
```

No prompt was shown, and no `VIZIER_ENCRYPTION_KEY` was needed. The key file is outside `vizier.db`.

## 2. Agent secret: set, get, and raw-database check (US1, US2)

In `dp-a`, send:

```json
{"tool":"execute_python","arguments":{"code":"print(set_secret(name='AGENT_TOKEN', value='agent-token-abc123'))"}}
```

Expect `created` and `AGENT_TOKEN`; the nested call arguments in the report show `[redacted]` for the value. The script itself contains the value, so it is in session history — that is the documented set-input exception. In a later turn:

```json
{"tool":"execute_python","arguments":{"code":"result = get_secret(name='AGENT_TOKEN')\nprint(result)"}}
```

Expect `found` and `"value": "[redacted]"` in the report. The script received the real value; the report did not.

Create a second secret through the HTTP API, so its value never enters any transcript and the whole-database scan below is meaningful:

```sh
curl -s "${auth[@]}" -X PUT "$API/agents/dp-a/secrets/WEB_TOKEN" \
  -d '{"value":"web-secret-abc123"}' | jq '.data.name'
```

Stop vizier (Ctrl-C). Check the stored column and scan the data files:

```sh
python3 - <<'PY'
import sqlite3
c = sqlite3.connect("/tmp/vizier-encryption/.runtime/vizier.db")
row = c.execute("SELECT value FROM agent_secret WHERE name='AGENT_TOKEN'").fetchone()
print("encrypted:", row[0].startswith("enc:v1:"))
print("plaintext in column:", "agent-token-abc123" in row[0])
PY

grep -a -c 'web-secret-abc123' /tmp/vizier-encryption/.runtime/vizier.db
grep -a -c 'web-secret-abc123' /tmp/vizier-encryption/.runtime/vizier.db-wal 2>/dev/null || true
```

Expect `encrypted: True`, `plaintext in column: False`, and `0` from both greps. The `agent_secret.value` column now holds `enc:v1:…`. `agent-token-abc123` is not scanned across the whole database because the agent set input is in session history by design; `web-secret-abc123` was set through the API and has no such copy. Restart with the same command and `get_secret` for `WEB_TOKEN` in a script: the exact value comes back while the report stays redacted.

## 3. Provider, custom provider, integration and shell env (US1, US5)

Save a fake provider key and a custom provider:

```sh
curl -s "${auth[@]}" -X PUT "$API/providers/openai" \
  -d '{"api_key":"provider-secret-abc123"}' | jq .data.source

curl -s "${auth[@]}" -X POST "$API/providers/custom" -d '{
  "id":"acme","name":"Acme","protocol":"openai_compatible",
  "base_url":"https://acme.invalid/v1",
  "api_key":"custom-secret-abc123",
  "headers":{"Authorization":"Bearer custom-header-secret-abc123"},
  "models":[{"id":"acme-1"}]
}' | jq '.data | {id, has_api_key, headers, base_url}'
```

Expect `has_api_key: true`, header names only, and the plain `base_url`. The API never returns the key or header value.

Add a Brave Search addon (siblings: `safesearch`) and an MCP server (siblings: `transport`, `command`, `args`; secret: `env`):

```sh
curl -s "${auth[@]}" -X POST "$API/agents/dp-a/integrations" -d '{
  "kind":"brave_search","name":"Brave",
  "settings":{"api_key":"brave-secret-abc123","safesearch":true}
}' | jq '.data.integration.settings'

curl -s "${auth[@]}" -X POST "$API/agents/dp-a/integrations" -d '{
  "kind":"mcp","name":"crypto-mcp",
  "settings":{"transport":"local","command":"cat","args":["--verbose"],
              "env":{"GITHUB_TOKEN":"mcp-secret-abc123"}}
}' | jq '.data.integration.settings'
```

Expect `safesearch: true`, `transport: "local"`, `command: "cat"`, `args: ["--verbose"]` in the responses; `api_key`/`env` are masked (or `has_api_key`), never plaintext.

Create an agent with a shell env secret. Use a second agent so `dp-a` keeps running:

```sh
curl -s "${auth[@]}" "$API/agents" -d '{
  "agent_id":"dp-env","name":"Dp Env","provider":"dummyplug","model":"dummyplug",
  "tools":{"shell":{"environment":"local","path":"/bin/sh",
                     "env":{"SHELL_TOKEN":"shell-secret-abc123"}}}
}' | jq '.data.agent_id'
```

Stop vizier and scan for every value:

```sh
for v in provider-secret-abc123 custom-secret-abc123 custom-header-secret-abc123 \
         brave-secret-abc123 mcp-secret-abc123 shell-secret-abc123; do
  printf '%s: ' "$v"
  grep -a -c "$v" /tmp/vizier-encryption/.runtime/vizier.db || true
done
```

Expect `0` for every line. Check the siblings are still readable:

```sh
grep -a -o '"safesearch":true' /tmp/vizier-encryption/.runtime/vizier.db | head -1
grep -a -o '"base_url":"https://acme.invalid/v1"' /tmp/vizier-encryption/.runtime/vizier.db | head -1
grep -a -o '"model":"dummyplug"' /tmp/vizier-encryption/.runtime/vizier.db | head -1
```

Expect each to match. The secrets are gone; the configuration is not.

**LIVE**: `POST /providers/openai/test` and a real Brave call would prove the decrypted values still reach the provider; they are not part of the offline gate.

## 4. Restart, backup, restore and key loss (US3, US4)

Restart the same workspace; `get_secret` for `AGENT_TOKEN` must still work, proving the key persisted and the value was not re-encrypted or lost.

Copy the workspace **with** the key:

```sh
cp -a /tmp/vizier-encryption /tmp/vizier-encryption-restore
VIZIER_JWT_SECRET=quickstart-secret \
  cargo run -- run --workspace /tmp/vizier-encryption-restore --port 9998
```

`get_secret` in the restored copy returns the value. Stop it.

Copy without the key:

```sh
cp -a /tmp/vizier-encryption /tmp/vizier-encryption-nokey
rm /tmp/vizier-encryption-nokey/.runtime/encryption.key
VIZIER_JWT_SECRET=quickstart-secret \
  cargo run -- run --workspace /tmp/vizier-encryption-nokey --port 9998
```

Expect a non-zero exit with a message like:

```text
encrypted data exists but no encryption key was found; set VIZIER_ENCRYPTION_KEY or restore the key file
```

No new key file was created. Restore the key into the copy and start again; the value works.

Wrong key:

```sh
VIZIER_JWT_SECRET=quickstart-secret \
VIZIER_ENCRYPTION_KEY="$(head -c 32 /dev/urandom | base64 | tr -d '\n')" \
  cargo run -- run --workspace /tmp/vizier-encryption --port 9998
```

Expect a non-zero exit saying the key does not match; no data is changed.

## 5. Upgrade migration from plaintext (US2)

Use a copy so the main test workspace is untouched:

```sh
cp -a /tmp/vizier-encryption /tmp/vizier-encryption-migrate
```

Simulate a pre-encryption row by replacing one stored value with plaintext, and remove the marker (keep the same key, so the rest of the data stays readable):

```sh
python3 - <<'PY'
import sqlite3, json
db = "/tmp/vizier-encryption-migrate/.runtime/vizier.db"
c = sqlite3.connect(db)
c.execute("UPDATE agent_secret SET value='migrate-token-abc123' WHERE name='AGENT_TOKEN'")
c.execute("DELETE FROM state WHERE key='migration.secrets_encrypted_v1'")
c.commit()
PY
```

Start the copy. It must:

- create no new key (the key file is present);
- log a migration count;
- let `get_secret('AGENT_TOKEN')` return `migrate-token-abc123`;
- leave no plaintext: `grep -a -c 'migrate-token-abc123' .runtime/vizier.db` → `0`;
- write `migration.secrets_encrypted_v1` to `state`.

Start it a second time; the log must show no conversion and the value must be unchanged. If `grep` finds the plaintext, the `VACUUM`/checkpoint step failed; that is a feature failure.

## 6. Tamper detection and final gates (US1, US5)
Stop the copy, alter one character of the protected value, and start it:

```sh
python3 - <<'PY'
import sqlite3
db = "/tmp/vizier-encryption-migrate/.runtime/vizier.db"
c = sqlite3.connect(db)
c.execute("UPDATE agent_secret SET value = value || 'x' WHERE name='AGENT_TOKEN'")
c.commit()
PY
```

Start and `get_secret('AGENT_TOKEN')`; expect a clear integrity/decryption error, not a partial value and not a silent "not found". Restore the copy or reset the secret afterwards.

Final checks:

```sh
cargo clippy
cargo test
grep -a -c 'web-secret-abc123' /tmp/vizier-encryption/.runtime/vizier.db
```

- `cargo clippy` and `cargo test` pass.
- The final grep is `0` after a clean shutdown.
- The key file is `600` on Unix and was never inside `vizier.db`.
- `docs/src/configuration/encryption.md` states what is protected, what is not, how to back up the key, and that losing it makes protected data unrecoverable.

## 7. Diagnostic scan (US5)

After the final clean shutdown, scan the log, the database and the WAL for the values set through the API or configuration surfaces and for the key material:

```sh
for v in web-secret-abc123 provider-secret-abc123 custom-secret-abc123 \
         custom-header-secret-abc123 brave-secret-abc123 mcp-secret-abc123 \
         shell-secret-abc123; do
  printf '%s log=' "$v"
  grep -a -c "$v" /tmp/vizier-encryption.log || true
  printf ' db='; grep -a -c "$v" /tmp/vizier-encryption/.runtime/vizier.db || true
  printf ' wal='; grep -a -c "$v" /tmp/vizier-encryption/.runtime/vizier.db-wal 2>/dev/null || echo 0
done
KEY=$(sed -n '2p' /tmp/vizier-encryption/.runtime/encryption.key)
printf 'key in db='; grep -a -c "$KEY" /tmp/vizier-encryption/.runtime/vizier.db || true
printf ' key in log='; grep -a -c "$KEY" /tmp/vizier-encryption.log || true
```

Expect `0` for every line. Do **not** include `agent-token-abc123` in the log scan: it was supplied by the agent inside the `set_secret` script, so it appears in that turn's request/debug log and session history — the documented set-input exception (`specs/027-agent-secrets`). The value is still absent from the stored column and from every management response.

**LIVE/optional**: connect a real Discord or Telegram bot and confirm it still reads its token after an upgrade; this needs a network and a valid token and is not part of the offline gate.
