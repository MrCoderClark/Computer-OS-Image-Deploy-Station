<#
.SYNOPSIS
    Installs all developer prerequisites for DeployStation.

.DESCRIPTION
    One-shot bootstrap so a fresh clone can go from zero to `cargo tauri dev`
    without hunting for install commands. Idempotent -- safe to re-run; each
    step skips when already satisfied.

    What it installs/checks:
      1. Rust toolchain                     (via rustup -- must be installed)
      2. cargo-tauri CLI                    (via `cargo install`)
      3. Windows 11 ADK + WinPE add-on      (manual, prints download links)
      4. Mesa3D llvmpipe (tools/mesa/)      (auto-downloads from GitHub)

.EXAMPLE
    .\tools\setup-dev.ps1
#>

[CmdletBinding()]
param(
    [switch]$Force  # Reinstall things that already look present
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot

function Test-Cmd { param([string]$Name) [bool](Get-Command $Name -ErrorAction SilentlyContinue) }

Write-Host "=== DeployStation developer setup ===" -ForegroundColor Cyan
Write-Host "Repo: $repoRoot"
Write-Host ""

# --- 1. Rust toolchain ---
Write-Host "[1/4] Rust toolchain..." -ForegroundColor Yellow
if (-not (Test-Cmd 'cargo')) {
    Write-Host "  cargo not found on PATH." -ForegroundColor Red
    Write-Host "  Install Rust first: https://rustup.rs" -ForegroundColor Yellow
    Write-Host "  Then rerun this script." -ForegroundColor Yellow
    exit 1
}
$rustV = (& rustc --version) 2>&1
Write-Host "  $rustV" -ForegroundColor Green

# --- 2. cargo-tauri ---
Write-Host "[2/4] cargo-tauri CLI..." -ForegroundColor Yellow
$needTauri = $Force -or -not (Test-Cmd 'cargo-tauri')
if ($needTauri) {
    Write-Host "  Installing cargo-tauri (this takes a few minutes)..." -ForegroundColor Cyan
    & cargo install tauri-cli --version "^2.0" --locked
    if ($LASTEXITCODE -ne 0) { throw "cargo install tauri-cli failed" }
    Write-Host "  cargo-tauri installed" -ForegroundColor Green
} else {
    $tV = (& cargo tauri --version) 2>&1
    Write-Host "  Already installed: $tV" -ForegroundColor Green
}

# --- 3. Windows 11 ADK + WinPE add-on ---
Write-Host "[3/4] Windows 11 ADK + WinPE add-on..." -ForegroundColor Yellow
$pf86 = [Environment]::GetEnvironmentVariable('ProgramFiles(x86)')
$adkBase = Join-Path $pf86 "Windows Kits\10\Assessment and Deployment Kit"
$adkOK   = Test-Path "$adkBase\Deployment Tools"
$peOK    = Test-Path "$adkBase\Windows Preinstallation Environment"
if ($adkOK -and $peOK) {
    Write-Host "  ADK + WinPE found at $adkBase" -ForegroundColor Green
} else {
    Write-Host "  MANUAL INSTALL REQUIRED (24H2 or newer):" -ForegroundColor Red
    if (-not $adkOK) {
        Write-Host "    ADK:        https://learn.microsoft.com/windows-hardware/get-started/adk-install" -ForegroundColor Yellow
    }
    if (-not $peOK) {
        Write-Host "    WinPE addon: same page, download 'Windows PE add-on for the ADK'" -ForegroundColor Yellow
    }
    Write-Host "  (Not a hard fail -- only needed when building WinPE media.)" -ForegroundColor DarkGray
}

# --- 4. Mesa3D llvmpipe ---
Write-Host "[4/4] Mesa3D llvmpipe (tools/mesa/)..." -ForegroundColor Yellow
$mesaDir = Join-Path $repoRoot "tools\mesa"
$mesaOGL = Join-Path $mesaDir "opengl32.dll"
$mesaLib = Join-Path $mesaDir "libgallium_wgl.dll"

if (-not $Force -and (Test-Path $mesaOGL) -and (Test-Path $mesaLib)) {
    Write-Host "  Already present at $mesaDir" -ForegroundColor Green
} else {
    New-Item -ItemType Directory -Path $mesaDir -Force | Out-Null

    Write-Host "  Fetching latest Mesa release metadata..." -ForegroundColor Cyan
    $release = Invoke-RestMethod -Uri 'https://api.github.com/repos/pal1000/mesa-dist-win/releases/latest' -UseBasicParsing
    $asset = $release.assets | Where-Object { $_.name -match 'release-msvc\.7z$' } | Select-Object -First 1
    if (-not $asset) { throw "Could not find release-msvc.7z asset in latest Mesa release" }
    Write-Host "  Mesa version: $($release.tag_name)" -ForegroundColor Green

    $sevenZip = @(
        "$env:ProgramFiles\7-Zip\7z.exe",
        (Join-Path $pf86 "7-Zip\7z.exe")
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $sevenZip) { throw "7-Zip not found. Install it: https://www.7-zip.org/" }

    $tmp = Join-Path $env:TEMP "deploystation-mesa"
    New-Item -ItemType Directory -Path $tmp -Force | Out-Null
    $archive = Join-Path $tmp $asset.name
    $extract = Join-Path $tmp "extract"
    if (Test-Path $extract) { Remove-Item $extract -Recurse -Force }

    Write-Host "  Downloading $($asset.name) ($([Math]::Round($asset.size/1MB,1)) MB)..." -ForegroundColor Cyan
    $pp = $ProgressPreference; $ProgressPreference = 'SilentlyContinue'
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $archive -UseBasicParsing
    $ProgressPreference = $pp

    Write-Host "  Extracting with 7-Zip..." -ForegroundColor Cyan
    & $sevenZip x $archive "-o$extract" -y | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "7z extraction failed: $LASTEXITCODE" }

    foreach ($name in @('opengl32.dll', 'libgallium_wgl.dll')) {
        $src = Join-Path $extract "x64\$name"
        if (-not (Test-Path $src)) { throw "Missing $name in Mesa archive -- upstream layout changed" }
        Copy-Item $src -Destination (Join-Path $mesaDir $name) -Force
    }
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue

    $total = (Get-ChildItem $mesaDir -File | Measure-Object Length -Sum).Sum / 1MB
    Write-Host "  Installed: opengl32.dll + libgallium_wgl.dll ($([Math]::Round($total,1)) MB)" -ForegroundColor Green
}

Write-Host ""
Write-Host "=== Setup complete ===" -ForegroundColor Cyan
Write-Host "Next: cargo tauri dev   (from repo root or crates/admin/src-tauri)" -ForegroundColor White
