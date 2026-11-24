# webui

The Vizier WebUI: a React Router 7 single-page app on the `@vizier/design` component library. Vizier embeds its build (`build/client/`) in the binary and serves it.

It is the `webui/` folder of the vizier repository and depends on `../design`, the component library beside it.

```sh
git clone --recurse-submodules https://github.com/vizier-lab/vizier.git
cd vizier && just install      # submodules (landing, dummyplug), npm i in design/ and webui/, cargo fetch
```

## Develop

```sh
just run                       # vizier on :9999 (separate terminal)
cd webui && npm run dev        # the app on http://localhost:5173, proxying /api (and the WebSocket) to :9999
```

Set `VIZIER_URL` to proxy to another server.

| Script | What it does |
|--------|--------------|
| `npm run dev` | Vite dev server on `:5173` |
| `npm run build` | Writes `build/client/` (what vizier embeds) |
| `npm run typecheck` | `react-router typegen && tsc` |
| `npm test` | vitest, for `app/lib` and `app/api` |
| `npm run e2e` | Playwright against a running vizier (`VIZIER_URL`, `VIZIER_USER`, `VIZIER_PASSWORD`) |
| `npm run gen:api` | Regenerates `app/api/schema.d.ts` from `openapi.json`; `node ../scripts/openapi.mjs` fetches the schema from a running server first |

## Layout

`app/routes/` one file per screen, `app/screens/<screen>/` the compositions a screen needs, `app/layouts/` the shell, `app/stores/` Zustand stores, `app/lib/` pure logic with its tests, `app/api/` the typed client and the topic socket, `e2e/` the Playwright suite.

Screens compose library components only, with token utilities and no arbitrary values. A change to a component belongs in `design/`.

## Commit order

A change that needs a new library component is made in `design/` first (with a story), then used here. Both are in the vizier repository, so they go in one branch and one pull request with any server change; `just web-check` runs what CI's `web` job runs.
