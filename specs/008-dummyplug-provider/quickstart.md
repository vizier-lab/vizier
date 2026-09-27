# Quickstart: Dummyplug Test Provider

These steps check the feature by hand, end to end. They are needed because the automated suite is sparse; see the constitution's quality gates.

## 1. Run with no credentials

```sh
env -u OPENAI_API_KEY -u ANTHROPIC_API_KEY just run
```

It's fine if `dev.vizier.yaml` has keys, because dummyplug never reads them. To show that nothing is required, you can also run config-less:

```sh
VIZIER_DATA_DIR=$(mktemp -d) cargo run -- run
```

## 2. Create a dummyplug agent

In the WebUI, go to **Agents → New**. Set **Provider** to `dummyplug (testing)`; the model defaults to `dummyplug`. Save.

Or use the HTTP API, with the same body as any other agent and `"provider": "dummyplug", "model": "dummyplug"`.

## 3. Walk the four behaviors in chat

| Send | Expect |
|------|--------|
| `hello` | Lorem ipsum paragraph(s). Sending it again gives different text. |
| `tools` (also try ` Tools `) | Bulleted list of every tool, including any `mcp_*` tools if MCP servers are configured |
| `READ_CORE` | Description, then a fenced JSON with `"tool": "READ_CORE"` |
| *(paste that JSON back unchanged)* | A tool-call entry in the UI, then `**Tool result** (READ_CORE): …` containing the agent's CORE.md |
| `{"tool": "nope"}` | `Unknown tool` message; nothing runs |
| `{"tool": ` | `Could not parse tool request` message; nothing runs |

## 4. Check a state-changing tool (US4-2)

1. Send `memory_write` to get the sample.
2. Edit the values and send it back.
3. Confirm the memory appears in the WebUI Memory view, and that its version history shows a revision.

## 5. Non-interactive paths (FR-010)

- Create a one-time scheduled task for the agent. It should complete, with a lorem ipsum result.
- Set `dream_interval` to a small value, or trigger a dream. The dream should complete without errors.
- Discord/Telegram, if configured: repeat step 3 there. The code-fence stripping lets a JSON block copied from Discord work unchanged.

## 6. Gates

```sh
cargo clippy
cargo test dummyplug
cd webui && npm run typecheck
```
