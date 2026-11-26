# Third-party notices

Guard Senpai is licensed under the MIT License (see [LICENSE](LICENSE)). The following third-party components are used in development and/or the distributed Windows application.

## JavaScript (runtime)

| Component | Licence (from package-lock.json) |
| --------- | -------------------------------- |
| React, react-dom, Vite frontend toolchain output | MIT |
| lucide-react | ISC |
| @tauri-apps/api | Apache-2.0 OR MIT |

## JavaScript (build / dev only)

| Component | Licence | Note |
| --------- | ------- | ---- |
| TypeScript | Apache-2.0 | Build-time only |
| @tauri-apps/cli | Apache-2.0 OR MIT | Installer build |
| Tailwind CSS, Vite, oxlint | MIT | Build-time |
| lightningcss (transitive) | MPL-2.0 | Tailwind/Vite CSS pipeline; MPL applies to lightningcss source if distributed separately |

## Rust (direct dependencies)

Direct crates include Tauri, rusqlite (bundled SQLite), serde, sha2, ureq, chrono, uuid, and windows-sys. Upstream licences are typically **MIT** and/or **Apache-2.0**. Run `cargo deny` on the build machine for a full transitive inventory (`Cargo.lock` does not embed SPDX fields).

## Bundled / platform components (installer)

| Component | Note |
| --------- | ---- |
| SQLite | Public domain (via rusqlite bundled) |
| Microsoft WebView2 Runtime | Installed via Microsoft bootstrapper when missing; subject to [Microsoft WebView2 licence terms](https://developer.microsoft.com/microsoft-edge/webview2/) |
| NSIS installer (Tauri bundle) | Per upstream NSIS and Tauri tooling licences |

## Attribution

QuickXor hashing implements Microsoft’s published QuickXorHash algorithm for comparison with OneDrive metadata; see `src-tauri/src/quickxor.rs`.