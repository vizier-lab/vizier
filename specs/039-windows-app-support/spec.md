# Feature Specification: Windows Support for the Desktop App

**Feature Branch**: `039-windows-app-support`

**Created**: 2026-10-31

**Status**: Draft

**Input**: User description: "let's add windows support only to the app edition, make the github workflow publish an installer, and update the justfile to support development in windows"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Install and use Vizier on Windows (Priority: P1)

A Windows user downloads a single installer from the project's release page, runs it, and gets the Vizier desktop app with a Start menu entry. Launching it opens the same window and workflow that macOS and Linux users get: sign-in-free first run, creating agents, chatting, and agent memory kept on disk in the user's workspace.

**Why this priority**: Without a working, installable Windows app there is nothing else to deliver; this is the whole point of the feature.

**Independent Test**: On a clean Windows 11 machine, download the installer from a release, install it, launch the app, create an agent on the offline test provider, send a message and receive a reply.

**Acceptance Scenarios**:

1. **Given** a clean Windows machine with no Vizier installed, **When** the user runs the installer, **Then** the app is installed, appears in the Start menu, and launches to the first-run screen.
2. **Given** the app is installed, **When** the user creates an agent and chats with it, **Then** replies stream, history persists, and the agent's memory is stored as readable documents in the workspace folder.
3. **Given** the app is running, **When** the user closes it and reopens it, **Then** their agents, conversations and settings are intact.
4. **Given** a second copy of the app (or another process) already holds the same workspace, **When** the app starts, **Then** it shows the existing "workspace locked" state rather than corrupting data.

---

### User Story 2 - Release publishes a Windows installer automatically (Priority: P1)

When a release is cut, the release pipeline builds the Windows app alongside the macOS and Linux apps and attaches the installer to the release, with the checksum/signature artifacts and update manifest entries the other platforms have.

**Why this priority**: Users can only get the Windows app through the release; without it story 1 is unreachable for anyone but developers.

**Independent Test**: Run the release pipeline in dry-run mode and confirm a Windows installer artifact is produced; for a real release confirm it is attached to the release page with the release notes naming it.

**Acceptance Scenarios**:

1. **Given** a release is triggered, **When** the pipeline finishes, **Then** the release page lists a Windows installer named consistently with the existing app downloads (product name, version, architecture).
2. **Given** the Windows build fails, **When** the pipeline runs, **Then** the failure is visible and does not silently ship a release that claims Windows support.
3. **Given** an installed Windows app and a newer release, **When** the app checks for updates, **Then** it finds the newer Windows installer through the same update manifest as other platforms.
4. **Given** the existing macOS, Linux and server artifacts, **When** the Windows job is added, **Then** those artifacts are produced exactly as before.

---

### User Story 3 - Develop the app on Windows with `just` (Priority: P2)

A contributor on Windows can set up and work on the desktop app using the same `just` recipes (`install-app`, `app-dev`, `app-build`, `app-test`) that Linux and macOS contributors use, without needing WSL or a Unix shell for the app workflow.

**Why this priority**: It enables contributing and verifying the Windows build, but users can be served without it.

**Independent Test**: On a Windows machine with the documented prerequisites, run the setup recipe, then the dev recipe, and see the app window open with hot reload; run the build recipe and get an installer.

**Acceptance Scenarios**:

1. **Given** a Windows machine, **When** the contributor runs the app setup recipe, **Then** it installs the project-level dependencies (the Tauri CLI, crates, submodules and npm packages), with the system prerequisites (C++ build tools, CMake, LLVM, WebView2) listed in the README and documentation.
2. **Given** setup is complete, **When** the contributor runs the dev recipe, **Then** the app opens in a window with live reload.
3. **Given** setup is complete, **When** the contributor runs the build recipe, **Then** a Windows installer is produced locally.
4. **Given** a Linux or macOS contributor, **When** they run the same recipes, **Then** behaviour is unchanged.

---

### Edge Cases

- Install paths or usernames containing spaces or non-ASCII characters must not break the app or its workspace.
- Windows file-path conventions (drive letters, backslashes, case-insensitive names, reserved names) must not break memory documents, artifacts, uploads or the workspace lock.
- A workspace on a synced or network folder: the lock and database behave safely or the user is warned.
- The system web view component required by the app is missing or outdated: the installer ensures it is present or the user gets an actionable message.
- Antivirus or SmartScreen warnings on an unsigned installer: the release notes explain how to proceed, as they do for unsigned macOS builds.
- Installing over an older version keeps the user's workspace and data.
- Uninstalling removes the app but leaves the user's workspace unless they explicitly choose otherwise.
- ARM64 Windows hardware: behaviour must be stated (supported or not) rather than left to fail.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The desktop app MUST run on Windows 10/11 with feature parity to the macOS and Linux app for chat, agents, memory, artifacts, scheduling and settings.
- **FR-002**: Windows support MUST be limited to the desktop app edition; the server binary, its release tarballs, container images and their CI MUST remain unchanged and gain no Windows target.
- **FR-003**: The release pipeline MUST build a Windows installer for the app and attach it to the release, named consistently with the other app downloads.
- **FR-004**: The Windows installer MUST install the app for the current user without requiring administrator rights where possible, create a Start menu entry, and support uninstalling.
- **FR-005**: The installer MUST preserve existing user data on upgrade and on uninstall by default.
- **FR-006**: The Windows build MUST participate in the app auto-update flow: its update payload is signed and listed in the update manifest.
- **FR-007**: The release notes MUST list the Windows download and state that the installer is unsigned (if so) with the steps to get past the SmartScreen warning.
- **FR-008**: A failing Windows app build MUST fail the release job visibly; it MUST NOT be skipped silently.
- **FR-009**: The `just` recipes for the app (setup, dev, build, test) MUST work from a native Windows shell without WSL, while behaving unchanged on Linux and macOS.
- **FR-010**: The system prerequisites for building on Windows (Rust MSVC toolchain, C++ build tools, CMake, LLVM/libclang, Node.js, WebView2) MUST be listed with install commands in the README and the documentation; the `just` recipes do not install or check them.
- **FR-011**: The app MUST resolve its default workspace and runtime files using Windows conventions and MUST keep the single-engine-per-workspace guarantee on Windows.
- **FR-012**: Features that depend on Unix-only facilities (for example local shell execution backends or file permission modes) MUST degrade gracefully on Windows with a clear message, not crash.
- **FR-013**: The README and the documentation MUST describe how to install the app on Windows and how to set up development on Windows, including FR-010's prerequisites.

### Key Entities

- **Windows installer**: The downloadable release artifact for Windows users; has a version, architecture, and a matching signed update payload.
- **Update manifest entry (Windows)**: The record that lets installed Windows apps discover and verify newer versions.
- **Workspace**: The user's data folder, now resolved with Windows conventions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A new Windows user can go from the release page to a working chat with an agent in under 10 minutes.
- **SC-002**: Every release produced after this change includes a Windows installer, in 100% of non-failed releases.
- **SC-003**: A contributor on Windows can go from a fresh checkout with documented prerequisites to a running dev app using only `just` recipes, in under 30 minutes of active effort.
- **SC-004**: 100% of existing macOS, Linux and server release artifacts are unchanged in name and content.
- **SC-005**: Upgrading from one Windows release to the next preserves all agents, conversations and memory in every tested upgrade.
- **SC-006**: An installed Windows app detects and applies a newer release through auto-update in a tested upgrade.

## Assumptions

- Target is 64-bit x86 Windows 10 and 11; Windows on ARM is out of scope for this feature unless it comes free with the build.
- The installer is unsigned initially, mirroring the macOS situation tracked in #132; code signing is a separate follow-up.
- Existing update signing keys and the update manifest mechanism are reused for Windows.
- The server edition is not supported on Windows; it remains Linux/macOS/Docker.
- Contributors on Windows install the system prerequisites themselves, following the README and documentation; the setup recipe only handles project-level dependencies (such as the Tauri CLI).
- The `just` recipes for the app can use a Windows-native shell where the existing Bash-based recipes cannot run there.
