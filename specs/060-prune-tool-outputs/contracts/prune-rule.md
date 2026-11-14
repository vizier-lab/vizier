# Contract: the pruning rule

`prune_for_model(messages: &[Message]) -> (Vec<Message>, PruneStats)`. It is pure and deterministic,
and does no I/O. The input is already narrowed by `messages_for_model`.

```text
round  := 0
queue  := []            # eligible, unpruned results: (index, tokens, round)
total  := 0
pruned := {}

for (i, m) in messages:
    if m is Assistant: round += 1
    if m is ToolResult and eligible(m):
        t := (bytes(text(m)) + 3) / 4
        queue.push((i, t, round)); total += t
        if total > HIGH_TOKENS:
            while total > LOW_TOKENS and queue.front.round < round:
                (j, tj, _) := queue.pop_front(); total -= tj; pruned.add(j)

output := messages with each j in pruned replaced by
          ToolResult { call_id, record, content: [Text(placeholder(j))] }
```

`eligible(m)`: `record` is `Some`, every part is text, the text is at least `MIN_BYTES`, and it
is not an `execute_python` report with `ok: false` or with a turn-ending call.

## Guarantees (each pinned by a unit test)

1. **Below the mark, identity**: if no prune fires, the output equals the input (SC-002).
2. **Prefix stability**: for any `a` and any `b` appended to it, the pruned set of `a ++ b`
   contains the pruned set of `a` (FR-005), and the first `len(a)` messages of `prune(a ++ b)`
   equal `prune(a)` unless a prune fires while walking `b` (FR-006). So the prefix changes only
   at a prune, never on an ordinary round or between turns.
3. **Newest round whole**: no result of the last round is ever pruned (FR-007).
4. **Hysteresis**: two prunes are separated by at least `HIGH_TOKENS − LOW_TOKENS` tokens of new
   eligible output (SC-005).
5. **Ineligible untouched**: small, image-bearing, unrecorded, failed and turn-ending results
   pass through unchanged and are not counted.
