#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Build a Hyper-V WinPE ISO wired for Online-mode deploy against the local
    imaging-server, and provision a VM to boot it.

.DESCRIPTION
    Does the end-to-end setup for Phase 20 verification without a physical USB:

      1. Fetches the server's cert PEM via GET /cert (so the baked config pins
         to the same cert the admin trusts — no "accept any" fallback).
      2. Writes a SuiteConfig JSON (Online mode, Fixed discovery, baked cert).
      3. Invokes test-winpe-hyperv.ps1 with -ConfigPath, which bundles the agent,
         Mesa3D, and the config into a bootable UEFI ISO and attaches it to a
         Gen2 Hyper-V VM on the Default Switch.

    After this completes, Start-VM / vmconnect to boot and hit Deploy.

.PARAMETER ServerHost
    The hostname (or IP) the agent should use to reach the server. Must appear
    in the cert's SAN list -- for the self-signed cert, that means `localhost`,
    `127.0.0.1`, `::1`, or the host's COMPUTERNAME. From a Hyper-V VM on the
    Default Switch, 127.0.0.1 doesn't reach the host; COMPUTERNAME does.
    Default: $env:COMPUTERNAME.

.PARAMETER ServerPort
    TCP port the server binds to. Default 7443.

.PARAMETER EnrollmentKey
    Optional pre-shared key the agent sends during auth. Required if the
    server has any active keys.

.PARAMETER OutputIso
    Where to write the bootable ISO. Default: .\winpe-online-test.iso

.PARAMETER VmName
    Hyper-V VM name. Default: WinPE-Agent-Online.

.PARAMETER AgentPath
    Agent binary to inject. Default: .\target\release\imaging-agent.exe.
#>
[CmdletBinding()]
param(
    [string]$ServerHost     = $env:COMPUTERNAME,
    [int]   $ServerPort     = 7443,
    [string]$EnrollmentKey  = $null,
    [string]$OutputIso      = ".\winpe-online-test.iso",
    [string]$VmName         = "WinPE-Agent-Online",
    [string]$AgentPath      = ".\target\release\imaging-agent.exe",
    # VM memory. Agent caches the WIM in WinPE's RAM-backed X:\Temp, so this
    # must exceed the server WIM size + ~1.5 GB headroom. 6 GB fits the 4.37 GB
    # Phase 19 test WIM; bump for larger images.
    [int64]$MemoryBytes     = 6GB
)

$ErrorActionPreference = 'Stop'
function Step($msg) { Write-Host ""; Write-Host "==> $msg" -ForegroundColor Cyan }
function Ok($msg)   { Write-Host "    [ok] $msg" -ForegroundColor Green }

# --- 1. Fetch the server cert (PEM) ---------------------------------------
# /cert is a public endpoint (no auth), returned as text/plain. We go via
# localhost because this script runs on the server host; the baked config
# will tell the agent to connect by $ServerHost instead.

Step "Fetching server cert from https://localhost:$ServerPort/cert"
try {
    $pem = Invoke-RestMethod "https://localhost:$ServerPort/cert" -SkipCertificateCheck
} catch {
    throw "Failed to fetch /cert -- is DeployStationServer running on port $ServerPort? ($($_.Exception.Message))"
}
if (-not $pem -or $pem -notlike '*BEGIN CERTIFICATE*') {
    throw "Server returned no PEM at /cert (got: $($pem.Substring(0,[Math]::Min(80,$pem.Length))))"
}
Ok "got cert ($([Math]::Round($pem.Length / 1024, 1)) KB)"

# --- 2. Write the suite config --------------------------------------------
# Matches the shape serde produces for SuiteConfig: lowercase enums
# (mode: "online", discovery: "fixed"), nested [server] block, baked cert_pem.
# chrono's DateTime<Utc> Serializes as RFC3339 with Z suffix.

Step "Writing suite config"
$built_at = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffffffZ")
$config = [ordered]@{
    schema_version = 1
    mode           = "online"
    server         = [ordered]@{
        discovery = "fixed"
        host      = $ServerHost
        port      = $ServerPort
        cert_pem  = $pem
    }
    built_by  = "$env:USERNAME@$env:COMPUTERNAME (build-hyperv-online.ps1)"
    built_at  = $built_at
}
if ($EnrollmentKey) { $config["enrollment_key"] = $EnrollmentKey }

$configPath = Join-Path $env:TEMP "deploystation-online-config.json"
$config | ConvertTo-Json -Depth 5 | Set-Content -Path $configPath -Encoding UTF8
Ok "wrote $configPath"
Ok "server target baked in: https://${ServerHost}:${ServerPort}"

# --- 3. Hand off to the WinPE+Hyper-V builder -----------------------------

Step "Building WinPE ISO + provisioning Hyper-V VM '$VmName'"
& "$PSScriptRoot\test-winpe-hyperv.ps1" `
    -AgentPath $AgentPath `
    -OutputIso $OutputIso `
    -VmName $VmName `
    -ConfigPath $configPath `
    -MemoryBytes $MemoryBytes

Write-Host ""
Write-Host "Done. To boot and deploy:" -ForegroundColor Yellow
Write-Host "  Start-VM -Name '$VmName'"
Write-Host "  vmconnect localhost '$VmName'"
Write-Host ""
Write-Host "Inside the VM: agent should auto-connect to $ServerHost`:$ServerPort," -ForegroundColor Yellow
Write-Host "show 'Server connected', and let you click Deploy to download + apply the WIM."
