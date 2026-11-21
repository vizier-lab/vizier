# Choose an Edition

Vizier comes in two editions. Both run the **same engine** and show the **same WebUI**; they differ in where it runs and how you reach it.

|  | **Server edition** | **Desktop app edition** |
|---|---|---|
| What it is | One `vizier` binary (or container) that you run and leave running | A native window (Tauri) that hosts the engine inside the app |
| Platforms | Linux, macOS, Docker, Kubernetes | macOS, Linux (AppImage), Windows 10/11 |
| How you open it | A browser at `http://<host>:9999`, REST API, WebSocket | The app window. No port is opened unless you turn on [web access](../app/web-access.md) |
| Sign-in | Create the first user, then log in with a password or API key | None after the first run: opening the app signs you in as the local owner |
| Users | Many users, roles and permissions | One local owner (more people only through web access) |
| Configuration | `.vizier.yaml` seed file, CLI flags, environment variables, then the WebUI | The app's own **Settings → Desktop**, then the WebUI |
| Database | SQLite, or PostgreSQL + pgvector | SQLite |
| Files | Local disk, or S3-compatible object storage | Local disk |
| Discord / Telegram bots, scheduled tasks | Run as long as the server runs | Run only while the app is running (it can stay in the tray) |
| Updates | Re-run the installer, `cargo install`, or pull a new image | The app checks for releases and installs them itself |
| Projects | Use projects that exist; cannot create them | Create projects from a folder on your computer (experimental) |

## Which one should I use?

- **Run the server edition** if the agents should be always on, shared with other people, reachable from Discord, Telegram or your own software, or deployed on a host, in Docker or on Kubernetes.
- **Run the desktop app** if the agents are for you alone on your own machine, you want no setup beyond an installer, or you want agents that work in folders on your computer.

You can move between them: both use the same workspace layout, so the app can [open a server's workspace](../app/installation.md#opening-an-existing-workspace) (one at a time).

## Follow the right path

- **Server edition:** [Install](../server/installation.md) → [Quick Start](../server/quick-start.md) → [Your first agent](./first-agent.md)
- **Desktop app edition:** [Install](../app/installation.md) → [Quick Start](../app/quick-start.md) → [Your first agent](./first-agent.md)

Most of the rest of this book (providers, agents, memory, skills, channels, tools) applies to both. Pages that apply to one edition only say so at the top.

> [!NOTE]
> Features marked **experimental** (Projects, Fleet) are still being built. They work, but their behaviour and settings may change between releases.
