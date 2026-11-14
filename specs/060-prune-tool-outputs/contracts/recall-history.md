# Contract: `recall_history`

Category `history`. On `default_toolset`, reached from scripts like every tool. Not offered to dreams.

## Input

```json
{ "id": "<history uid from a placeholder>" }
```

## Output

Found: the id names a tool result of **this** conversation.

```json
{ "status": "found", "id": "<id>", "output": <the stored text, parsed as JSON when it is JSON, else the string> }
```

Not found: no such entry, an entry that is not a tool result, or an entry of another conversation
or agent. The three cases are indistinguishable on purpose.

```json
{ "status": "not_found", "id": "<id>" }
```

- No size limit and no truncation (FR-010). This will be revisited after real use.
- Never an `Err` for a miss. A storage failure is an `Err` like any tool's.
- Description (as offered): "Read back a tool output that was pruned from this conversation's
  view, by the id in its placeholder. Returns {status: \"found\", id, output} with the output
  exactly as stored, or {status: \"not_found\", id}. Call it in a script and return only the part
  you need."

## Example script

```python
a = recall_history(id="0f6c…")["output"]
b = recall_history(id="91ab…")["output"]
[line for line in a["result"].splitlines() if "fn prune" in line] + b["result"][:400]
```
