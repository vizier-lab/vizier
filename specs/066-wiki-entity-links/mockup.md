# Mockup: Entity suggestion rows in the composer popover (T001)

Fallback medium (ASCII) — the FigCraft MCP/app was not reachable in this session (constitution v2.3.0 allows this without stalling). Structure follows design's existing `FileRow`/`MemberRow` rows in `design/src/components/MentionMenu/MentionMenu.tsx`.

Triggered by typing `[[` in the composer. Same popover (`MentionMenu` via design `Composer`'s `suggest` prop), new row variant `EntityRow`.

```text
┌ SUGGESTIONS · Links ────────────────────────────────────────┐
│                                                              │
│  ▤ report                          artifact                  │  ← EntityRow:
│  ▤ draft-v2                        artifact                  │     icon (per kind)
│  ⚙ fetch_weather                   function                  │     name strong
│  ⏱ weekly-report                   task                      │     kind note quiet (right)
│  📗 summarize                      skill · agent             │     scope/extra in note
│  📗 onboarding                     skill · global            │
│  ▤ research                        memory · bundle           │
│  ▤ research/topics/energy          memory                    │
│                                                              │
│  Type to filter · ↑↓ to move · Enter to insert · Esc closes │
└──────────────────────────────────────────────────────────────┘
```

After typing `[[skill:` (kind prefix narrows the list):

```text
┌ SUGGESTIONS · Links ────────────────────────────────────────┐
│  📗 summarize                      skill · agent             │
│  📗 onboarding                     skill · global            │
└──────────────────────────────────────────────────────────────┘
```

Pick → the token becomes `[[skill:summarize]] ` (complete form, trailing space), exactly like `@path ` insertion today. Highlighting of the typed fragment uses the existing `HighlightMatch`.

Row anatomy (matches `MemberRow` rhythm, 14px icon, `type-code` name):

- **Icon per kind** from the existing icon set: artifact `package`/`file`, function `settings`/`function`, task `clock`, skill `book`, memory `book-open` — final names chosen from the design icon set at implementation; tone `faint`.
- **Name**: `text-fg`, truncated.
- **Note** (right, `text-muted`): the kind; for skills the scope (`skill · agent` / `skill · global`); for memory a bundle `memory · bundle` or plain `memory`.

No new colors, no new tokens, no layout change to the popover — one new row variant in the closed `MentionMenuItem` union.
