# Data Model: Windows Support for the Desktop App

No database or schema change. The only "data" are release artifacts and one manifest entry.

## Windows installer (release asset set)

| Field | Value |
|---|---|
| File | `Vizier_<version>_x64-setup.exe` |
| Update signature | `Vizier_<version>_x64-setup.exe.sig` (minisign, `TAURI_SIGNING_PRIVATE_KEY`) |
| Checksum | `Vizier_<version>_x64-setup.exe.sha256` |
| Architecture | x86_64 only |
| Install scope | current user; no admin |
| Authenticode | unsigned (until #132-style follow-up) |

Rules: produced only by the `windows-x64` leg of `build-app`; a real (non-dry-run) release is refused if the `.sig` is missing.

## Update manifest entry (`latest.json`)

| Key | Value |
|---|---|
| `platforms["windows-x86_64"].url` | `https://github.com/<repo>/releases/download/v<version>/Vizier_<version>_x64-setup.exe` |
| `platforms["windows-x86_64"].signature` | contents of the `.sig` file (empty on dry runs, as for other platforms) |

## Workspace (existing entity)

Unchanged layout. Default location on Windows: `%APPDATA%\rs.vizier.app\workspace`. Lock: `{workspace}\.runtime\vizier.lock`; keys: `{workspace}\.runtime\encryption.key`, `web-access.key`.

## State transitions

None new. Upgrade = installer run over an existing install (data directory untouched); uninstall removes the install directory only.
