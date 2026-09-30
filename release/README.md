# Installer output (local builds)

This folder is filled when you run **`npm run installer`** or **`scripts\build-installer.ps1`**.

Each build produces a **versioned, build-stamped** setup file, for example:

`BackupGuard_0.1.0_20260930-211500_x64-setup.exe`

A copy named **`BackupGuard_latest_x64-setup.exe`** always points at the most recent build.

These `.exe` files are **not committed to git** (only this README is). Upload the setup exe to a GitHub Release if you want to distribute it without rebuilding.
