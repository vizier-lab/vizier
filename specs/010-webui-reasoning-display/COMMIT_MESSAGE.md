# Commit message for this feature

`git-cliff` renders `[**breaking**]` from the `!` in the type (`cliff.toml:24`), and
`BREAKING CHANGE:` footers become the changelog's breaking-change notes. Both breaking changes
from [spec.md](./spec.md) § Breaking Changes are named below.

```text
feat!: keep reasoning and tool activity attached to the turn that produced it

An agent's thinking, narration and tool calls were shown live and thrown away when
the answer arrived. They are kept now, folded, attached to their turn, and they
survive a reload.

- session_history gains a nullable `seq` column, assigned on every insert and applied
  as a tie-break at every ordered read. A turn's entries are flushed in one tight loop,
  so sharing a millisecond is the normal case, not an edge case.
- `AssistantMessage` entries are recorded and returned instead of filtered on both the
  write and the read path. They are written before the tool calls they accompany, and
  replay merges the two back into the one assistant message the model sent, so a
  narrated tool turn still satisfies tool-call/result adjacency.
- `execute_python` takes a required `intent`: one sentence, in plain language, that the
  person watching reads in place of the script.
- A python script's own tool calls no longer appear anywhere in the WebUI. A new
  `VizierSessionHook::on_nested_tool_call` (defaulting to `on_tool_call`) lets the
  streaming hooks stay quiet for them, and the report's tool-call list is gone from the
  view. What the agent receives is unchanged.
- The WebUI gains one normalized trail model with two producers — the live stream and
  stored history — feeding one renderer, so a turn does not change appearance on refresh.

BREAKING CHANGE: history recorded before this change is not repaired. Those entries keep
a NULL ordering position permanently — there is no backfill, because session_history grows
without bound — so entries that share a timestamp and predate this change still come back
in an arbitrary order. Ordering guarantees apply only to entries recorded after it.

BREAKING CHANGE: `execute_python` now requires an `intent` argument alongside `code`. A call
that omits it fails deserialization and returns an error naming the field, without running.
Agents adapt without intervention because the field is in the tool definition they are handed;
any caller that hand-writes an `execute_python` call must add it.
```
