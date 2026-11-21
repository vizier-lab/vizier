# Desktop App Quick Start

> **Desktop app edition.** For the server, see the [server quick start](../server/quick-start.md).

You need the app [installed](./installation.md) and an API key for a model provider, or [Ollama](https://ollama.com) for local models. There is no config file, no port to open and no command line.

## 1. Open the app and create the owner

The first launch asks for a name and a password (at least 8 characters) for the local owner. There is no login screen afterwards: opening the app is signing in. The password only matters if you later turn on [web access](./web-access.md). Details are in [First run](./installation.md#first-run).

## 2. Connect a provider and create an agent

Follow [Your First Agent](../getting-started/first-agent.md): add a provider under **Settings → Providers**, create an agent from Home and say hello.

## 3. Things that differ from the server

- **Your workspace is a folder** in the app's data directory. **Settings → Desktop** shows it and can open another.
- **Closing the window keeps agents running** in the tray, so scheduled tasks and Discord/Telegram bots keep working. Quit from the tray icon to stop everything.
- **No users to manage**: you are the only person unless you turn on web access.
- **Notifications** arrive as operating system notifications when the window is hidden.
- **Projects** can be created from a folder on your computer (experimental, see [Projects](../configuration/projects.md)).

## 4. Reach it from elsewhere (optional)

Turn on [web access](./web-access.md) to open the app's WebUI from a browser, a phone or another computer.

## Next steps

- [Desktop App: install, workspaces, updates](./installation.md)
- [Providers](../configuration/providers.md) and [Agents](../configuration/agents.md)
- [Color Themes](../configuration/themes.md) under **Settings → Appearance**
