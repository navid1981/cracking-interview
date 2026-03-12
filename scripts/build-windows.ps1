#Requires -Version 5.1
<#
.SYNOPSIS
    Build, sign, and package CrackingInterview for Windows.

.DESCRIPTION
    Automates the full pipeline:
      1. Build NSIS installer via Tauri
      2. Sign the application .exe with SSL.com eSigner (CodeSignTool)
      3. Sign the NSIS installer .exe
      4. Verify both signatures

.NOTES
    Credentials can be provided via environment variables or interactive prompts:
      $env:SSL_USERNAME       - SSL.com account email
      $env:SSL_PASSWORD       - SSL.com account password
      $env:SSL_CREDENTIAL_ID  - eSigner credential ID
      $env:SSL_TOTP_SECRET    - eSigner TOTP secret for automated OTP
      $env:CODESIGNTOOL_PATH  - Path to CodeSignTool.bat (if not in PATH)
#>

$ErrorActionPreference = "Stop"

# ── Constants ────────────────────────────────────────────────────────────────
$AppName    = "CrackingInterview"
$TauriConf  = "src-tauri\tauri.conf.json"

# ── Helpers ──────────────────────────────────────────────────────────────────
function Step($num, $msg) { Write-Host "`n>> Step ${num}: $msg" -ForegroundColor Cyan }
function Ok($msg)         { Write-Host "  [OK] $msg" -ForegroundColor Green }
function Fail($msg)       { Write-Host "  [FAIL] $msg" -ForegroundColor Red; exit 1 }

# ── Pre-flight checks ───────────────────────────────────────────────────────
if (-not (Test-Path $TauriConf)) {
    Fail "$TauriConf not found. Run this script from the project root."
}

$conf = Get-Content $TauriConf -Raw | ConvertFrom-Json
$Version = $conf.version
if (-not $Version) { Fail "Could not read version from $TauriConf" }

$BundleDir    = "src-tauri\target\release\bundle"
$ExePath      = "src-tauri\target\release\$($AppName.ToLower() -replace ' ','-').exe"
$ExePath      = "src-tauri\target\release\cracking-interview.exe"
$InstallerPath = "$BundleDir\nsis\${AppName}_${Version}_x64-setup.exe"

Write-Host ""
Write-Host "================================================" -ForegroundColor White
Write-Host "  $AppName v$Version - Windows Build & Sign Pipeline"
Write-Host "================================================" -ForegroundColor White

# ── Resolve CodeSignTool ─────────────────────────────────────────────────────
$CodeSignTool = if ($env:CODESIGNTOOL_PATH) { $env:CODESIGNTOOL_PATH } else { "CodeSignTool.bat" }

try {
    & $CodeSignTool --version 2>&1 | Out-Null
} catch {
    # Check common install locations
    $commonPaths = @(
        "$env:USERPROFILE\Desktop\CodeSignTool-v1.3.2-windows\CodeSignTool.bat",
        "$env:USERPROFILE\Downloads\CodeSignTool-v1.3.2-windows\CodeSignTool.bat",
        "C:\CodeSignTool\CodeSignTool.bat"
    )
    $found = $false
    foreach ($p in $commonPaths) {
        if (Test-Path $p) {
            $CodeSignTool = $p
            $found = $true
            break
        }
    }
    if (-not $found) {
        Fail "CodeSignTool not found. Set `$env:CODESIGNTOOL_PATH or add it to PATH.`nDownload from: https://www.ssl.com/downloads/"
    }
}
Write-Host "  CodeSignTool: $CodeSignTool"

# ── Collect credentials ──────────────────────────────────────────────────────
$SslUsername = if ($env:SSL_USERNAME) { $env:SSL_USERNAME } else {
    Read-Host "Enter SSL.com username (email)"
}
if (-not $SslUsername) { Fail "SSL.com username is required" }

$SslPassword = if ($env:SSL_PASSWORD) { $env:SSL_PASSWORD } else {
    $secPwd = Read-Host "Enter SSL.com password" -AsSecureString
    [Runtime.InteropServices.Marshal]::PtrToStringAuto(
        [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secPwd))
}
if (-not $SslPassword) { Fail "SSL.com password is required" }

$SslCredentialId = if ($env:SSL_CREDENTIAL_ID) { $env:SSL_CREDENTIAL_ID } else {
    Read-Host "Enter eSigner credential ID"
}
if (-not $SslCredentialId) { Fail "Credential ID is required" }

$SslTotpSecret = if ($env:SSL_TOTP_SECRET) { $env:SSL_TOTP_SECRET } else {
    $secTotp = Read-Host "Enter eSigner TOTP secret" -AsSecureString
    [Runtime.InteropServices.Marshal]::PtrToStringAuto(
        [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secTotp))
}
if (-not $SslTotpSecret) { Fail "TOTP secret is required" }

# ── Step 1: Build NSIS installer ─────────────────────────────────────────────
Step 1 "Build NSIS installer"

npm run tauri build
if ($LASTEXITCODE -ne 0) { Fail "Tauri build failed" }

if (-not (Test-Path $ExePath)) { Fail "Build failed - $ExePath not found" }
if (-not (Test-Path $InstallerPath)) { Fail "Build failed - $InstallerPath not found" }
Ok "Built $InstallerPath"

# ── Step 2: Sign the application binary ──────────────────────────────────────
Step 2 "Sign application binary ($ExePath)"

& $CodeSignTool sign `
    -username="$SslUsername" `
    -password="$SslPassword" `
    -credential_id="$SslCredentialId" `
    -totp_secret="$SslTotpSecret" `
    -input_file_path="$ExePath" `
    -override="true"
if ($LASTEXITCODE -ne 0) { Fail "Failed to sign $ExePath" }
Ok "Application binary signed"

# ── Step 3: Sign the NSIS installer ──────────────────────────────────────────
Step 3 "Sign NSIS installer ($InstallerPath)"

& $CodeSignTool sign `
    -username="$SslUsername" `
    -password="$SslPassword" `
    -credential_id="$SslCredentialId" `
    -totp_secret="$SslTotpSecret" `
    -input_file_path="$InstallerPath" `
    -override="true"
if ($LASTEXITCODE -ne 0) { Fail "Failed to sign $InstallerPath" }
Ok "NSIS installer signed"

# ── Step 4: Verify signatures ────────────────────────────────────────────────
Step 4 "Verify signatures"

$exeSig = Get-AuthenticodeSignature $ExePath
if ($exeSig.Status -ne "Valid") {
    Fail "Application binary signature invalid: $($exeSig.StatusMessage)"
}
Write-Host "  $ExePath"
Write-Host "    Subject : $($exeSig.SignerCertificate.Subject)"
Write-Host "    Status  : $($exeSig.Status)"
Ok "Application binary signature verified"

$instSig = Get-AuthenticodeSignature $InstallerPath
if ($instSig.Status -ne "Valid") {
    Fail "Installer signature invalid: $($instSig.StatusMessage)"
}
Write-Host "  $InstallerPath"
Write-Host "    Subject : $($instSig.SignerCertificate.Subject)"
Write-Host "    Status  : $($instSig.Status)"
Ok "Installer signature verified"

# ── Done ─────────────────────────────────────────────────────────────────────
$fileSize = "{0:N1} MB" -f ((Get-Item $InstallerPath).Length / 1MB)

Write-Host ""
Write-Host "================================================" -ForegroundColor White
Write-Host "  Done! Signed installer ready:"
Write-Host "  $InstallerPath ($fileSize)"
Write-Host "================================================" -ForegroundColor White
