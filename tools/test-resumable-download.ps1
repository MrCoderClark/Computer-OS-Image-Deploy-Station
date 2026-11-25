<#
.SYNOPSIS
    End-to-end test for Phase 19 resumable image transfers.

.DESCRIPTION
    Copies a real WIM into the server's images_dir under a seeded image ID,
    then exercises the /api/images/{id}/download endpoint three ways:
      1. Full download (200 OK, whole file).
      2. Range download (206 Partial Content, back half of the file).
      3. Reassembly of the two halves + SHA-256 comparison against the original.

    If the final SHA-256 matches, resumable transfer is bit-exact and the
    server/client plumbing works.

.PARAMETER WimPath
    UNC or local path to the source WIM. Default: \\192.168.70.7\captures\WIMs\install.wim

.PARAMETER ServerUrl
    Base URL of imaging-server. Default: https://localhost:7443

.PARAMETER WorkDir
    Scratch directory for the downloaded parts and reassembled WIM.
    Default: current directory \ ds-resume-test
#>
[CmdletBinding()]
param(
    [string]$WimPath   = '\\192.168.70.7\captures\WIMs\install.wim',
    [string]$ServerUrl = 'https://localhost:7443',
    [string]$WorkDir   = (Join-Path (Get-Location) 'ds-resume-test')
)

$ErrorActionPreference = 'Stop'

function Step($msg) { Write-Host ""; Write-Host "==> $msg" -ForegroundColor Cyan }
function Ok($msg)   { Write-Host "    [ok] $msg" -ForegroundColor Green }
function Warn($msg) { Write-Host "    [warn] $msg" -ForegroundColor Yellow }

# --- 0. Preflight ------------------------------------------------------------

if (-not (Test-Path $WimPath)) {
    throw "Source WIM not found: $WimPath"
}
$srcSize = (Get-Item $WimPath).Length
$srcSizeGB = [math]::Round($srcSize / 1GB, 2)
Write-Host "Source WIM: $WimPath ($srcSizeGB GB)"
Write-Host "Server:     $ServerUrl"
Write-Host "Workdir:    $WorkDir"

New-Item -ItemType Directory -Path $WorkDir -Force | Out-Null
$fullWim      = Join-Path $WorkDir 'out.wim'
$partialHead  = Join-Path $WorkDir 'out-partial-head.wim'
$partialTail  = Join-Path $WorkDir 'out-partial-tail.wim'
$reassembled  = Join-Path $WorkDir 'reassembled.wim'
foreach ($p in @($fullWim, $partialHead, $partialTail, $reassembled)) {
    if (Test-Path $p) { Remove-Item $p -Force }
}

# --- 1. Hash the source so we have an authoritative reference ----------------
#
# Cached in the workdir keyed by (path, size, mtime). Hashing 4+ GB over a UNC
# share is the slowest step in the script; if the source hasn't changed since
# last run, reuse the cached value.

$srcMtime = (Get-Item $WimPath).LastWriteTimeUtc.ToString('o')
$cachePath = Join-Path $WorkDir 'source-hash.cache'
$cacheKey  = "$WimPath|$srcSize|$srcMtime"

$srcHash = $null
if (Test-Path $cachePath) {
    $line = Get-Content $cachePath -Raw -ErrorAction SilentlyContinue
    if ($line) {
        $parts = $line.Trim() -split '\|', 4
        if ($parts.Length -eq 4 -and "$($parts[0])|$($parts[1])|$($parts[2])" -eq $cacheKey) {
            $srcHash = $parts[3]
            Step "Source hash: reusing cached value (path+size+mtime unchanged)"
            Ok "source SHA-256: $srcHash"
        }
    }
}

if (-not $srcHash) {
    Step "Hashing source WIM (this may take a few minutes for a $srcSizeGB GB file)"
    $srcHash = (Get-FileHash -Algorithm SHA256 $WimPath).Hash
    "$cacheKey|$srcHash" | Set-Content -Path $cachePath -Encoding UTF8
    Ok "source SHA-256: $srcHash (cached for next run)"
}

# --- 2. Authenticate against the server --------------------------------------

Step "Authenticating against $ServerUrl"
$auth = Invoke-RestMethod "$ServerUrl/api/auth" -Method Post `
    -Body '{"machine_name":"resume-test"}' -ContentType application/json `
    -SkipCertificateCheck
$tok = $auth.session_token
Ok "got session token"

# --- 3. Pick an image ID from the server catalog -----------------------------

Step "Fetching image catalog"
$cat = Invoke-RestMethod "$ServerUrl/api/images" `
    -Headers @{Authorization="Bearer $tok"} -SkipCertificateCheck
if (-not $cat.images -or $cat.images.Count -eq 0) {
    throw "Server catalog is empty. Nothing to hook the WIM to."
}
$id = $cat.images[0].id
Ok "using image id $id ($($cat.images[0].name))"

# --- 4. Copy the WIM into the server's images_dir ----------------------------

$dest = Join-Path $env:ProgramData "DeployStation\images\$id.wim"
Step "Copying source WIM to $dest"
Copy-Item -Path $WimPath -Destination $dest -Force
Ok "copied ($([math]::Round((Get-Item $dest).Length / 1GB, 2)) GB)"

# --- 5. Full download (200 OK) -----------------------------------------------

Step "Full download (expect HTTP 200)"
$t0 = Get-Date
Invoke-WebRequest "$ServerUrl/api/images/$id/download" `
    -Headers @{Authorization="Bearer $tok"} `
    -SkipCertificateCheck -OutFile $fullWim
$dt = (Get-Date) - $t0
$mb = [math]::Round($srcSize / 1MB, 1)
$mbs = [math]::Round($mb / [math]::Max($dt.TotalSeconds, 0.001), 1)
Ok "downloaded ${mb} MB in $([math]::Round($dt.TotalSeconds,1))s ($mbs MB/s)"

$fullHash = (Get-FileHash -Algorithm SHA256 $fullWim).Hash
if ($fullHash -ne $srcHash) {
    throw "Full download SHA-256 mismatch!`n  got:      $fullHash`n  expected: $srcHash"
}
Ok "full download sha256 matches source"

# --- 6. Range download: split halfway, request the back half -----------------

$size = (Get-Item $fullWim).Length
$half = [int64]($size / 2)
Step "Splitting the full download at byte $half, then requesting Range: bytes=$half-"

# Write the first half to partialHead using streams (WIM is > 2 GB).
$fsIn  = [IO.File]::OpenRead($fullWim)
$fsOut = [IO.File]::Create($partialHead)
try {
    $buf  = New-Object byte[] (4MB)
    [int64]$left = $half
    while ($left -gt 0) {
        # Min is called on two [int64]s, then the result (always <= $buf.Length)
        # is safely narrowed to [int] for the Read() call.
        $toRead = [int][Math]::Min([int64]$buf.Length, $left)
        $n = $fsIn.Read($buf, 0, $toRead)
        if ($n -le 0) { break }
        $fsOut.Write($buf, 0, $n)
        $left -= $n
    }
} finally {
    $fsOut.Close(); $fsIn.Close()
}
Ok "wrote head half ($([math]::Round($half / 1MB, 1)) MB)"

# Request the back half.
$t0 = Get-Date
Invoke-WebRequest "$ServerUrl/api/images/$id/download" `
    -Headers @{Authorization="Bearer $tok"; Range="bytes=$half-"} `
    -SkipCertificateCheck -OutFile $partialTail
$dt = (Get-Date) - $t0
$tailSize = (Get-Item $partialTail).Length
Ok "server returned $([math]::Round($tailSize / 1MB, 1)) MB in $([math]::Round($dt.TotalSeconds,1))s"

if ($tailSize -ne ($size - $half)) {
    throw "Range download size wrong: got $tailSize, expected $($size - $half)"
}

# --- 7. Reassemble + verify --------------------------------------------------

Step "Reassembling partial-head + partial-tail into $reassembled"
$fsOut = [IO.File]::Create($reassembled)
try {
    foreach ($part in @($partialHead, $partialTail)) {
        $fsIn = [IO.File]::OpenRead($part)
        try { $fsIn.CopyTo($fsOut) } finally { $fsIn.Close() }
    }
} finally {
    $fsOut.Close()
}

$reHash = (Get-FileHash -Algorithm SHA256 $reassembled).Hash
Write-Host ""
if ($reHash -eq $srcHash) {
    Write-Host "SUCCESS: resumable transfer is bit-exact." -ForegroundColor Green
    Write-Host "  sha256: $reHash"
    exit 0
} else {
    Write-Host "FAIL: reassembled WIM does NOT match source." -ForegroundColor Red
    Write-Host "  got:      $reHash"
    Write-Host "  expected: $srcHash"
    exit 1
}
