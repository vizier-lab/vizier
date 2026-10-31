# Contract: Release artifacts (Windows additions)

Extends `specs/038-desktop-app-edition/contracts/release-artifacts.md`. Existing rows are unchanged.

## Assets on the GitHub release

| Platform | File | Also |
|---|---|---|
| Windows 10/11 x64 | `Vizier_<version>_x64-setup.exe` | `.sig`, `.sha256` |

The server tarballs and their checksums are not affected; no Windows server asset is produced.

## `latest.json`

```json
{
  "version": "1.2.3",
  "notes": "…",
  "pub_date": "…",
  "platforms": {
    "darwin-aarch64": { "url": "…", "signature": "…" },
    "darwin-x86_64":  { "url": "…", "signature": "…" },
    "linux-x86_64":   { "url": "…", "signature": "…" },
    "linux-aarch64":  { "url": "…", "signature": "…" },
    "windows-x86_64": {
      "url": "https://github.com/<repo>/releases/download/v1.2.3/Vizier_1.2.3_x64-setup.exe",
      "signature": "<contents of Vizier_1.2.3_x64-setup.exe.sig>"
    }
  }
}
```

`buildManifest` in `scripts/app-latest-json.mjs` adds the `windows-x86_64` entry; its test asserts the five keys and the URL shape.

## Workflow behaviour

- `build-app` gains matrix entry `windows-x64` (`windows-latest`); artifact name `vizier-app-windows-x64`.
- A Windows build failure fails `build-app`, which `create-release` needs, so no release is published without it.
- "Check update signatures" (real releases only) requires `Vizier_<version>_x64-setup.exe.sig`.
- Dry run: builds and uploads the artifact, writes `latest.json` with an empty Windows signature, publishes nothing.
- Release notes list the Windows download and state it is not code-signed, with the SmartScreen steps (**More info → Run anyway**).

## Installer behaviour

- Installs for the current user (no UAC prompt), adds a Start menu entry, registers the `vizier://` scheme, and installs the WebView2 runtime if absent.
- Upgrade over an existing install keeps `%APPDATA%\rs.vizier.app` (workspace included).
- Uninstall removes the install directory and shortcuts; it does not delete the workspace.
