# Security policy

## Supported versions

| Version | Supported |
| ------- | --------- |
| 0.1.x   | Yes       |

## Reporting a vulnerability

Open a **private security advisory** on GitHub for [sensourav/BackupGuard](https://github.com/sensourav/BackupGuard) or email the maintainer through GitHub profile contact. Do not file public issues for undisclosed vulnerabilities.

## Design notes

BackupGuard is **read-only**: it does not modify watched backup folders. OneDrive uses OAuth with PKCE; access and refresh tokens are stored as **DPAPI-protected blobs** in the local SQLite database under `%APPDATA%\com.backupguard.app\`. The Azure application **client ID** is not a secret, but it belongs in `onedrive.client.local.json` on the user machine—**never commit** that file or any `.env` containing secrets.

For automated builds in the future, store signing keys and Azure credentials only in **GitHub Actions secrets**, not in the repository.