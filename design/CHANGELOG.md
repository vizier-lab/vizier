# Changelog

## Unreleased (spec 024, WebUI rewrite)

- Motion: a timing and easing scale (`--vz-duration-fast|base|slow`, `--vz-ease-standard|arrive`), one-shot keyframes (rise-in, fade-in, pop-in, slide-in-right, slide-down-in) and `motion/react` for the parts CSS cannot express (presence exits, shared-layout indicators, springs, height auto). `MotionProvider` honours prefers-reduced-motion.
- Dialog, Modal, SlideOver: the scrim and panel settle in, and animate out before the dialog closes.
- Menu, Picker, Select, SortSelect, EmojiPicker, DateTimeField: popovers fade and rise; the listbox and picker keep their surface while they leave.
- Toast: slides in, and the stack glides up when one is dismissed.
- Switch: the thumb springs between its ends. Checkbox and Radio: the mark pops in.
- Tab, FilterTabs, Segment: the active underline and pill glide between their options.
- ToggleSection: its settings unfold with the switch.
- Sidebar: the rail eases between its two widths.
- Banner: banners rise into place. Tooltip: a quick fade.
- StackedBarChart, BarList, ActivityGrid: bars grow from the baseline, dots arrive by row. NightStrip: the selection halo glides between nights. ContextRing: the arc sweeps on.
- Button and IconButton: a press micro-scale; every hover, press and selected state eases at house timing.

- X81 LoadMore: the end of a paged list, with a button or auto-load look.
- X41 Picker: `onEndReached`, `loading`, `search.mode` for server-paged lists.
- X78 EmojiPicker: a searchable emoji grid in a popover, with a recent row and a bundled list.
- Table: sortable columns (`onSort`, `sorted`) for the CSV preview.
- InteractivePage: `actions` slot beside full screen.
- Composer: while busy, typed text turns Stop back into Send (the message queues).
- X77 DateTimeField: a date and a time in one field, with a month grid and a time row.
- X69 NightStrip: `live`, `hint`, `selected`, `onSelectNight`.
- X56 DreamJournalEntry: a `removed` change kind and `href` on a change.
- Icons: `calendar`.
- X76 MarkdownEditor: Tiptap 3 with markdown in and out; rich, markdown and split modes (the library's only new runtime dependency).
- RevisionRow: `added`/`removed` are optional.
