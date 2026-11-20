<#
.SYNOPSIS
    Injects the imaging agent and its transitive DLL dependencies into a mounted
    WinPE image, plus Mesa3D's software OpenGL (llvmpipe) for glow/egui rendering.

.DESCRIPTION
    WinPE ships a minimal subset of Windows DLLs. The agent (built against
    eframe/glow + winit) pulls in a wider set that must be satisfied before the
    EXE will load at all — a single missing static import fails the loader with
    0xC0000135 (STATUS_DLL_NOT_FOUND) before main() runs.

    This script:
      1. Copies the agent binary into WinPE's System32.
      2. Parses the agent's PE import table and walks it transitively, copying
         every missing real DLL from the host's System32 (and downlevel). API
         Set contracts (api-ms-win-*, ext-ms-*) are not recursed into — WinPE
         resolves those at load time via its apiset schema.
      3. Copies Mesa3D's software OpenGL stack (opengl32.dll +
         libgallium_wgl.dll) over the host opengl32.dll. The stock Windows
         opengl32.dll is just an ICD loader expecting a GPU driver that WinPE
         does not have; Mesa's llvmpipe does the whole pipeline in CPU.

.PARAMETER MountDir
    Path to the mounted boot.wim root (contains Windows\System32).

.PARAMETER AgentPath
    Path to the compiled imaging-agent.exe.

.PARAMETER MesaDir
    Directory holding Mesa3D's x64 opengl32.dll and libgallium_wgl.dll.
    Download from https://github.com/pal1000/mesa-dist-win/releases
    (x64 release-msvc build).

.PARAMETER HostSystem32
    Path to the host's System32 to source DLLs from. Default: C:\Windows\System32.
#>
param(
    [Parameter(Mandatory = $true)][string]$MountDir,
    [Parameter(Mandatory = $true)][string]$AgentPath,
    [Parameter(Mandatory = $true)][string]$MesaDir,
    [string]$HostSystem32 = "$env:SystemRoot\System32"
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path (Join-Path $MountDir 'Windows\System32'))) {
    throw "MountDir does not look like a mounted WinPE image: $MountDir"
}
if (-not (Test-Path $AgentPath)) { throw "Agent binary not found: $AgentPath" }

$winpeSys32 = Join-Path $MountDir 'Windows\System32'
$dllSearchDirs = @($HostSystem32, (Join-Path $HostSystem32 'downlevel'))

Copy-Item -Path $AgentPath -Destination (Join-Path $winpeSys32 'imaging-agent.exe') -Force
Write-Host "  Copied imaging-agent.exe into WinPE" -ForegroundColor Green

# --- PE import parser: returns the DLL names a PE file statically imports. ---
function Get-PeImports {
    param([string]$FilePath)
    $out = @()
    try { $bytes = [System.IO.File]::ReadAllBytes($FilePath) } catch { return $out }
    if ($bytes.Length -lt 0x40) { return $out }
    $peOff = [BitConverter]::ToUInt32($bytes, 0x3C)
    if (($peOff + 24) -ge $bytes.Length) { return $out }
    if ($bytes[$peOff] -ne 0x50 -or $bytes[$peOff + 1] -ne 0x45) { return $out }
    $coff = $peOff + 4
    $numSections = [BitConverter]::ToUInt16($bytes, $coff + 2)
    $optSize = [BitConverter]::ToUInt16($bytes, $coff + 16)
    $optOff = $coff + 20
    $magic = [BitConverter]::ToUInt16($bytes, $optOff)
    if ($magic -eq 0x20b) { $dataDirOff = $optOff + 112 }
    elseif ($magic -eq 0x10b) { $dataDirOff = $optOff + 96 }
    else { return $out }
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
    $impOff = -1
    foreach ($sec in $secs) {
        $size = [Math]::Max($sec.VS, 1)
        if ($importRva -ge $sec.V -and $importRva -lt ($sec.V + $size)) {
            $impOff = $sec.R + ($importRva - $sec.V); break
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
                    $nameOff = $sec.R + ($nameRva - $sec.V); break
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

function Find-DllOnHost {
    param([string]$Dll)
    foreach ($dir in $dllSearchDirs) {
        $p = Join-Path $dir $Dll
        if (Test-Path $p) { return $p }
    }
    return $null
}

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

function Test-IsApiSet {
    param([string]$Dll)
    return ($Dll -like 'api-ms-win-*' -or $Dll -like 'ext-ms-*')
}

# --- Install Mesa3D llvmpipe FIRST so its DLLs join the walk. ---
# opengl32.dll is only the ICD stub; libgallium_wgl.dll holds the renderer.
$mesaOpenGL = Join-Path $MesaDir 'opengl32.dll'
if (-not (Test-Path $mesaOpenGL)) {
    throw @"
Mesa3D opengl32.dll not found at $mesaOpenGL.
Download the x64 release-msvc build from:
  https://github.com/pal1000/mesa-dist-win/releases
Then copy x64\opengl32.dll AND x64\libgallium_wgl.dll into $MesaDir.
(opengl32.dll alone is not enough.)
"@
}

$toScan = [System.Collections.Generic.Queue[string]]::new()
$seen   = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)

$mesaDlls = Get-ChildItem -Path $MesaDir -Filter '*.dll' -File
foreach ($dll in $mesaDlls) {
    $dst = Join-Path $winpeSys32 $dll.Name
    Copy-Item -Path $dll.FullName -Destination $dst -Force
    [void]$seen.Add($dll.Name)
    $toScan.Enqueue($dst)
    Write-Host "  Installed Mesa $($dll.Name) ($([Math]::Round($dll.Length / 1MB, 1)) MB)" -ForegroundColor Green
}

# Runtime-only DLLs the renderer loads dynamically (not in the import table).
# VC++ runtime is a safety net in case crt-static misses something.
$runtimeExtras = @('vcruntime140.dll', 'vcruntime140_1.dll', 'msvcp140.dll')
foreach ($x in $runtimeExtras) {
    if ($seen.Contains($x)) { continue }
    [void]$seen.Add($x)
    $p = Add-DllToWinPE $x
    if ($p) { $toScan.Enqueue($p) }
    elseif (-not (Test-Path (Join-Path $winpeSys32 $x))) {
        Write-Host "  (optional) $x not on host, skipping" -ForegroundColor DarkGray
    }
}

# Enqueue the agent itself and walk the transitive closure.
$toScan.Enqueue((Join-Path $winpeSys32 'imaging-agent.exe'))
while ($toScan.Count -gt 0) {
    $file = $toScan.Dequeue()
    foreach ($imp in (Get-PeImports $file)) {
        if ($seen.Contains($imp)) { continue }
        [void]$seen.Add($imp)
        if (Test-Path (Join-Path $winpeSys32 $imp)) { continue }
        $injected = Add-DllToWinPE $imp
        if ($injected) {
            if (-not (Test-IsApiSet $imp)) { $toScan.Enqueue($injected) }
        } elseif (-not (Test-IsApiSet $imp)) {
            $leaf = Split-Path $file -Leaf
            Write-Host "  WARN: $imp needed by $leaf is not on the host either" -ForegroundColor Yellow
        }
    }
}

Write-Host "  Agent dependency injection complete." -ForegroundColor Cyan
