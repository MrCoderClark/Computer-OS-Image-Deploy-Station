<#
.SYNOPSIS
    Install DeployStation imaging-server as a Windows Service.

.DESCRIPTION
    Copies imaging-server.exe to %ProgramFiles%\DeployStation\,
    creates %ProgramData%\DeployStation\ for config + db + logs,
    seeds server.toml if absent, and registers the service.
    Idempotent -- safe to re-run.

.PARAMETER ExePath
    Path to a built imaging-server.exe. Defaults to the one under
    target\release\ next to the repo root.

.EXAMPLE
    .\tools\install-server-service.ps1
#>
[CmdletBinding()]
param(
    [string]$ExePath
)

$ErrorActionPreference = 'Stop'

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "This script must run from an elevated PowerShell (service install requires admin)."
}

$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $ExePath) {
    $ExePath = Join-Path $repoRoot 'target\release\imaging-server.exe'
}
if (-not (Test-Path $ExePath)) {
    throw "imaging-server.exe not found at $ExePath. Build it first: cargo build --release -p imaging-server"
}

$installDir = Join-Path $env:ProgramFiles 'DeployStation'
$dataDir    = Join-Path $env:ProgramData 'DeployStation'
$cfgPath    = Join-Path $dataDir 'server.toml'
$dstExe     = Join-Path $installDir 'imaging-server.exe'

New-Item -ItemType Directory -Path $installDir -Force | Out-Null
New-Item -ItemType Directory -Path $dataDir    -Force | Out-Null

Write-Host "Copying $ExePath -> $dstExe" -ForegroundColor Cyan
Copy-Item -Path $ExePath -Destination $dstExe -Force

if (-not (Test-Path $cfgPath)) {
    Write-Host "Writing default $cfgPath" -ForegroundColor Cyan
    $tomlSample = Join-Path $repoRoot 'crates\server\server.sample.toml'
    if (Test-Path $tomlSample) {
        Copy-Item -Path $tomlSample -Destination $cfgPath
    } else {
        @"
bind_addr = "0.0.0.0:7443"
db_path = "imaging-server.db"
log_filter = "imaging_server=info,tower_http=info"
log_dir = "logs"
"@ | Set-Content -Path $cfgPath -Encoding UTF8
    }
} else {
    Write-Host "Keeping existing $cfgPath" -ForegroundColor DarkGray
}

Write-Host "Registering service (DeployStationServer)" -ForegroundColor Cyan
& $dstExe install
if ($LASTEXITCODE -ne 0) {
    throw "Service install failed (exit code $LASTEXITCODE)."
}

Write-Host "Starting service" -ForegroundColor Cyan
Start-Service -Name DeployStationServer
Get-Service -Name DeployStationServer

Write-Host ""
Write-Host "Installed. Config: $cfgPath" -ForegroundColor Green
Write-Host "Logs:    $dataDir\logs\"   -ForegroundColor Green
Write-Host "Data:    $dataDir\imaging-server.db" -ForegroundColor Green
