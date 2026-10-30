# Contract: App release artifacts

Published on the project's GitHub release for each version, next to the unchanged server
assets. Nowhere else (spec FR-021).

| Asset | Platform | Installs on |
|---|---|---|
| `Vizier_<version>_universal.dmg` | macOS | Apple Silicon and Intel, macOS 11.0+ |
| `Vizier_universal.app.tar.gz` + `.sig` | macOS (updater payload) | same |
| `Vizier_<version>_amd64.AppImage` + `.sig` | Linux x86_64 | glibc ≥ 2.35 (Ubuntu 22.04, Debian 12, Fedora 36 and later) |
| `Vizier_<version>_aarch64.AppImage` + `.sig` | Linux arm64 | glibc ≥ 2.35 |
| `latest.json` | updater manifest | — |
| `*.sha256` | checksums for the `.dmg` and AppImages | — |

## `latest.json`

```json
{
  "version": "<version>",
  "notes": "<changelog section>",
  "pub_date": "<rfc3339>",
  "platforms": {
    "darwin-aarch64": { "url": ".../Vizier_universal.app.tar.gz", "signature": "<sig>" },
    "darwin-x86_64":  { "url": ".../Vizier_universal.app.tar.gz", "signature": "<sig>" },
    "linux-x86_64":   { "url": ".../Vizier_<version>_amd64.AppImage", "signature": "<sig>" },
    "linux-aarch64":  { "url": ".../Vizier_<version>_aarch64.AppImage", "signature": "<sig>" }
  }
}
```

Updater endpoint compiled into the app:
`https://github.com/vizier-lab/vizier/releases/latest/download/latest.json`. Prereleases are
never `latest`, so installed apps only move between stable releases.

## Invariants checked in the release job

- `lipo -archs` of the universal binary prints `x86_64 arm64`.
- `file` on each AppImage reports `x86-64` / `ARM aarch64`.
- Every `latest.json` URL resolves to an asset in the same release, and each signature
  verifies against the public key compiled into the app.
- The server's assets (`vizier-v<version>-<target>.tar.gz` + `.sha256` for the four targets),
  the Docker image and `install.sh` are byte-for-byte produced by the unchanged server job.
- Release notes gain an "Desktop app" section listing the three downloads.
