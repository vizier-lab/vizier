# 1.4 Desktop App

Vizier also ships as a desktop app. It runs the same engine as the server inside the app, shows the same WebUI in a window, and opens **no network port** unless you turn on [web access](../configuration/desktop-web-access.md).

## Download

Every [GitHub release](https://github.com/vizier-lab/vizier/releases/latest) carries:

| Platform | File |
|---|---|
| macOS 11+ (Apple Silicon and Intel) | `Vizier_<version>_universal.dmg` |
| Linux x86_64 | `Vizier_<version>_amd64.AppImage` |
| Linux arm64 | `Vizier_<version>_aarch64.AppImage` |
| Windows 10/11 x64 | `Vizier_<version>_x64-setup.exe` |

**Windows.** Run the installer. It installs for your user without asking for administrator rights, adds a Start menu entry, and installs the WebView2 runtime if your PC lacks it (Windows 11 has it). The installer is not code-signed yet, so SmartScreen may warn: choose **More info**, then **Run anyway**. Windows on ARM runs the x64 build under emulation. Uninstalling removes the app but keeps your workspace. To build it yourself, see [Building from Source](contributing.md#developing-the-desktop-app-on-windows).

**macOS.** The build is not signed or notarized yet. Open the `.dmg`, drag Vizier to Applications and open it once. macOS will refuse; open **System Settings → Privacy & Security** and choose **Open Anyway** next to the Vizier message. This is only needed the first time.

**Linux.** Make the file executable and run it:

```sh
chmod +x Vizier_<version>_amd64.AppImage
./Vizier_<version>_amd64.AppImage
```

The AppImage needs glibc 2.35 or newer (Ubuntu 22.04, Debian 12, Fedora 36 and later).

## First run

The first time you open the app it asks for a name and a password for the local owner (at least 8 characters). There is no login screen in the app afterwards: opening it is signing in as that owner. The password is for opening Vizier from a browser, if you turn web access on. If you forget it, change it in **Settings → Password** inside the app: the window is the owner of the workspace, so it does not ask for the old one. (A browser signed in over web access does.) The owner has the built-in system role, so everything the server's first user can do, you can do.

## Where your data lives

Your workspace is a folder, by default inside the app's data directory (`~/Library/Application Support/rs.vizier.app/workspace` on macOS, `~/.local/share/rs.vizier.app/workspace` on Linux, `%APPDATA%\rs.vizier.app\workspace` on Windows). It has the same layout as a server workspace. **Settings → Desktop** shows the current folder and lets you open another one.

## Opening an existing workspace

Choose **Open another workspace** in **Settings → Desktop** and pick a folder created by the server or by another copy of the app. If it has exactly one system user, the app signs in as that user; if several, it asks which one you are.

A workspace can be open in only one place at a time. If a server or another app already has it open, the app tells you so, names what holds it, and offers to open a different workspace. Stop the other one first to use this folder.

## Close to tray

Closing the window hides it and keeps your agents running; a tray icon brings it back (**Open Vizier**) or quits (**Quit**). Turn this off in **Settings → Desktop** to make closing the window quit the app. Scheduled tasks and channel bots (Discord, Telegram) only run while the app is running.

## Notifications and links

When the window is hidden or unfocused, notifications arrive as operating system notifications; selecting one opens the related screen. `vizier://` links open the app at that screen. Links to other websites open in your browser.

## Updates

The app checks for a new release when it starts (switch this off in **Settings → Desktop**, or use **Check for updates** at any time). When one is available, choose **Install and restart**. Only stable releases are offered.
