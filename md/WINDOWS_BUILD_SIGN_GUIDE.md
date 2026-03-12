# Windows Build, Code Signing & Installer Guide

Complete step-by-step guide to build an NSIS installer on Windows and code-sign it with SSL.com eSigner cloud signing (no hardware token required).

---

## Prerequisites

- **Rust** installed via [rustup](https://rustup.rs/) with the `stable-x86_64-pc-windows-msvc` toolchain
- **Node.js** (v18+) and **npm**
- **SSL.com account** with an active code signing certificate and **eSigner cloud signing activated**
- **CodeSignTool** downloaded from [ssl.com/downloads](https://www.ssl.com/downloads/) (Windows version includes Java runtime)
- **eSigner TOTP secret** (obtained during eSigner enrollment — enables fully automated signing)

### Your Credentials

| Item | Value |
|------|-------|
| SSL.com Username | *(your SSL.com account email)* |
| SSL.com Password | *(your SSL.com account password)* |
| Credential ID | *(retrieve with `get_credential_ids` — see below)* |
| TOTP Secret | *(from eSigner QR code during enrollment — stored privately)* |

### Retrieve Your Credential ID

After activating eSigner on your certificate, find your credential ID:

```powershell
CodeSignTool.bat get_credential_ids -username=your@email.com -password="YourPassword"
# Output:
# Credential ID(s):
# - fe537ace-e132-52a9-c2e7-egcd2ac3f1e6
```

### Getting Your TOTP Secret

When you enroll your certificate in eSigner and scan the QR code with an authenticator app, SSL.com also displays a **secret code value**. Copy and save this string — it allows CodeSignTool to generate OTPs automatically without manual entry.

If you've already enrolled but didn't save the secret, you can re-enroll or contact SSL.com support.

---

## Step 1: Build the NSIS Installer

Tauri produces an NSIS installer on Windows by default:

```powershell
npm run tauri build
```

This produces:
- **Application binary**: `src-tauri\target\release\cracking-interview.exe`
- **NSIS installer**: `src-tauri\target\release\bundle\nsis\CrackingInterview_1.0.0_x64-setup.exe`

The `tauri.conf.json` already configures NSIS with `installMode: "both"` (per-user and per-machine) and a custom installer icon.

---

## Step 2: Sign the Application Binary

Sign the main `.exe` before the NSIS installer so that users see a valid signature when the app runs:

```powershell
CodeSignTool.bat sign ^
  -username=your@email.com ^
  -password="YourPassword" ^
  -credential_id=fe537ace-e132-52a9-c2e7-egcd2ac3f1e6 ^
  -totp_secret="YOUR_TOTP_SECRET" ^
  -input_file_path="src-tauri\target\release\cracking-interview.exe" ^
  -override="true"
```

Expected output: `Code signed successfully: ...cracking-interview.exe`

### Parameters Explained

| Parameter | Purpose |
|-----------|---------|
| `-username` / `-password` | SSL.com account credentials |
| `-credential_id` | Identifies which certificate to sign with |
| `-totp_secret` | Enables automated OTP generation (no manual entry) |
| `-input_file_path` | File to sign |
| `-override="true"` | Overwrite the original file with the signed version |

---

## Step 3: Sign the NSIS Installer

The installer `.exe` wrapper also needs to be signed — this is what Windows SmartScreen checks when users download and run it:

```powershell
CodeSignTool.bat sign ^
  -username=your@email.com ^
  -password="YourPassword" ^
  -credential_id=fe537ace-e132-52a9-c2e7-egcd2ac3f1e6 ^
  -totp_secret="YOUR_TOTP_SECRET" ^
  -input_file_path="src-tauri\target\release\bundle\nsis\CrackingInterview_1.0.0_x64-setup.exe" ^
  -override="true"
```

Expected output: `Code signed successfully: ...CrackingInterview_1.0.0_x64-setup.exe`

---

## Step 4: Verify Signatures

Use PowerShell to confirm both files are properly signed:

```powershell
Get-AuthenticodeSignature "src-tauri\target\release\cracking-interview.exe" | Format-List
Get-AuthenticodeSignature "src-tauri\target\release\bundle\nsis\CrackingInterview_1.0.0_x64-setup.exe" | Format-List
```

Expected output for each:

```
SignerCertificate  : [Subject]   CN=Cracking Interview LLC, O=Cracking Interview LLC, ...
                     [Issuer]    CN=SSL.com EV Code Signing Intermediate CA ...
Status             : Valid
StatusMessage      : Signature verified.
```

Alternatively, right-click either `.exe` in File Explorer → Properties → Digital Signatures tab to visually confirm.

---

## Step 5: Copy to Website for Distribution

```powershell
copy "src-tauri\target\release\bundle\nsis\CrackingInterview_1.0.0_x64-setup.exe" "C:\path\to\your\website\"
```

---

## Quick Reference: Full Pipeline

```powershell
# 1. Build NSIS installer
npm run tauri build

# 2. Sign the application binary
CodeSignTool.bat sign ^
  -username=your@email.com -password="YourPassword" ^
  -credential_id=YOUR_CREDENTIAL_ID ^
  -totp_secret="YOUR_TOTP_SECRET" ^
  -input_file_path="src-tauri\target\release\cracking-interview.exe" ^
  -override="true"

# 3. Sign the NSIS installer
CodeSignTool.bat sign ^
  -username=your@email.com -password="YourPassword" ^
  -credential_id=YOUR_CREDENTIAL_ID ^
  -totp_secret="YOUR_TOTP_SECRET" ^
  -input_file_path="src-tauri\target\release\bundle\nsis\CrackingInterview_1.0.0_x64-setup.exe" ^
  -override="true"

# 4. Verify
Get-AuthenticodeSignature "src-tauri\target\release\bundle\nsis\CrackingInterview_1.0.0_x64-setup.exe"

# 5. Copy to website
copy "src-tauri\target\release\bundle\nsis\CrackingInterview_1.0.0_x64-setup.exe" "C:\path\to\your\website\"
```

---

## Automated Script

An automated PowerShell script is available at `scripts/build-windows.ps1`. It handles the full pipeline with error checking:

```powershell
# Run from the project root:
.\scripts\build-windows.ps1
```

The script reads credentials from environment variables or prompts interactively. See the script header for details.

---

## Troubleshooting

### "Error: invalid otp"

- Ensure the TOTP secret matches the credential ID and SSL.com account
- If using a shared certificate, the TOTP secret must be from the account owner who enrolled in eSigner
- Check that your system clock is accurate (TOTP is time-based)

### Credential ID Not Found

Run `get_credential_ids` to list available certificates:

```powershell
CodeSignTool.bat get_credential_ids -username=your@email.com -password="YourPassword"
```

### SmartScreen Still Shows Warning

SmartScreen reputation is built over time. EV code signing certificates get immediate trust, while OV certificates may need download volume to build reputation. Your SSL.com certificate is organization-validated, which helps establish trust faster.

### CodeSignTool Not Found

Ensure the CodeSignTool directory is in your PATH, or use the full path:

```powershell
C:\path\to\CodeSignTool-v1.3.2-windows\CodeSignTool.bat sign ...
```

### Testing with SSL.com Sandbox

SSL.com provides a sandbox environment for testing. See [eSigner Demo Credentials](https://www.ssl.com/guide/esigner-demo-credentials-and-certificates/) for test account details.
