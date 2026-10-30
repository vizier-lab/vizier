# 2.17 Desktop Web Access

The [desktop app](../getting-started/desktop-app.md) opens no network port by default. **Web access** is an opt-in setting (**Settings → Desktop**) that also serves the WebUI and API from the app, so you can reach your agents from a browser, a phone or another computer.

## Turning it on

1. The owner is created with a password, so there is one to sign in with. (An owner from an older workspace may have none: set one in **Settings → Password**.) The switch refuses to turn on without one, because a browser has to sign in.
2. Turn on **Web access** and choose a port (default 9999).
3. Open the address shown (`http://127.0.0.1:<port>`) and sign in with the owner's username and password, exactly as on a server.

Turning it off closes the port and ends browser sessions.

## Who can reach it

| Setting | Reachable from |
|---|---|
| Web access on (default) | this computer only (`127.0.0.1`) |
| **Allow other devices on the network** | anything that can reach this computer's address |

Allowing the network binds every interface. Traffic is **plain HTTP**: passwords and chats are not encrypted in transit. Use it only on a network you trust, or put a reverse proxy with TLS in front.

## Security boundary

- The browser always signs in with a password, the same rules as the server edition. The window's automatic sign-in ("desktop identity") is never accepted over the network; a request that claims it is refused.
- Allowed browser origins (CORS) are limited to the address being served.
- Your operating system account is the boundary around the workspace: anyone who can run programs as you can read the workspace folder directly, and desktop access is the same as filesystem access. Encryption at rest ([2.14](./encryption.md)) protects secrets in the database from other people with a copy of the file, not from you.
- The signing key for browser sessions lives in `{workspace}/.runtime/web-access.key` and is created on first use.
