# Contract: dummyplug §7, `pruned`

To be added to `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`.

## 7. `pruned`: list the placeholders the model was sent

**Input**: the command text of the incoming user message, trimmed, is exactly `pruned`
(case-insensitive).

**Behaviour**: the reply is one text message listing every tool result in the request the model
received that is a pruning placeholder, in order, one per line:

```text
<n> pruned:
- <call_id> <record> <size>
```

or `(nothing pruned)` when there is none.

**Precedence**: checked after §1 (`tools`) and §6 (`context`), before §2, §3 and §5. No built-in
tool is named `pruned`.

Additive: no existing command changes, so existing quickstart scripts keep working.
