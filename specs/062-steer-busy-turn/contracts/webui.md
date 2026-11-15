# Contract: WebUI — waiting message, split trail, composer hint

ASCII mockups for the design review (FigCraft was unreachable; constitution v2.2.0 accepts ASCII). Match the existing chat page in both themes; no new colors (the WebUI stays monochrome).

**Status**: approved 2026-11-15.

## W1 — Composer while busy

```text
┌──────────────────────────────────────────────────────────────┐
│ also check the error logs                                    │
│                                                              │
│ 📎  @                                              ■ Stop  ➤ │
└──────────────────────────────────────────────────────────────┘
  Enter adds to the current turn · Ctrl+Enter queues it        ← muted, small, only while busy
```

- Idle: no hint line, layout unchanged.
- macOS shows `⌘↵`; other platforms `Ctrl+Enter`.
- Phone (below `lg`): no hint (no shortcut); `/queue` works.

## W2 — Waiting (sent while busy, not read yet)

```text
 you   refactor the auth module
   ▸ Working · step 3 · execute_python…
 you   also check the error logs              ← existing "queued" row
       Waiting for the agent                   ← steer: "Waiting for the agent"
                                                 queue: "Queued · runs after this turn"
```

## W3 — Read mid-turn (live, after reload, any tab — identical)

```text
 you   refactor the auth module
   ▸ Worked · 3 steps · 12s                    ← trail part 1 (collapsed by default as today)
 you   also check the error logs               ← ordinary bubble, at the point it was read
   ▸ Worked · 5 steps · 40s                    ← trail part 2 (live part shows "Working…")
 agent Done. Refactored the module; the error logs show…
```

- Several messages read at one boundary: consecutive bubbles between the two trail parts.
- Read at two different boundaries: three trail parts.
- A background delivery at the same boundary stays a step inside the trail (part 2's first step), not a bubble.
- A steer message that fell back (turn ended first): ordinary new turn after the reply.

## W4 — Rules

- Only `Request` entries with `mid_turn: true` **and chat content** split a trail; a mid-turn silent read is drawn as silent reads are today.
- Trail part durations/step counts come from that part's own steps.
- Reactions, copy, and other bubble actions work on a mid-turn bubble as on any person message.
- Every popup involved (none new) follows the dismissal convention.
