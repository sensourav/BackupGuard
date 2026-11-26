# Guard Senpai

Read-only Windows desktop app for backup auditing: compare local disks, external volumes, and OneDrive metadata; detect bit-rot and surface a three-way discrepancy matrix. Built with **Tauri 2**, **React**, and **SQLite**.

## Features

- **Watched folder sets** — local path, external path, and optional OneDrive folder per set
- **Audits** — full tree, single date folder (`YYYY-MM-DD`), or single file scope
- **Integrity** — SHA-256 on local/external files; QuickXorHash alignment with OneDrive when connected
- **Health states** — healthy, missing, corrupt, bit-rot (with plain-language notes)
- **Installer** — per-user NSIS setup with optional WebView2 bootstrapper (see [release/README.md](release/README.md))

Guard Senpai does **not** write, delete, or rename files in your backup locations.

## Install (end user)

1. Obtain **`release/GuardSenpai_latest_x64-setup.exe`** from a maintainer build or a GitHub Release.
2. Run the setup wizard (WebView2 is installed automatically if missing).
3. Launch **Guard Senpai** from the Start menu.

Default install location: `%LOCALAPPDATA%\Programs\Guard Senpai\` (`guardsenpai.exe` and `uninstall.exe`).

### OneDrive (optional)

1. Register a public Azure app with redirect URI `http://127.0.0.1:43821/callback`.
2. Copy [onedrive.client.example.json](onedrive.client.example.json) to `onedrive.client.local.json` in `%APPDATA%\com.guardsenpai.app\` (or next to the app when developing).
3. Set your Azure **application (client) ID** — this is a public identifier, not a client secret. **Do not commit** `onedrive.client.local.json`.

OAuth scopes used at runtime: `openid`, `profile`, `offline_access`, `User.Read`, and **`Files.Read`** (metadata only; file contents are not downloaded).

### Uninstall

- **Settings → Apps → Guard Senpai → Uninstall**, or run `uninstall.exe` in the install folder.
- From a dev clone after installing via setup: `npm run uninstall` (add **`-RemoveAppData`** to remove `%APPDATA%\com.guardsenpai.app\` and encrypted tokens). Watched backup folders on disk are never deleted by Guard Senpai.

## Build the installer (developer machine)

Requires [Node.js](https://nodejs.org/) (LTS), [Rust](https://rustup.rs/) with the MSVC toolchain, and Windows build tools.

```powershell
git clone https://github.com/sensourav/guardsenpai.git
cd guardsenpai
npm run installer
```

Or run `scripts\build-installer.cmd`. Outputs land under `release/` (installers are gitignored). Repeat builds: `.\scripts\build-installer.ps1 -SkipNpmInstall`.

## Development

```powershell
npm install
npm run tauri dev
```

## Security

See [SECURITY.md](SECURITY.md). Report vulnerabilities through GitHub private security advisories.

## Licence

MIT — see [LICENSE](LICENSE). Third-party components are listed in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).