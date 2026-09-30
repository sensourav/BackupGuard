# Builds the Windows NSIS installer (setup.exe). The setup registers BackupGuard in
# Windows "Apps & features", installs under %LOCALAPPDATA%\Programs\..., and places
# uninstall.exe in that install folder. Use scripts/uninstall.ps1 to run uninstall.
param(
    [switch]$SkipNpmInstall
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

function Get-AppVersion {
    $package = Get-Content (Join-Path $repoRoot "package.json") -Raw | ConvertFrom-Json
    return $package.version
}

function Get-BuildStamp {
    return (Get-Date -Format "yyyyMMdd-HHmmss")
}

Write-Host "BackupGuard — building Windows installer..." -ForegroundColor Cyan

if (-not $SkipNpmInstall) {
    Write-Host "npm install"
    npm install
    if ($LASTEXITCODE -ne 0) {
        throw "npm install failed with exit code $LASTEXITCODE"
    }
}

Write-Host "npm run build:installer (tauri build)"
npm run build:installer
if ($LASTEXITCODE -ne 0) {
    throw "tauri build failed with exit code $LASTEXITCODE"
}

$metadata = cargo metadata --format-version 1 --manifest-path (Join-Path $repoRoot "src-tauri\Cargo.toml") | ConvertFrom-Json
$targetDir = $metadata.target_directory
$version = Get-AppVersion
$stamp = Get-BuildStamp
$bundleDir = Join-Path $targetDir "release\bundle\nsis"
$tauriSetupName = "BackupGuard_${version}_x64-setup.exe"
$builtSetup = Join-Path $bundleDir $tauriSetupName

if (-not (Test-Path $builtSetup)) {
    $fallback = Get-ChildItem -Path $bundleDir -Filter "BackupGuard_*_x64-setup.exe" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if ($fallback) {
        $builtSetup = $fallback.FullName
    } else {
        throw "Installer not found under $bundleDir"
    }
}

$releaseDir = Join-Path $repoRoot "release"
New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null

$stampedName = "BackupGuard_${version}_${stamp}_x64-setup.exe"
$stampedCopy = Join-Path $releaseDir $stampedName
$latestCopy = Join-Path $releaseDir "BackupGuard_latest_x64-setup.exe"
$manifestPath = Join-Path $releaseDir "latest-build.json"

Copy-Item -Path $builtSetup -Destination $stampedCopy -Force
Copy-Item -Path $builtSetup -Destination $latestCopy -Force

$manifest = @{
    version = $version
    buildStamp = $stamp
    setupExe = $stampedName
    latestExe = "BackupGuard_latest_x64-setup.exe"
    builtAt = (Get-Date).ToUniversalTime().ToString("o")
    cargoBundle = $builtSetup
} | ConvertTo-Json -Depth 4
Set-Content -Path $manifestPath -Value $manifest -Encoding utf8

$portableExe = Join-Path $targetDir "release\backupguard.exe"

Write-Host ""
Write-Host "Done." -ForegroundColor Green
Write-Host "  Build-stamped: $stampedCopy"
Write-Host "  Latest copy:   $latestCopy"
Write-Host "  Manifest:      $manifestPath"
Write-Host "  Cargo bundle:  $builtSetup"
if (Test-Path $portableExe) {
    Write-Host "  Portable exe:  $portableExe"
}
Write-Host ""
Write-Host "Install by running the setup exe. Uninstall via Windows Settings or scripts\uninstall.ps1"
