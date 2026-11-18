#Requires -Version 5.1
<#
.SYNOPSIS
    Signs the imaging-agent.exe binary with an Authenticode certificate.

.DESCRIPTION
    Scaffold script for code-signing the DeployStation agent binary.
    Requires signtool.exe (Windows SDK) and a code-signing certificate.

    This signs the user-mode EXE for integrity verification. It does NOT
    affect Secure Boot — WinPE's boot chain is already Microsoft-signed.
    See docs/secure-boot-signing.md for the full analysis.

.PARAMETER CertThumbprint
    SHA-1 thumbprint of the code-signing certificate in the local machine
    or current user certificate store.

.PARAMETER PfxPath
    Path to a PFX file containing the code-signing certificate and private key.

.PARAMETER PfxPassword
    Password for the PFX file (prompted if not supplied).

.PARAMETER TimestampServer
    RFC 3161 timestamp server URL. Defaults to DigiCert's server.

.EXAMPLE
    .\sign-agent.ps1 -CertThumbprint "A1B2C3D4..."
    .\sign-agent.ps1 -PfxPath .\certs\code-signing.pfx
#>
[CmdletBinding(DefaultParameterSetName = 'Thumbprint')]
param(
    [Parameter(ParameterSetName = 'Thumbprint')]
    [string]$CertThumbprint,

    [Parameter(ParameterSetName = 'Pfx')]
    [string]$PfxPath,

    [Parameter(ParameterSetName = 'Pfx')]
    [SecureString]$PfxPassword,

    [string]$TimestampServer = 'http://timestamp.digicert.com',

    [string]$AgentPath = (Join-Path $PSScriptRoot '..\target\x86_64-pc-windows-msvc\release\imaging-agent.exe')
)

$ErrorActionPreference = 'Stop'

# --- Locate signtool ---
$signtool = $null
$sdkPaths = @(
    "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe"
    "${env:ProgramFiles}\Windows Kits\10\bin\*\x64\signtool.exe"
)
foreach ($pattern in $sdkPaths) {
    $found = Get-Item $pattern -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1
    if ($found) { $signtool = $found.FullName; break }
}

if (-not $signtool) {
    Write-Error "signtool.exe not found. Install the Windows SDK."
    exit 1
}
Write-Host "Using signtool: $signtool" -ForegroundColor Cyan

# --- Verify agent binary exists ---
if (-not (Test-Path $AgentPath)) {
    Write-Error "Agent binary not found at: $AgentPath`nBuild with: cargo build --release --target x86_64-pc-windows-msvc -p imaging-agent"
    exit 1
}

# --- Sign ---
$signArgs = @('/fd', 'SHA256', '/tr', $TimestampServer, '/td', 'SHA256')

if ($PSCmdlet.ParameterSetName -eq 'Thumbprint') {
    if (-not $CertThumbprint) {
        Write-Error "Provide -CertThumbprint or use -PfxPath instead."
        exit 1
    }
    $signArgs += @('/sha1', $CertThumbprint)
}
else {
    if (-not (Test-Path $PfxPath)) {
        Write-Error "PFX file not found: $PfxPath"
        exit 1
    }
    $signArgs += @('/f', $PfxPath)
    if ($PfxPassword) {
        $bstr = [System.Runtime.InteropServices.Marshal]::SecureStringToBSTR($PfxPassword)
        $plain = [System.Runtime.InteropServices.Marshal]::PtrToStringAuto($bstr)
        $signArgs += @('/p', $plain)
        [System.Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)
    }
}

$signArgs += $AgentPath

Write-Host "Signing: $AgentPath" -ForegroundColor Yellow
& $signtool sign @signArgs
if ($LASTEXITCODE -ne 0) {
    Write-Error "Signing failed (exit code $LASTEXITCODE)"
    exit $LASTEXITCODE
}

Write-Host "Verifying signature..." -ForegroundColor Yellow
& $signtool verify /pa /v $AgentPath
if ($LASTEXITCODE -ne 0) {
    Write-Warning "Signature verification returned non-zero — check output above."
}

Write-Host "`nAgent signed successfully." -ForegroundColor Green
