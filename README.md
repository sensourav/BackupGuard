# BackupGuard

Read-only desktop app for tracking local, external, and OneDrive backups: audits, bit-rot detection, and a 3-way discrepancy matrix. Built with Tauri 2, React, and SQLite.

## Requirements checklist

| # | What you asked for | Status |
|---|-------------------|--------|
| 1 | After each build, a **build-specific** `.exe` installer in the repo tree | **Yes** — `npm run installer` writes `release/BackupGuard_<version>_<timestamp>_x64-setup.exe` plus `release/BackupGuard_latest_x64-setup.exe` and `release/latest-build.json`. Installers are gitignored (not pushed as blobs); see [release/README.md](release/README.md). |
| 2 | Install to a folder, **uninstall.exe** in that folder, **registry** entry, uninstall removes install dir + registry | **Yes** — Tauri NSIS (per-user install under `%LOCALAPPDATA%\Programs\...`) ships `uninstall.exe` beside the app and registers **Settings → Apps**. Uninstall removes the program files and the uninstall registry key. It does **not** delete your backup folders or `%APPDATA%\com.backupguard.app\` unless you opt in (see below). |
| 3 | **Script in the repo** to build the installer on demand | **Yes** — [scripts/build-installer.ps1](scripts/build-installer.ps1) / [scripts/build-installer.cmd](scripts/build-installer.cmd), or `npm run installer`. |
| 4 | **README** install + build instructions | **This file** |
| 5 | Installer installs **Rust / Node / build tools** on the target PC | **No (by design)** — end users only need the setup exe. The app is already compiled inside it. The installer **can** install **Microsoft Edge WebView2** if missing (`webviewInstallMode: downloadBootstrapper` in [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json)). Rust, Node, and MSVC are **only** required on the machine that **builds** the installer, not on machines that **run** BackupGuard. Bundling Rust via NSIS would be hundreds of MB and is not how desktop apps are distributed. |

---

## Install BackupGuard (end user)

1. Obtain **`release/BackupGuard_latest_x64-setup.exe`** after someone builds the repo, or download a release build from GitHub Releases.
2. Double-click the setup exe and complete the wizard.
3. If WebView2 is missing, the installer will download/run Microsoft’s WebView2 bootstrapper (one-time).
4. Launch **BackupGuard** from the Start menu.

Default install location (per-user, no admin):

`%LOCALAPPDATA%\Programs\BackupGuard\`

That folder contains `backupguard.exe` and **`uninstall.exe`**.

### OneDrive (optional)

Copy [onedrive.client.example.json](onedrive.client.example.json) to `onedrive.client.local.json` next to the app or under `%APPDATA%\com.backupguard.app\`, set your Azure **clientId**, and register redirect `http://127.0.0.1:43821/callback`.

### Uninstall

**Windows (normal):**

- **Settings → Apps → Installed apps → BackupGuard → Uninstall**, or  
- Run **`uninstall.exe`** in the install folder, or  
- Start menu → **Uninstall BackupGuard**

This removes the install directory and the Windows uninstall registry entry.

**From a dev clone (after you installed via setup):**

```powershell
npm run uninstall
# or: .\scripts\uninstall.ps1
```

Add **`-RemoveAppData`** to also delete `%APPDATA%\com.backupguard.app\` (SQLite DB and encrypted OneDrive tokens). Your **watched backup files on disk are never deleted** by BackupGuard.

---

## Create the installer (developer / build machine)

**On the PC that builds** (not on every end-user PC), install once:

- [Node.js](https://nodejs.org/) (LTS)
- [Rust](https://rustup.rs/) with the **MSVC** toolchain (`rustup default stable` and Visual Studio Build Tools with C++)
- WebView2 (usually already on Windows 10/11)

Then from the repo root:

```powershell
git clone https://github.com/sensourav/BackupGuard.git
cd BackupGuard
npm run installer
```

Or double-click **`scripts\build-installer.cmd`**.

**Outputs:**

| File | Purpose |
|------|---------|
| `release/BackupGuard_<version>_<yyyyMMdd-HHmmss>_x64-setup.exe` | This build’s installer |
| `release/BackupGuard_latest_x64-setup.exe` | Same as the last build |
| `release/latest-build.json` | Version, stamp, file names |
| Cargo target `...\release\bundle\nsis\` | Tauri’s raw bundle output |

Skip `npm install` on repeat builds:

```powershell
.\scripts\build-installer.ps1 -SkipNpmInstall
```

---

## Development (no installer)

```powershell
npm install
npm run tauri dev
```

## License

Private / all rights reserved unless you add a license file.
