#Requires -RunAsAdministrator
# Creates a WinPE ISO with the imaging agent for testing in Hyper-V.
# Usage: .\tools\test-winpe-hyperv.ps1

param(
    [string]$AgentPath = ".\target\release\imaging-agent.exe",
    [string]$OutputIso = ".\winpe-test.iso",
    [string]$VmName = "WinPE-Agent-Test"
)

$ErrorActionPreference = "Stop"

# --- Locate ADK ---
$adkBase = "C:\Program Files (x86)\Windows Kits\10\Assessment and Deployment Kit"
$deployTools = "$adkBase\Deployment Tools"
$winpeAddon = "$adkBase\Windows Preinstallation Environment"

if (-not (Test-Path $deployTools)) { throw "Windows ADK not found at $deployTools" }
if (-not (Test-Path $winpeAddon)) { throw "WinPE add-on not found at $winpeAddon" }

# --- Check agent binary ---
if (-not (Test-Path $AgentPath)) { throw "Agent binary not found at $AgentPath" }
Write-Host "[1/7] Agent binary: $AgentPath" -ForegroundColor Cyan

# --- Create WinPE working directory ---
$workDir = Join-Path $env:TEMP "DeployStation-HyperV-Test"
if (Test-Path $workDir) { Remove-Item $workDir -Recurse -Force }

Write-Host "[2/7] Creating WinPE working directory..." -ForegroundColor Cyan
$env:WinPERoot = $winpeAddon
$env:DISMRoot = "$deployTools\amd64\DISM"
$env:OSCDImgRoot = "$deployTools\amd64\Oscdimg"
$env:BCDBootRoot = "$deployTools\amd64\BCDBoot"
$env:ImagingRoot = "$deployTools\amd64"
$env:Path = "$winpeAddon;$deployTools\amd64\DISM;$deployTools\amd64;$deployTools;$env:Path"

& cmd.exe /c "`"$winpeAddon\copype.cmd`" amd64 `"$workDir`"" 2>&1 | Out-Null
if (-not (Test-Path "$workDir\media")) { throw "copype failed" }

# --- Mount boot.wim and inject agent ---
Write-Host "[3/7] Mounting boot.wim..." -ForegroundColor Cyan
$bootWim = "$workDir\media\sources\boot.wim"
$mountDir = "$workDir\mount"
New-Item -ItemType Directory -Path $mountDir -Force | Out-Null
Mount-WindowsImage -ImagePath $bootWim -Index 1 -Path $mountDir -ErrorAction Stop

Write-Host "[4/7] Injecting agent and DLLs..." -ForegroundColor Cyan
Copy-Item -Path $AgentPath -Destination "$mountDir\Windows\System32\imaging-agent.exe" -Force

# Inject every DLL the agent needs but WinPE lacks. Rather than hardcode a list
# (which we kept guessing wrong), parse the agent's PE import table and walk the
# dependency tree transitively, injecting each missing DLL from the host. A missing
# static import makes the loader fail the process with 0xC0000135 (STATUS_DLL_NOT_FOUND)
# before any agent code runs.

# Parse the import directory of a PE file; return the names of DLLs it statically imports.
function Get-PeImports {
    param([string]$FilePath)
    $out = @()
    try {
        $bytes = [System.IO.File]::ReadAllBytes($FilePath)
    } catch {
        return $out
    }
    if ($bytes.Length -lt 0x40) { return $out }
    $peOff = [BitConverter]::ToUInt32($bytes, 0x3C)
    if (($peOff + 24) -ge $bytes.Length) { return $out }
    # "PE" signature
    if ($bytes[$peOff] -ne 0x50 -or $bytes[$peOff + 1] -ne 0x45) { return $out }
    $coff = $peOff + 4
    $numSections = [BitConverter]::ToUInt16($bytes, $coff + 2)
    $optSize = [BitConverter]::ToUInt16($bytes, $coff + 16)
    $optOff = $coff + 20
    $magic = [BitConverter]::ToUInt16($bytes, $optOff)
    if ($magic -eq 0x20b) {
        $dataDirOff = $optOff + 112
    } elseif ($magic -eq 0x10b) {
        $dataDirOff = $optOff + 96
    } else {
        return $out
    }
    # Data directory [1] is the import table.
    $importRva = [BitConverter]::ToUInt32($bytes, $dataDirOff + 8)
    if ($importRva -eq 0) { return $out }

    $secOff = $optOff + $optSize
    $secs = @()
    for ($i = 0; $i -lt $numSections; $i++) {
        $s = $secOff + ($i * 40)
        $secs += [pscustomobject]@{
            V  = [BitConverter]::ToUInt32($bytes, $s + 12)
            VS = [BitConverter]::ToUInt32($bytes, $s + 8)
            R  = [BitConverter]::ToUInt32($bytes, $s + 20)
        }
    }

    # Map a relative virtual address to a file offset using the section table.
    $impOff = -1
    foreach ($sec in $secs) {
        $size = [Math]::Max($sec.VS, 1)
        if ($importRva -ge $sec.V -and $importRva -lt ($sec.V + $size)) {
            $impOff = $sec.R + ($importRva - $sec.V)
            break
        }
    }
    if ($impOff -lt 0) { return $out }

    while (($impOff + 20) -le $bytes.Length) {
        $nameRva = [BitConverter]::ToUInt32($bytes, $impOff + 12)
        $firstThunk = [BitConverter]::ToUInt32($bytes, $impOff + 16)
        if ($nameRva -eq 0 -and $firstThunk -eq 0) { break }
        if ($nameRva -ne 0) {
            $nameOff = -1
            foreach ($sec in $secs) {
                $size = [Math]::Max($sec.VS, 1)
                if ($nameRva -ge $sec.V -and $nameRva -lt ($sec.V + $size)) {
                    $nameOff = $sec.R + ($nameRva - $sec.V)
                    break
                }
            }
            if ($nameOff -ge 0) {
                $end = $nameOff
                while ($end -lt $bytes.Length -and $bytes[$end] -ne 0) { $end++ }
                $name = [System.Text.Encoding]::ASCII.GetString($bytes, $nameOff, $end - $nameOff)
                if ($name) { $out += $name }
            }
        }
        $impOff += 20
    }
    return $out
}

$winpeSys32 = Join-Path $mountDir "Windows\System32"
$dllSearchDirs = @("C:\Windows\System32", "C:\Windows\System32\downlevel")

function Find-DllOnHost {
    param([string]$Dll)
    foreach ($dir in $dllSearchDirs) {
        $p = Join-Path $dir $Dll
        if (Test-Path $p) { return $p }
    }
    return $null
}

# Ensure a DLL is present in WinPE, injecting it from the host if missing.
# Returns the WinPE path if present/injected, else $null.
function Add-DllToWinPE {
    param([string]$Dll)
    $dst = Join-Path $winpeSys32 $Dll
    if (Test-Path $dst) { return $dst }
    $src = Find-DllOnHost $Dll
    if (-not $src) { return $null }
    Copy-Item -Path $src -Destination $dst -Force
    Write-Host "  Injected $Dll" -ForegroundColor Green
    return $dst
}

# API Set contract DLLs (api-ms-win-*, ext-ms-*) are virtual: they have no real
# file and are resolved at load time by WinPE's API Set schema to real DLLs like
# kernelbase.dll. We inject a stub if the host happens to have one (harmless), but
# we never recurse into one or warn when it has no file -- that is expected.
function Test-IsApiSet {
    param([string]$Dll)
    return ($Dll -like 'api-ms-win-*' -or $Dll -like 'ext-ms-*')
}

# Runtime-only DLLs wgpu loads dynamically (NOT in the import table, so the walker
# below cannot discover them) but which it needs for software rendering in WinPE,
# which has no GPU:
#   d3d12.dll          Direct3D 12 runtime      (wgpu loads via libloading)
#   dxgi.dll           DXGI adapter enumeration  (wgpu loads via libloading)
#   d3d10warp.dll      WARP software Direct3D rasterizer (the only renderer WinPE can use)
#   d3dcompiler_47.dll shader compilation
#   d3d12core.dll      D3D12 core redistributable (newer Windows)
#   vcruntime140*.dll  VC++ runtime safety net (should be covered by crt-static)
$runtimeExtras = @(
    "d3d12.dll", "dxgi.dll", "d3d10warp.dll", "d3dcompiler_47.dll", "d3d12core.dll",
    "vcruntime140.dll", "vcruntime140_1.dll"
)

# Breadth-first walk: scan the exe's imports, inject any missing DLL from the host,
# then scan that injected DLL's own imports too (transitive closure). DLLs already
# present in WinPE are core and assumed self-consistent, so we do not recurse into them.
$toScan = [System.Collections.Generic.Queue[string]]::new()
$seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)

$toScan.Enqueue((Join-Path $winpeSys32 "imaging-agent.exe"))
foreach ($x in $runtimeExtras) {
    [void]$seen.Add($x)
    $p = Add-DllToWinPE $x
    if ($p) {
        $toScan.Enqueue($p)
    } elseif (-not (Test-Path (Join-Path $winpeSys32 $x))) {
        Write-Host "  (optional) $x not on host, skipping" -ForegroundColor DarkGray
    }
}

while ($toScan.Count -gt 0) {
    $file = $toScan.Dequeue()
    foreach ($imp in (Get-PeImports $file)) {
        if ($seen.Contains($imp)) { continue }
        [void]$seen.Add($imp)
        if (Test-Path (Join-Path $winpeSys32 $imp)) { continue }
        $injected = Add-DllToWinPE $imp
        if ($injected) {
            # Recurse into real DLLs only; API Set stubs are leaf virtual contracts.
            if (-not (Test-IsApiSet $imp)) { $toScan.Enqueue($injected) }
        } elseif (-not (Test-IsApiSet $imp)) {
            $leaf = Split-Path $file -Leaf
            Write-Host "  WARN: $imp needed by $leaf is not on the host either" -ForegroundColor Yellow
        }
    }
}

# Write startnet.cmd
Write-Host "[5/7] Writing startnet.cmd..." -ForegroundColor Cyan
$startnet = @"
@echo off
wpeinit
echo.
echo ============================================
echo   DeployStation - Windows Imaging Agent
echo ============================================
echo.
echo Starting agent...
imaging-agent.exe
set AGENT_EXIT=%errorlevel%
if %AGENT_EXIT% NEQ 0 (
    echo.
    echo !! Agent exited with code %AGENT_EXIT%
    echo.
    echo --- DLL presence check in %SystemRoot%\System32 ---
    for %%D in (bcryptprimitives.dll dwmapi.dll uxtheme.dll opengl32.dll d3d12.dll dxgi.dll d3dcompiler_47.dll vcruntime140.dll vcruntime140_1.dll api-ms-win-core-synch-l1-2-0.dll) do (
        if exist %SystemRoot%\System32\%%D (echo   %%D ... OK) else (echo   %%D ... MISSING)
    )
    echo.
    echo Code -1073741515 ^(0xC0000135^) = a MISSING dll above is a static import.
)
echo.
echo Press any key to open command prompt...
pause >nul
cmd.exe
"@
Set-Content -Path "$mountDir\Windows\System32\startnet.cmd" -Value $startnet -Encoding ASCII

# --- Unmount ---
Write-Host "[6/7] Saving boot.wim (this takes a moment)..." -ForegroundColor Cyan
Dismount-WindowsImage -Path $mountDir -Save -ErrorAction Stop

# --- Create ISO ---
Write-Host "[7/7] Creating ISO..." -ForegroundColor Cyan
$oscdimg = "$deployTools\amd64\Oscdimg\oscdimg.exe"

# efisys boot sector lives in copype's fwfiles dir on some ADK versions, and always
# in the ADK Oscdimg folder. Prefer the _noprompt variant so the VM boots without
# waiting for a "Press any key to boot from CD" keystroke it will never receive.
$efisysCandidates = @(
    "$workDir\fwfiles\efisys_noprompt.bin",
    "$deployTools\amd64\Oscdimg\efisys_noprompt.bin",
    "$workDir\fwfiles\efisys.bin",
    "$deployTools\amd64\Oscdimg\efisys.bin"
)
$efisysSrc = $efisysCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $efisysSrc) {
    throw "efisys.bin not found. Looked in:`n  $($efisysCandidates -join "`n  ")"
}
# Copy to the space-free work dir so the -bootdata path needs no quote-escaping
# (the ADK source lives under 'C:\Program Files (x86)\...' which has spaces).
$efisys = "$workDir\efisys.bin"
Copy-Item -Path $efisysSrc -Destination $efisys -Force
Write-Host "  Using boot sector: $efisysSrc" -ForegroundColor Green

if (Test-Path $OutputIso) { Remove-Item $OutputIso -Force }

# UEFI-only El Torito boot: 1 entry, platform EF (EFI), no emulation (e), boot file (b).
# This mirrors MakeWinPEMedia.cmd minus the BIOS/etfsboot entry. -u2 => UDF; -n conflicts with -u2.
# %TEMP% has no spaces, so the boot path is passed without quote-escaping (which is what broke before).
$bootData = "-bootdata:1#pEF,e,b$efisys"
& $oscdimg -m -o -u2 -udfver102 $bootData "$workDir\media" $OutputIso
if ($LASTEXITCODE -ne 0) { throw "oscdimg failed with exit code $LASTEXITCODE" }

if (-not (Test-Path $OutputIso)) { throw "Failed to create ISO" }
Write-Host "`nISO created: $OutputIso" -ForegroundColor Green

# --- Create/Update Hyper-V VM ---
Write-Host "`nSetting up Hyper-V VM '$VmName'..." -ForegroundColor Cyan

$isoFull = (Resolve-Path $OutputIso).Path
$existingVm = Get-VM -Name $VmName -ErrorAction SilentlyContinue
if ($existingVm) {
    if ($existingVm.State -ne 'Off') { Stop-VM -Name $VmName -Force -TurnOff }
    # Ensure a DVD drive exists and points to the new ISO
    $dvd = Get-VMDvdDrive -VMName $VmName
    if (-not $dvd) {
        Add-VMDvdDrive -VMName $VmName -Path $isoFull
        $dvd = Get-VMDvdDrive -VMName $VmName
    } else {
        Set-VMDvdDrive -VMName $VmName -Path $isoFull
    }
} else {
    New-VM -Name $VmName -MemoryStartupBytes 2GB -Generation 2 -NoVHD | Out-Null
    Add-VMDvdDrive -VMName $VmName -Path $isoFull
    $dvd = Get-VMDvdDrive -VMName $VmName
    Set-VMProcessor -VMName $VmName -Count 2
}

# Apply boot settings on EVERY run so a VM created earlier (e.g. with Secure Boot on)
# gets corrected. WinPE media is unsigned, so Secure Boot must be off, and the DVD
# must be first in the boot order.
Set-VMFirmware -VMName $VmName -EnableSecureBoot Off
Set-VMFirmware -VMName $VmName -FirstBootDevice $dvd

Write-Host "VM '$VmName' ready (Secure Boot off, DVD first)." -ForegroundColor Green
Write-Host ""
Write-Host "To test:" -ForegroundColor Yellow
Write-Host "  Start-VM -Name '$VmName'"
Write-Host "  vmconnect localhost '$VmName'"
Write-Host ""
Write-Host "To iterate (rebuild + retest):" -ForegroundColor Yellow
Write-Host "  cargo build --release -p imaging-agent"
Write-Host "  .\tools\test-winpe-hyperv.ps1"
Write-Host "  Start-VM -Name '$VmName'"

# Cleanup working directory
Remove-Item $workDir -Recurse -Force -ErrorAction SilentlyContinue
