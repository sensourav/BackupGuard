# Runs Guard Senpai's registered Windows uninstaller (uninstall.exe in the install folder).
# Removes the program directory and the uninstall registry entry. Does not delete
# watched backup paths or %APPDATA%\com.guardsenpai.app\ unless you remove that yourself.
param(
    [switch]$Quiet,
    [switch]$RemoveAppData
)

$ErrorActionPreference = "Stop"

function Find-GuardsenpaiUninstallEntry {
    $roots = @(
        "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*",
        "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*"
    )
    foreach ($root in $roots) {
        $items = Get-ItemProperty $root -ErrorAction SilentlyContinue
        foreach ($item in $items) {
            if (-not $item.DisplayName) {
                continue
            }
            if ($item.DisplayName -match "Guard Senpai") {
                return $item
            }
        }
    }
    return $null
}

$entry = Find-GuardsenpaiUninstallEntry
if (-not $entry) {
    Write-Error "Guard Senpai is not installed (no entry under Windows Uninstall registry)."
    exit 1
}

$uninstall = $entry.UninstallString
if (-not $uninstall) {
    Write-Error "Found Guard Senpai in the registry but UninstallString is missing."
    exit 1
}

$installLocation = $entry.InstallLocation
Write-Host "Display name:    $($entry.DisplayName)"
Write-Host "Install folder:  $installLocation"
Write-Host "Uninstall:       $uninstall"

if (-not $Quiet) {
    $answer = Read-Host "Run the Windows uninstaller now? [y/N]"
    if ($answer -notmatch "^[yY]") {
        Write-Host "Cancelled."
        exit 0
    }
}

if ($uninstall -match '^"(?<exe>[^"]+)"(?<args>.*)$') {
    $exe = $Matches["exe"]
    $args = $Matches["args"].Trim()
} elseif ($uninstall -match "^(?<exe>\S+)(?<args>.*)$") {
    $exe = $Matches["exe"]
    $args = $Matches["args"].Trim()
} else {
    throw "Could not parse UninstallString: $uninstall"
}

if ($Quiet -and $args -notmatch "/S") {
    $args = "$args /S".Trim()
}

Write-Host "Starting uninstaller..."
$process = Start-Process -FilePath $exe -ArgumentList $args -Wait -PassThru
if ($process.ExitCode -ne 0) {
    Write-Error "Uninstaller exited with code $($process.ExitCode)"
    exit $process.ExitCode
}

if ($RemoveAppData) {
    $appData = Join-Path $env:APPDATA "com.guardsenpai.app"
    if (Test-Path $appData) {
        Write-Host "Removing app data: $appData"
        Remove-Item -Path $appData -Recurse -Force
    }
}

Write-Host "Guard Senpai uninstall finished." -ForegroundColor Green
Write-Host "Install folder and uninstall registry entry should be removed by the NSIS uninstaller."
if (-not $RemoveAppData) {
    Write-Host "App database/tokens (if any) remain at:"
    Write-Host "  $env:APPDATA\com.guardsenpai.app"
    Write-Host "Pass -RemoveAppData to delete that folder too."
}
