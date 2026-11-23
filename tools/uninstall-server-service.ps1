<#
.SYNOPSIS
    Stop and remove the DeployStation imaging-server Windows Service.

.DESCRIPTION
    Removes the service registration and the installed exe. Leaves
    %ProgramData%\DeployStation\ (config, db, logs) in place unless -Purge
    is passed.

.PARAMETER Purge
    Also delete %ProgramData%\DeployStation\ (DESTROYS the database).
#>
[CmdletBinding()]
param(
    [switch]$Purge
)

$ErrorActionPreference = 'Stop'

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "This script must run from an elevated PowerShell."
}

$installDir = Join-Path $env:ProgramFiles 'DeployStation'
$dataDir    = Join-Path $env:ProgramData 'DeployStation'
$exe        = Join-Path $installDir 'imaging-server.exe'

$svc = Get-Service -Name DeployStationServer -ErrorAction SilentlyContinue
if ($svc) {
    if ($svc.Status -ne 'Stopped') {
        Write-Host "Stopping DeployStationServer" -ForegroundColor Cyan
        Stop-Service -Name DeployStationServer -Force
    }
    if (Test-Path $exe) {
        Write-Host "Uninstalling service" -ForegroundColor Cyan
        & $exe uninstall
    } else {
        sc.exe delete DeployStationServer | Out-Null
    }
} else {
    Write-Host "Service not registered; nothing to uninstall." -ForegroundColor DarkGray
}

if (Test-Path $installDir) {
    Write-Host "Removing $installDir" -ForegroundColor Cyan
    Remove-Item -Path $installDir -Recurse -Force
}

if ($Purge -and (Test-Path $dataDir)) {
    Write-Host "Purging $dataDir (database and logs)" -ForegroundColor Yellow
    Remove-Item -Path $dataDir -Recurse -Force
} elseif (Test-Path $dataDir) {
    Write-Host "Kept $dataDir (config, db, logs). Pass -Purge to remove." -ForegroundColor DarkGray
}

Write-Host "Uninstalled." -ForegroundColor Green
