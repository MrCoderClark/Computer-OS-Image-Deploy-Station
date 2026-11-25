#Requires -RunAsAdministrator
# Creates a WinPE ISO with the imaging agent for testing in Hyper-V.
# Usage: .\tools\test-winpe-hyperv.ps1

param(
    [string]$AgentPath = ".\target\release\imaging-agent.exe",
    [string]$OutputIso = ".\winpe-test.iso",
    [string]$VmName = "WinPE-Agent-Test",
    # Directory containing the Mesa3D distribution's x64 contents (all DLLs together).
    # opengl32.dll is only the ICD stub; it needs libgallium_wgl.dll + friends shipped
    # alongside it. The whole x64\ folder from pal1000/mesa-dist-win release-msvc goes
    # here. Replaces the host opengl32.dll in WinPE so glow/egui renders without a GPU
    # or DirectComposition.
    # Download: https://github.com/pal1000/mesa-dist-win/releases  (x64 release-msvc)
    [string]$MesaDir = ".\tools\mesa",
    # Optional: SuiteConfig JSON to inject at X:\imaging\config.json. Without
    # it, the agent runs in standalone mode (no server, no deploy).
    [string]$ConfigPath = $null,
    # Optional: Hyper-V virtual switch to attach so the VM can reach the LAN.
    # "Default Switch" is Hyper-V's built-in NAT switch (DNS-forwarded,
    # internet-capable). Pass an empty string to leave the VM offline.
    [string]$SwitchName = "Default Switch",
    # VM memory. 2 GB is fine for a GUI-only smoke test. For Online-mode
    # deploys the agent currently caches the WIM in WinPE's RAM-backed X:\Temp,
    # so this needs to be >= WIM size + ~1.5 GB headroom.
    [int64]$MemoryBytes = 2GB,
    # WinPE scratch space (writable X: capacity) in MB. Must be one of
    # 32/64/128/256/512/1024 -- 1024 is the hard max enforced by the WinPE
    # boot loader. Capping download stages the agent can hold in RAM.
    [int]$ScratchMB = 1024
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

# A prior aborted run may have left boot.wim mounted at $workDir\mount. The
# mount holds a file handle that blocks Remove-Item. Discard any WinPE mount
# pointing into our workdir, then let DISM reap orphaned mount records.
Get-WindowsImage -Mounted -ErrorAction SilentlyContinue | Where-Object {
    $_.Path -and $_.Path.StartsWith($workDir, [System.StringComparison]::OrdinalIgnoreCase)
} | ForEach-Object {
    Write-Host "  Discarding stale mount at $($_.Path)" -ForegroundColor Yellow
    Dismount-WindowsImage -Path $_.Path -Discard -ErrorAction SilentlyContinue | Out-Null
}
& Dism.exe /Cleanup-Wim 2>&1 | Out-Null
& Dism.exe /Cleanup-Mountpoints 2>&1 | Out-Null

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

Write-Host "[4/7] Injecting agent, DLLs, and Mesa3D..." -ForegroundColor Cyan
& "$PSScriptRoot\winpe-inject-deps.ps1" -MountDir $mountDir -AgentPath $AgentPath -MesaDir $MesaDir

# Inject suite config for a server-driven run (Online/Hybrid). Agent reads it
# from X:\imaging\config.json inside WinPE.
if ($ConfigPath) {
    if (-not (Test-Path $ConfigPath)) { throw "ConfigPath not found: $ConfigPath" }
    $imagingDir = Join-Path $mountDir 'imaging'
    New-Item -ItemType Directory -Path $imagingDir -Force | Out-Null
    Copy-Item -Path $ConfigPath -Destination (Join-Path $imagingDir 'config.json') -Force
    Write-Host "  Injected suite config: $ConfigPath -> X:\imaging\config.json" -ForegroundColor Green
}

# Bump WinPE scratch space so the agent has more room to stage downloads in RAM.
# Default is 512 MB; DISM accepts 32/64/128/256/512, and 1024 on newer ADKs only.
# Non-fatal: if DISM rejects the requested size we fall back to 512 and keep
# building -- the default WinPE scratch (~32 MB on some templates, ~512 on 24H2)
# is enough for a tiny Phase 20 test WIM, which is what Hyper-V can hold anyway.
$allowed = 32, 64, 128, 256, 512, 1024
if ($allowed -notcontains $ScratchMB) { throw "ScratchMB must be one of: $($allowed -join ', ')" }
$trySizes = @($ScratchMB)
if ($ScratchMB -ne 512) { $trySizes += 512 }
$scratchSet = $false
foreach ($size in $trySizes) {
    Write-Host "  Setting WinPE scratch space to $size MB..." -ForegroundColor Green
    $dismOut = & Dism.exe /Image:$mountDir /Set-ScratchSpace:$size 2>&1
    if ($LASTEXITCODE -eq 0) { $scratchSet = $true; break }
    Write-Host "    DISM rejected $size MB (exit $LASTEXITCODE): $($dismOut | Select-Object -Last 2)" -ForegroundColor Yellow
}
if (-not $scratchSet) {
    Write-Host "  Continuing with the WIM's default scratch space." -ForegroundColor Yellow
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
REM Force Mesa3D to use its llvmpipe software renderer directly; skips the
REM failed ZINK/Vulkan probe that prints "MESA: error: ZINK: failed to load
REM vulkan-1.dll" on startup (harmless but confusing).
set GALLIUM_DRIVER=llvmpipe
echo Starting agent...
imaging-agent.exe
set AGENT_EXIT=%errorlevel%
if %AGENT_EXIT% NEQ 0 (
    echo.
    echo !! Agent exited with code %AGENT_EXIT%
    echo.
    echo --- DLL presence check in %SystemRoot%\System32 ---
    for %%D in (bcryptprimitives.dll dwmapi.dll uxtheme.dll opengl32.dll libgallium_wgl.dll gdi32.dll gdi32full.dll vcruntime140.dll vcruntime140_1.dll msvcp140.dll shlwapi.dll shell32.dll ole32.dll imm32.dll secur32.dll crypt32.dll api-ms-win-core-synch-l1-2-0.dll) do (
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
    New-VM -Name $VmName -MemoryStartupBytes $MemoryBytes -Generation 2 -NoVHD | Out-Null
    Add-VMDvdDrive -VMName $VmName -Path $isoFull
    $dvd = Get-VMDvdDrive -VMName $VmName
    Set-VMProcessor -VMName $VmName -Count 2
}
# Apply memory on every run so a VM created earlier at a smaller size gets
# the requested amount. Only touches startup memory (dynamic memory off by default).
Set-VMMemory -VMName $VmName -StartupBytes $MemoryBytes

# Apply boot settings on EVERY run so a VM created earlier (e.g. with Secure Boot on)
# gets corrected. WinPE media is unsigned, so Secure Boot must be off, and the DVD
# must be first in the boot order.
Set-VMFirmware -VMName $VmName -EnableSecureBoot Off
Set-VMFirmware -VMName $VmName -FirstBootDevice $dvd

# Attach a network adapter so the agent can reach the host's imaging-server.
# "Default Switch" is Hyper-V's built-in NAT switch; it exists on every Win10+
# host. Only touched if $SwitchName is set.
if ($SwitchName) {
    if (-not (Get-VMSwitch -Name $SwitchName -ErrorAction SilentlyContinue)) {
        Write-Host "  (switch '$SwitchName' not found; skipping network attach)" -ForegroundColor Yellow
    } else {
        $nic = Get-VMNetworkAdapter -VMName $VmName | Select-Object -First 1
        if ($nic) {
            Connect-VMNetworkAdapter -VMName $VmName -SwitchName $SwitchName
        } else {
            Add-VMNetworkAdapter -VMName $VmName -SwitchName $SwitchName
        }
        Write-Host "  Attached to switch: $SwitchName" -ForegroundColor Green
    }
}

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
