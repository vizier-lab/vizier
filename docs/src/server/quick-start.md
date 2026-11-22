# Server Quick Start

> **Server edition.** For the desktop app, see the [desktop app quick start](../app/quick-start.md).

From a fresh install to a running agent in a few minutes. You need Vizier [installed](./installation.md) and an API key for a model provider, or [Ollama](https://ollama.com) for local models.

## 1. Run

```sh
export VIZIER_JWT_SECRET="$(openssl rand -hex 32)"   # signs login tokens
vizier run
```

Vizier works with **no config file**. The config file is looked up in this order: `-c <path>`, `$VIZIER_CONFIG`, `./.vizier.yaml`, then the built-in defaults. With no file, the workspace is `$VIZIER_DATA_DIR` or `$HOME/.vizier` and storage is SQLite. See [Config-less mode](../configuration/index.md#config-less-mode).

Add `-d` to run in the background, and stop it with `vizier shutdown`.

Optional: `vizier onboard` is a wizard that writes a seed `.vizier.yaml` (workspace path, HTTP port, JWT secret, worker threads, WebSocket idle timeout). Skip it if the defaults suit you. Everything else is set in the WebUI.

## 2. Create the first user

Open `http://localhost:9999`. A fresh server sends you to a page that creates the first user, who gets the `superadmin` role. After that, sign in normally. (The same through the API: `GET /api/v1/auth/setup-status`, then `POST /api/v1/auth/setup` with `{ "username", "password" }`.)

## 3. Connect a provider and create an agent

From here the steps are the same as in the desktop app: connect a model under **Settings → Providers**, create an agent from Home, and talk to it. Follow [Your First Agent](../getting-started/first-agent.md).

## 4. Make it reachable and safe

Before you expose the server beyond your own machine:

- Put it behind HTTPS (a reverse proxy such as Caddy or nginx) and keep `VIZIER_JWT_SECRET` private.
- Create a user per person and give them roles under **Settings → Users** and **Roles** rather than sharing the first account.
- Back up the workspace and the [encryption key](../configuration/encryption.md).
- Connect Discord or Telegram per agent in [Channels](../configuration/channels.md).

## Running it long term

- Run it under systemd, Docker or Kubernetes so it restarts. A subsystem that stops makes `vizier run` exit with code 1 so the supervisor restarts it, instead of running half-working.
- Send traces and metrics to your collector with [Observability](../configuration/observability.md).
- For managed databases and object storage, see [Storage & Shell](../configuration/storage-shell.md) and [Kubernetes](./kubernetes.md).

## Next steps

- [CLI reference](../configuration/cli.md)
- [Main configuration](../configuration/main-config.md)
- [REST API](../api-integration/rest-api.md)
