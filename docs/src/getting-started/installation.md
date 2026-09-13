# 1.1 Installation

## Prerequisites

None for the standard install — the script downloads a prebuilt binary for your platform (Linux/macOS, x86_64/aarch64).

Optional, depending on what you enable later:

- **git** — required by `vizier skill install` (registry and git sources)
- **Docker** — only if you configure an agent's shell with `environment: docker`
- **Ollama / llama.cpp** — only if you want to run local models

### Building from source

- [Rust and Cargo](https://rust-lang.org/) (edition 2024)
- Node.js + npm (the WebUI is built by `build.rs` during `cargo build`)

## Installing Vizier

### Standard Installation (Recommended)

```sh
curl -fsSL https://get.vizier.rs | sh
```

Installs to `$HOME/.local/bin` (override with `INSTALL_DIR=...`). Pin a version with `VERSION=x.y.z`.

### Cargo

```sh
cargo install vizier
```

Or with cargo-binstall (prebuilt, faster):

```sh
cargo binstall vizier
```

### Docker

The image is published to Docker Hub (`blinfoldking/vizier`) and GHCR (`ghcr.io/vizier-lab/vizier`); they are identical.

```sh
docker run --rm -p 9999:9999 blinfoldking/vizier
```

See [CLI → Docker](../configuration/cli.md#docker) for the full list of environment variables, or use the sample `docker-compose.yaml` in the repository.

## Building from Source

```sh
git clone https://github.com/vizier-lab/vizier
cd vizier
just install        # cargo fetch + npm install in webui/
cargo build --release
```

> **Build note:** `build.rs` runs `npm run build` in `webui/` on every `cargo build` **if** `webui/node_modules/` exists. If `node_modules/` is missing and `webui/build/client/` doesn't exist either, the build fails — run `just install` first.

## Updating

### Install script

Re-run the installer:

```sh
curl -fsSL https://get.vizier.rs | sh
```

### Cargo

```sh
cargo install cargo-update   # once
cargo install-update vizier
```

### Docker

```sh
docker pull blinfoldking/vizier:latest
```

> Upgrading from a deployment that used the old `filesystem` storage backend is handled automatically: on first startup the data is migrated into the embedded SQLite database. See [Storage](../configuration/storage-shell.md).
