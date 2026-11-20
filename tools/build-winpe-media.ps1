#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Builds a WinPE boot image with the imaging agent injected.

.DESCRIPTION
    Uses the Windows 11 ADK (24H2+) to create a WinPE working directory,
    inject the imaging agent binary, and optionally write to a USB drive
    or create an ISO.

.PARAMETER AgentPath
    Path to the compiled imaging-agent.exe binary.

.PARAMETER OutputPath
    Path for the output ISO file. Defaults to .\output\DeployStation-WinPE.iso

.PARAMETER UsbDriveLetter
    If specified, writes the WinPE media directly to this USB drive.
    WARNING: All data on the drive will be erased.

.PARAMETER AdkPath
    Path to the ADK installation. Defaults to the standard install location.

.EXAMPLE
    .\build-winpe-media.ps1 -AgentPath .\target\release\imaging-agent.exe

.EXAMPLE
    .\build-winpe-media.ps1 -AgentPath .\target\release\imaging-agent.exe -UsbDriveLetter E
#>

param(
    [Parameter(Mandatory = $true)]
    [string]$AgentPath,

    [string]$OutputPath = ".\output\DeployStation-WinPE.iso",

    [string]$UsbDriveLetter,

    [string]$AdkPath = "${env:ProgramFiles(x86)}\Windows Kits\10\Assessment and Deployment Kit",

    # Directory holding Mesa3D's x64 opengl32.dll + libgallium_wgl.dll (software
    # OpenGL). Required because WinPE has no GPU driver; the agent renders via
    # eframe/glow → OpenGL → llvmpipe (CPU). See tools/winpe-inject-deps.ps1.
    [string]$MesaDir = (Join-Path $PSScriptRoot 'mesa')
)

$ErrorActionPreference = "Stop"

# --- Validate prerequisites ---

$winpeAddon = Join-Path $AdkPath "Windows Preinstallation Environment"
$deployTools = Join-Path $AdkPath "Deployment Tools"

if (-not (Test-Path $deployTools)) {
    Write-Error "ADK Deployment Tools not found at $deployTools. Install the Windows 11 ADK (24H2+)."
    exit 1
}

if (-not (Test-Path $winpeAddon)) {
    Write-Error "WinPE add-on not found at $winpeAddon. Install the WinPE add-on for the Windows 11 ADK."
    exit 1
}

if (-not (Test-Path $AgentPath)) {
    Write-Error "Agent binary not found at $AgentPath. Build it first: cargo build --release -p imaging-agent"
    exit 1
}

Write-Host "=== DeployStation WinPE Media Builder ===" -ForegroundColor Cyan
Write-Host "ADK Path: $AdkPath"
Write-Host "Agent: $AgentPath"
Write-Host ""

# --- Create WinPE working directory ---

$workDir = Join-Path $env:TEMP "DeployStation-WinPE-$(Get-Date -Format 'yyyyMMdd-HHmmss')"
$mountDir = Join-Path $workDir "mount"

Write-Host "[1/6] Creating WinPE working directory..." -ForegroundColor Yellow

# Set ADK environment variables that copype.cmd expects
$env:WinPERoot = $winpeAddon
$env:DISMRoot = Join-Path $deployTools "amd64\DISM"
$env:OSCDImgRoot = Join-Path $deployTools "amd64\Oscdimg"
$env:BCDBootRoot = Join-Path $deployTools "amd64\BCDBoot"
$env:ImagingRoot = Join-Path $deployTools "amd64"
$env:Path = "$winpeAddon;$env:DISMRoot;$deployTools\amd64;$deployTools;$env:Path"

$copypeCmd = Join-Path $winpeAddon "copype.cmd"
if (-not (Test-Path $copypeCmd)) {
    Write-Error "copype.cmd not found at $copypeCmd"
    exit 1
}

Write-Host "  Using copype from: $copypeCmd"
& cmd.exe /c "`"$copypeCmd`" amd64 `"$workDir`"" 2>&1 | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }

if (-not (Test-Path (Join-Path $workDir "media"))) {
    Write-Error "copype failed to create WinPE directory."
    exit 1
}

# --- Mount boot.wim ---

Write-Host "[2/6] Mounting boot.wim..." -ForegroundColor Yellow

$bootWim = Join-Path $workDir "media\sources\boot.wim"

if (-not (Test-Path $mountDir)) {
    New-Item -ItemType Directory -Path $mountDir -Force | Out-Null
}

Mount-WindowsImage -ImagePath $bootWim -Index 1 -Path $mountDir

# --- Inject agent binary + its DLL closure + Mesa3D llvmpipe ---

Write-Host "[3/6] Injecting imaging agent, DLL closure, and Mesa3D..." -ForegroundColor Yellow
& "$PSScriptRoot\winpe-inject-deps.ps1" -MountDir $mountDir -AgentPath $AgentPath -MesaDir $MesaDir

# --- Create startnet.cmd ---

Write-Host "[4/6] Configuring startnet.cmd..." -ForegroundColor Yellow

$startnetPath = Join-Path $mountDir "Windows\System32\startnet.cmd"
$startnetContent = @"
@echo off
wpeinit
echo.
echo ============================================
echo   DeployStation - Windows Imaging Agent
echo ============================================
echo.

REM Force Mesa3D to use its llvmpipe software renderer directly; skips the failed
REM ZINK/Vulkan probe that prints a harmless but confusing error on startup.
set GALLIUM_DRIVER=llvmpipe

REM Map capture share if config exists
if exist X:\imaging\config.json (
    echo Loading configuration...
) else (
    echo No configuration found on USB.
)

echo Starting imaging agent...
imaging-agent.exe
"@

Set-Content -Path $startnetPath -Value $startnetContent -Encoding ASCII
Write-Host "  Updated startnet.cmd"

# --- Create config directory on media ---

$imagingDir = Join-Path $workDir "media\imaging"
New-Item -ItemType Directory -Path $imagingDir -Force | Out-Null
Write-Host "  Created media\imaging directory for config.json"

# --- Unmount boot.wim ---

Write-Host "[5/6] Unmounting and saving boot.wim..." -ForegroundColor Yellow

Dismount-WindowsImage -Path $mountDir -Save

# --- Create output ---

Write-Host "[6/6] Creating bootable media..." -ForegroundColor Yellow

if ($UsbDriveLetter) {
    # Write to USB
    Write-Host "  Writing to USB drive $UsbDriveLetter`:..." -ForegroundColor Red
    $makeMedia = Join-Path $winpeAddon "MakeWinPEMedia.cmd"
    & cmd.exe /c "`"$makeMedia`" /UFD `"$workDir`" $UsbDriveLetter`:" 2>&1 | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }
    Write-Host ""
    Write-Host "USB media created on drive $UsbDriveLetter`:!" -ForegroundColor Green
} else {
    # Create ISO
    $outputDir = Split-Path $OutputPath -Parent
    if (-not (Test-Path $outputDir)) {
        New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
    }

    $oscdimg = Join-Path $deployTools "amd64\Oscdimg\oscdimg.exe"
    $efisys = Join-Path $deployTools "amd64\Oscdimg\efisys.bin"
    $mediaDir = Join-Path $workDir "media"

    & $oscdimg -m -o -u2 -udfver102 "-bootdata:1#pEF,e,b$efisys" $mediaDir $OutputPath 2>&1 | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }

    if (Test-Path $OutputPath) {
        $size = (Get-Item $OutputPath).Length / 1MB
        Write-Host ""
        Write-Host "ISO created: $OutputPath ($([math]::Round($size, 1)) MB)" -ForegroundColor Green
    } else {
        Write-Error "Failed to create ISO."
    }
}

# --- Cleanup ---

Write-Host ""
Write-Host "Working directory: $workDir" -ForegroundColor Gray
Write-Host "You can delete it when done: Remove-Item -Recurse -Force '$workDir'" -ForegroundColor Gray
Write-Host ""
Write-Host "Done!" -ForegroundColor Cyan
