# Contract: WebUI

**Feature**: `035-add-artifacts`

The WebUI is the `webui/` submodule on `design/`. The design change (sidebar page) is made first, then the WebUI, then the vizier gitlink is bumped. No new UI dependency.

---

## 1. Navigation and route

- Add `artifacts` to the design `SidebarPage` union and the agent-pages group (alongside Memory/Tasks; icon `folder`). Update `Sidebar.stories.tsx`.
- Add `/:agentId/artifacts` to `webui/app/routes.ts`, imported as `routes/artifacts.tsx`.
- Add `artifacts` to `AppShell`'s `AGENT_PAGES` and `PAGE_PATH` (`/artifacts`).
- The page is only reachable with an agent selected; it is not a Settings page and has no separate cross-agent view.
- The gallery is agent-level: an artifact promoted from a Discord/Telegram conversation is visible there under the agent visibility rule (no platform-chat gate; spec FR-029).

---

## 2. Gallery page

`ArtifactsGallery` uses the shared `PageFrame` (title `Artifacts`, description short) and a responsive body.

### Header and stats

- Total artifact count and total bytes for the agent (`agent_total`, `agent_total_bytes`) always visible.
- A primary/secondary tab switch: **Artifacts** / **Activity** (`FilterTabs`).
- Search input (name), kind filter (`All`, Images, PDFs, Text, Markdown, CSV, Audio, Video, Other), and a date range (From/To). The chosen filters persist while paging and while switching pages; they reset only on an explicit Clear filters action.
- A `LoadMore`/paging control with no duplicates/gaps; the cursor from the response is used.

### Artifact list

Each row/card shows:
- name (monospace/truncated, full accessible name), `FileTypeTag`/kind, size, version, last change time, and `Used by N` (from `references` or a summary count).
- an explicit unavailable state if `available: false` (text/icon, not only colour).
- actions: Open/Preview, Download, Delete (only if the person may edit the agent).

Desktop: a `Table` (or dense list) with the columns above. Phone: stacked cards with the same content and actions; no horizontal scroll.

### Detail

Selecting an artifact opens a detail view (right pane on wide screens, full-screen sheet/dialog on phone) with:
- preview using the shared preview body (image, PDF, CSV table/raw, markdown, plain text); unsupported kinds show "No preview for this type. Download it to open it." and a Download button.
- metadata: name, kind/MIME, size, version, created/updated, `available`.
- origin: conversation label/link when readable, source filename.
- provenance: the change history (promoted/replaced/deleted by actor name at time); the activity view has the full agent-wide list.
- references: every message and memory that uses it, with a label and a link when the destination is readable; an empty state "Not attached anywhere yet."
- Delete: `ConfirmDialog` names the artifact, states the reference count and says references will show as deleted. On success, close the detail, remove the row and refresh the stats/list. A 403 is shown; the action is absent for non-editors.

### Activity

Activity rows show action icon/word, artifact name, actor name, time and source (conversation/file). Newest first, paged. Deleted artifacts remain visible as records. The list is read-only.

### States

- Loading skeletons.
- Empty gallery: "No artifacts yet. Promote a file from a conversation to keep it." with a link to Chat.
- Empty filter: "No artifacts match these filters." with Clear filters.
- Empty activity: "No artifact activity yet."
- Load failure: `Notice` with a Retry action; do not show an empty list as if there were none.
- Unavailable content: row/detail shows an unavailable badge; preview/download explains it; no broken image/empty download.

---

## 3. Chat side panel

`SessionFilesPanel` keeps its current session-file list and preview, and adds:

### Promote

- A `Promote` action per file row (e.g. a `TextLink` before the type tag). Opens a small dialog with the file's name prefilled; empty/whitespace or invalid name shows the server error inline.
- Calls `POST …/files/promote` with `{filename, name}`.
- On success: toast `Promoted “<name>” to artifacts.` and a `View in gallery` link. The source stays in the session-file list (copy, no artifact marker).
- A duplicate name error is shown with a hint that an artifact with that name exists; the person can choose another name.
- The panel does not list the agent's artifact library; the gallery is the permanent view.

### Clear session files

- A `Clear session files` button near the panel header, only when the list is non-empty and the turn is idle (server refuses active turns anyway).
- `ConfirmDialog`:
  - title `Clear session files?`;
  - body `This removes N files from this conversation. Artifacts are unaffected. Older messages or memories that reference these files will no longer open them.`;
  - destructive confirm.
- On success: toast `Cleared N session files.`, dispatch the `files` action with `[]`, keep the rest of the chat state.
- On 409 active turn: explain and leave the list unchanged.
- The action clears only the open conversation.

### File preview

- Session files keep the existing `FilePreviewDialog`; promoted source files are not visually different. The preview must not imply permanence.

---

## 4. Message attachments

`frames.ts` extends `VizierAttachment`:

```ts
content:
  | { url: string }
  | { bytes: number[] }
  | { base64: string }
  | { local: string }
  | { artifact: { agent_id: string; artifact_id: string; available?: boolean } }
```

`ReplyMedia`:
- `attachmentUrl` returns the artifact content URL for an `artifact` content:
  `…/api/v1/agents/{agent_id}/artifacts/{artifact_id}/content`, with the auth token query when the URL is used by `<img>`/`<audio>`/`<video>`/`<iframe>`.
- `available === false` renders a `FilePeek`/notice named `"<name> — artifact deleted"` with no download; the rest of the message remains.
- A content fetch 404 (or image load error) flips that attachment to the same unavailable state.
- Available images/audio/video use the existing media components; other kinds use the existing FilePeek/Download path.
- Session-file (legacy `local`/`url`) attachments keep the current rendering.

`FilePreviewDialog` and the new `ArtifactPreviewDialog` share the extracted preview body:
- move `kind`, `readText`, `parseCsv` and the body switch into `screens/chat/FilePreviewContent.tsx` (or equivalent);
- `FilePreviewDialog` supplies the `/files/{file_id}` URL;
- `ArtifactPreviewDialog` supplies the artifact content URL and uses the artifact's display name/mime/size;
- both keep the CSV table/raw switch, markdown rendering, PDF/image viewers, keyboard arrows/Escape, download and open-in-tab.

The chat composer continues to upload people's files as session files; it does not offer an artifact picker for user messages.

---

## 5. Memory screen

- No attachment picker is added in this release. The agent/API path is the supported way to attach artifacts.
- If the memory detail ever shows attachments, it must render artifact names and a deleted/missing state; the current screen does not show them, so this is not a blocker for the gallery.

---

## 6. Accessibility and responsive behavior

- Sidebar entry has an accessible name `Artifacts`.
- Gallery filters have labels; the search input has a placeholder and an accessible name; the kind/date controls are keyboard operable.
- Artifact rows/cards are links/buttons with a meaningful name; delete is a separate control with a confirmation dialog.
- Focus moves into the detail/dialog, is contained, Escape closes, and focus returns to the invoking row/control.
- `available: false`, activity action and delete warnings are conveyed by text/icon in addition to colour.
- At phone widths the gallery uses stacked cards, filters wrap or move into a sheet, the detail is full-screen, and nothing scrolls horizontally.
- Loading/empty/error states are announced; the download action has an accessible name.

---

## 7. Tests

- `lib/artifacts.ts` unit tests: kind mapping, filter query building, artifact content URL, `available: false` tombstone helper, reference labels.
- WebUI store/reducer: the side-panel clear action empties `files` and leaves other topic state; a new session-file frame after clear prepends normally.
- `npm run typecheck` and `npm test`.
- Playwright (`e2e/artifacts.spec.ts`, desktop and phone):
  - sidebar entry and gallery list/stats/search/kind/date/paging;
  - preview image/PDF/CSV/markdown/text and unsupported download;
  - detail provenance/references/activity;
  - delete as owner vs a view-only user (dialog, 403, row removal);
  - side-panel promote (name dialog, toast, source remains) and clear (confirm, count, artifacts remain, active-turn refusal);
  - a reply with an artifact attachment, replacement reflecting new bytes, deleted artifact tombstone;
  - mobile layout and focus/Escape.
- If a browser is unavailable, record the manual checks in `quickstart.md` and run the unit/typecheck gates.
