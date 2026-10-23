# Building from Source

Vizier is one Rust binary that embeds its WebUI. The WebUI and the component library it is built from live in their own repositories, which a vizier checkout pulls in as git submodules:

| Path | Repository | What it is |
|------|------------|------------|
| `webui/` | `vizier-lab/webui` | The WebUI (React Router, built to `webui/build/client/`) |
| `design/` | `vizier-lab/design` | The component library and its Storybook catalogue |
| `legacy/` | (this repository) | The previous WebUI, kept for reference. It is never built, served or packaged |

## First build

```sh
git clone --recurse-submodules https://github.com/vizier-lab/vizier.git
cd vizier
just install     # submodules, npm install in design/ and webui/, cargo fetch
just run         # builds the WebUI, then runs vizier with dev.vizier.yaml
```

If you cloned without `--recurse-submodules`, run `git submodule update --init --recursive`. A `cargo build` with no `webui/node_modules/` and no pre-built `webui/build/client/` stops with a message saying so.

## Working on the WebUI

```sh
just run                    # vizier on :9999
cd webui && npm run dev     # the WebUI on http://localhost:5173, proxying /api to :9999
```

A change that needs a new or changed component is made in `design/` first. Commit it there, then commit the screen in `webui/`, then commit vizier with both submodule pins bumped.
