# Contract: WebUI `Transport` (`webui/app/api/transport/`)

The only module in the WebUI allowed to call `fetch`, open a `WebSocket`, build an
`/api/v1/...` URL or import `@tauri-apps/api`.

```ts
export type AuthMode = 'http' | 'desktop'

export interface Transport {
  authMode: AuthMode

  /** JSON request; resolves to the envelope's `data` (or the envelope when raw). Same errors as today's `request`. */
  request<T>(method: string, path: string, body?: unknown, init?: { signal?: AbortSignal; raw?: boolean }): Promise<T>

  /** Multipart upload. `http`: fetch with FormData. `desktop`: Response(form) → raw `api` invoke. */
  upload<T>(path: string, form: FormData): Promise<T>

  /** Binary GET with optional range (preview, bundle export). */
  fetchFile(path: string, init?: { range?: [number, number]; signal?: AbortSignal }): Promise<Response>

  /** A URL usable in <img>/<audio>/<video>/<a>. `http`: `/api/v1{path}?token=…`. `desktop`: `vizier-asset://localhost/api/v1{path}`. */
  assetUrl(path: string): string

  /** Save or open a file. `http`: anchor download / window.open. `desktop`: `save_file` / `open_file`. */
  saveFile(path: string, filename: string): Promise<void>
  openFile(path: string, filename: string): Promise<void>

  /** Live frames for one topic. Same callbacks as today's TopicSocket. */
  openTopic(t: { agentId: string; channelId: string; topicId: string }, opts: TopicSocketOptions): TopicHandle

  /** OS notification (desktop) or no-op (http, where the in-app toast is used). */
  notify(n: { title: string; body: string; href: string }): Promise<void>

  /** Desktop-only shell calls; undefined over http. */
  desktop?: DesktopShell
}

export interface TopicHandle { readonly isOpen: boolean; send(frame: object): boolean; retryNow(): void; close(): void }
```

## Selection

`'__TAURI_INTERNALS__' in window` → `tauri.ts` (loaded with dynamic `import()`), else
`http.ts`. Decided once at startup; exported as `transport`.

## Behaviour that must match across both

| Case | http | desktop |
|---|---|---|
| Envelope unwrapping, `ApiError(status, code, message, data)` | from `fetch` | from `api` result `{status, body}` |
| 401 | clear token, go to `/login` | cannot happen for the bound user; if it does (user deleted), show the bootstrap screen |
| Session bootstrap | token → `GET /auth/users/me` | `desktop_state` → `ready` → `GET /auth/users/me` |
| Sign out | shown | hidden |
| Topic reconnect / `reopened` | WebSocket backoff | `topic_open` again after `desktop://state` returns to `ready` |
| Asset with token | `?token=` query | none (the scheme carries no credential) |

## Call sites to migrate (exhaustive, from research)

`api/client.ts`, `api/socket.ts`, `api/endpoints/chat.ts` (socket URL), `api/endpoints/memory.ts`
(bundle export), `api/endpoints/artifacts.ts`, `stores/connection.ts`, `stores/session.ts`,
`screens/shared/FilePreviewModal.tsx`, `screens/chat/FilePreviewDialog.tsx`,
`screens/artifacts/ArtifactsGallery.tsx`, `lib/generativeUi/media.ts` and
`lib/generativeUi/registry/{anchor,media,canvas2d}.tsx`, `lib/trail.ts`,
`screens/shared/NotificationToasts.tsx` (calls `notify` when hidden), the sign-out control.

## New desktop-only screens

- `screens/desktop/Bootstrap.tsx`: `starting` (spinner), `locked` (holder + "Open another
  workspace"), `needs_owner` (name, optional password), `choose_owner` (list), `failed`
  (error + Restart). Rendered instead of the router outlet until `ready`.
- `screens/desktop/RuntimeBanner.tsx`: `degraded` banner with Restart.
- Settings › Desktop (`routes/settings-desktop.tsx`, nav item only when `authMode ===
  'desktop'`): workspace folder, close to tray, web access (toggle, allow other devices with
  warning, port, current URL), updates.

All use existing `@vizier/design` components and follow the dismissal convention.
