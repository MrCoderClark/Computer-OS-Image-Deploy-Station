# Secure Boot & Code Signing — Investigation Notes

## Status: Investigation / Design Phase

This document captures decisions and open questions around Secure Boot
support and code signing for the DeployStation imaging suite.

## Background

The agent binary runs inside WinPE after USB boot. By default, Secure
Boot trusts only Microsoft-signed binaries. Our agent is a custom EXE,
which means one of the following must happen for it to run on a
Secure-Boot-enabled machine:

1. **Disable Secure Boot** on every target machine (simplest, least secure).
2. **Sign with a Microsoft-trusted certificate** (expensive, long lead time).
3. **Enroll a custom Secure Boot key (MOK / db)** on each target machine.
4. **Sign the boot media's bootloader (shim/grub)**, leave the OS-level
   agent unsigned (WinPE's own loader is already signed by Microsoft).

## Current Decision

For Phase 11 we document the tradeoffs and provide a signing-script
scaffold. Actual certificate procurement and signing integration are
Phase 12+ work.

### Recommended path

Option 4 is the pragmatic choice for most enterprise deployments:

- WinPE's boot chain (bootmgfw.efi -> winload.efi) is Microsoft-signed.
  Secure Boot allows WinPE to boot without modification.
- The agent binary runs *inside* WinPE as a user-mode process. Secure
  Boot validates the boot chain, not every EXE that runs after boot.
- Therefore: **Secure Boot does NOT need to be disabled for the agent to
  run, as long as the WinPE boot media itself is built from the official
  ADK and not tampered with post-build.**

### When signing IS required

If the organization modifies boot.wim or replaces the bootloader,
those components must be signed. The `sign-agent.ps1` scaffold below
covers signing the agent EXE with a code-signing certificate (Authenticode),
which is good practice for integrity even though Secure Boot does not
enforce it at the user-mode level.

## Signing Script Scaffold

See `scripts/sign-agent.ps1` for the scaffold. It expects:

- A PFX file or certificate thumbprint in the local machine store.
- `signtool.exe` from the Windows SDK.
- The built agent binary at `target/x86_64-pc-windows-msvc/release/imaging-agent.exe`.

## Certificate Options

| Option | Cost | Lead Time | Notes |
|--------|------|-----------|-------|
| Self-signed (test only) | Free | Immediate | Not trusted by default |
| Internal CA (AD CS) | Free (infra cost) | Days | Trusted on domain machines |
| Public CA (EV code signing) | ~$400/yr | 1-4 weeks | Trusted everywhere, SmartScreen |
| Microsoft WHQL | ~$500 + HLK | Weeks | Required for kernel drivers, overkill for us |

## Open Questions

- [ ] Does the organization have an internal CA (AD CS) for code signing?
- [ ] Is there a hardware security module (HSM) requirement for private keys?
- [ ] Should boot.wim integrity be validated with a detached signature?
- [ ] Timeline for procuring a public code-signing certificate?
