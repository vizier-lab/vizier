# Mockups (ASCII) — Share dialog, viewer, My shares

Constitution: design before UI code. FigCraft is the preferred medium; this ASCII version is the fallback and is what is reviewed first. Copy follows the WebUI's existing tone.

## 1. Share dialog (chat header ▸ Share)

```
┌ Share this conversation ─────────────────────────────────────── ✕ ┐
│  [ Link ]  [ Offline file ]                                        │
│                                                                    │
│  Who can open it                                                   │
│   ( ) Specific people      [ add people…            ▾ ]            │
│   ( ) Anyone signed in on this instance                            │
│   (•) Anyone with the link                                         │
│                                                                    │
│  [ ] Include the agent's activity trail (tool calls and results)   │
│      Off: only what you and the agent said, plus images and pages. │
│                                                                    │
│  What will be shared                                               │
│   42 messages · 3 images · 1 interactive page · 612 KB of 5 MB     │
│   Left out automatically:                                          │
│     2 recalled memory passages · 7 reasoning steps                 │
│     1 key-like text replaced · 4 server paths shortened            │
│   ⚠ 1 image over 1 MB was replaced by a placeholder                │
│   [ Preview what a visitor sees ]                                  │
│                                                                    │
│  Anyone with the link can read this. You can revoke it later,      │
│  but a copy someone already made cannot be taken back.            │
│                                                   [Cancel] [Create link] │
└────────────────────────────────────────────────────────────────────┘
After: [ https://vizier.example/share/V1StGXR8_Z5jdHi6B-myT  (Copy) ]
Desktop app: the Link tab says "A link needs a server others can reach. Use Offline file."
No permission: no Share button.  Empty chat: button disabled, tooltip "Nothing to share yet".
```

## 2. Viewer (`/share/:id` and the exported file)

```
┌───────────────────────────────────────────────────────────────┐
│ Trip planning with Atlas                       Shared snapshot │
│ Read-only copy · 8 Nov 2026 · activity trail not included      │
├───────────────────────────────────────────────────────────────┤
│  You   Plan a 3-day trip to Kyoto …                            │
│  Atlas Here is a plan …  [image]  [ 3D scene (drag to rotate) ]│
│        ┌ Interactive page ───────────────┐  (buttons that talk │
│        │ [Day 1] [Day 2] [Day 3]         │   to the agent show │
│        └─────────────────────────────────┘   "not in a copy")  │
├───────────────────────────────────────────────────────────────┤
│  This is a copy; the conversation can't continue here.         │
└───────────────────────────────────────────────────────────────┘
Neutral page (unknown / revoked / not allowed): "This link isn't available."  [Sign in] (only if the server said 401)
Damaged file: "This file is damaged or was edited, so it was not opened."
```

## 3. My shares (Settings ▸ Sharing)

```
 Title                  Who          Trail  Created      
 Trip planning          Anyone       no     8 Nov 2026   [Copy link] [Revoke]
 Q3 draft               2 people     yes    6 Nov 2026   [Copy link] [Revoke]
 Old demo               Anyone       no     1 Nov 2026   revoked
 (admin, with "All shares": + Author column)
```

## Review (T007)

- Tinjauan maintainer **belum diperoleh**: mockup ASCII ini belum dibaca atau disetujui maintainer.
- FigCraft **tidak dicoba**.
- Implementasi UI (T036, T048, T033–T035) dilanjutkan **atas perintah delegator**, bukan karena tinjauan. Jika maintainer meninjau nanti, catat hasilnya di sini; T007 tetap belum dicentang.
- Penyimpangan dari mockup di MVP ini: tab Offline file belum ada (US4); pilihan audiens selain "Anyone with the link" tampil nonaktif ("coming soon"); label giliran di penampil memakai "Person" bukan "You" (pengunjung bukan penulisnya).
