# Install the Server

> **Server edition.** Looking for the desktop app? See [Install the Desktop App](../app/installation.md). Not sure which one? See [Choose an Edition](../getting-started/editions.md).

The server is one `vizier` binary with the WebUI built in. It runs on Linux and macOS (x86_64 and aarch64), in Docker, and on Kubernetes. Windows is supported through the [desktop app](../app/installation.md) only.

## Prerequisites

None for the standard install: the script downloads a prebuilt binary for your platform.

Optional, depending on what you enable later:

- **git**: needed by `vizier skill install` (registry and git sources)
- **Docker**: only if you run Vizier in a container
- **Ollama / llama.cpp**: only if you want local models
- **PostgreSQL with pgvector** and **S3-compatible storage**: only for larger deployments, see [Storage & Shell](../configuration/storage-shell.md)

## Install

### Install script (recommended)

```sh
curl -fsSL https://get.vizier.rs | sh
```

It installs to `$HOME/.local/bin` (override with `INSTALL_DIR=...`). Pin a version with `VERSION=x.y.z`.

### Cargo

```sh
cargo install vizier
# or, prebuilt and faster:
cargo binstall vizier
```

### Docker

The image is published to Docker Hub (`blinfoldking/vizier`) and GHCR (`ghcr.io/vizier-lab/vizier`); they are identical.

```sh
docker run --rm -p 9999:9999 -v vizier-data:/data blinfoldking/vizier
```

The container takes its settings from environment variables such as `VIZIER_PORT`, `VIZIER_JWT_SECRET` and `VIZIER_DATA_DIR`; the full list is in [CLI → Docker](../configuration/cli.md#docker). The repository also has a sample `docker-compose.yaml`. **Set your own `VIZIER_JWT_SECRET`**: the image ships a placeholder.

### Kubernetes

A Helm chart with PostgreSQL, S3 and an encryption-key Secret is described in [Kubernetes](./kubernetes.md).

### From source

See [Building from Source](../getting-started/contributing.md).

## Check it works

```sh
vizier --version
vizier run
```

Then open `http://localhost:9999`. The [Quick Start](./quick-start.md) takes it from there.

## Updating

| Installed with | Update with |
|---|---|
| Install script | Run the script again |
| Cargo | `cargo install-update vizier` (needs `cargo install cargo-update` once) |
| Docker | `docker pull blinfoldking/vizier:latest`, then recreate the container |

Your data lives in the workspace (see [Configuration Overview](../configuration/index.md#workspace-layout)), outside the binary, so updating keeps it. Deployments that still use the old `filesystem` storage backend are migrated into SQLite automatically on the first start after updating. See [Storage & Shell](../configuration/storage-shell.md).

> [!NOTE]
> Back up the workspace and the [encryption key](../configuration/encryption.md) together, and keep the key somewhere separate from the database.
