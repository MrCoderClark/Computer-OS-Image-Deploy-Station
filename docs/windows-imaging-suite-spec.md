# Windows Imaging Suite — Project Specification

**Target platform:** Microsoft Windows 10 / 11, x64
**Primary language:** Rust
**Status:** Design phase — requirements converged, implementation not started
**Last updated:** 2026-11-17

---

## 1. Purpose

Build a two-part imaging system:

1. **Admin application** — runs on a full Windows host. Manages images, builds bootable USB media, drives deployments, stores the audit record.
2. **Client agent** — runs inside WinPE after booting from the USB. Presents a GUI, connects to the admin server, and performs the capture or apply operation.

The USB is a **thin boot shim**, not the imaging engine. It delivers WinPE, the agent binary, and a configuration file. Image payload travels over the network in online mode, or sits on the stick in offline mode.

---

## 2. Hard Constraints (locked)

| Constraint | Value | Consequence |
|---|---|---|
| Target OS | Windows 10 + 11, x64 only | One WinPE build images both. No 32-bit, no ARM64. |
| ADK | Windows 11 ADK, 24H2 or newer | WinPE baseline 10.0.26100.x. Do **not** use the Windows 10 ADK. |
| Firmware | **All targets are UEFI** | No Legacy/BIOS path. No MBR. Single unconditional GPT layout. |
| Partition style | GPT only | ESP (FAT32, ~300 MB) + MSR (16 MB) + Windows partition. |
| Images per stick | Exactly one | No catalogue, no dedupe, no multi-image picker. |
| Mode toggle | Admin app only, build-time | Changing mode requires rebuilding the USB. Never runtime-flippable. |
| Capture destination | Admin app's shared drive (UNC) or server | Captures never write back to the stick. Stick stays effectively read-only. |

**Rust rationale:** memory safety matters when writing raw sectors — the bug class that turns an imaging tool into a data-loss tool is largely eliminated. `windows-rs` provides direct access to the same Win32 / WinRT surface a C# or C++ implementation would use.

---

## 3. System Architecture

### 3.1 Boot chain

```
USB (FAT32 boot partition)
  └── /EFI/Boot/bootx64.efi
        └── /sources/boot.wim          (WinPE + Rust agent)
              └── startnet.cmd         (maps capture share, launches agent)
                    └── agent.exe      (GUI + network + imaging engine)
```

UEFI only — `bootx64.efi` is the single entry point. **No** `bootmgr`, **no** BIOS BCD, **no** firmware branching.

### 3.2 USB layout

```
Partition 1 — FAT32 (UEFI boot, small)
  /EFI/Boot/bootx64.efi
  /sources/boot.wim                 WinPE + agent
  /imaging/config.json              see §6

Partition 2 — NTFS (payload, OPTIONAL)
  /images/deploy.wim                present in offline/hybrid mode only
  /logs/                            local audit spool
```

Partition 2 is **empty in online mode**. One USB template produces all modes; only the config and payload contents differ.

### 3.3 Operating modes

Mode is a **build-time property of the stick**, written into `config.json` by the admin app.

| Mode | Deploy (apply) | Capture (create) | Stick contents |
|---|---|---|---|
| **online** | Stream from server | Upload to server | boot.wim + config |
| **offline** | Read from stick | Write to admin shared drive (UNC) | boot.wim + config + deploy.wim |
| **hybrid** | Try server, fall back to stick | Server if reachable, else UNC share | boot.wim + config + deploy.wim |

**Design rule:** the agent's code path is identical in all modes. It talks to an **image provider abstraction** with two implementations — a network provider and a local-disk provider. The config selects the provider. Never scatter `if offline { ... } else { ... }` through the apply or capture logic.

**Hybrid** is the recommended default for shipping media.

---

## 4. Agent ↔ Server Protocol

### 4.1 Discovery

In preference order:

1. **DNS service record** (`_imaging._tcp`) queried on the local subnet — survives DHCP changes. Preferred.
2. **DHCP option 66/67** — classic PXE-style, works in most managed networks.
3. **Fixed host in `config.json`** — fallback for odd topologies.
4. **UDP broadcast discovery** — runtime fallback; agent caches last-known-good server so a re-IP'd lab doesn't brick every stick.

### 4.2 Transport

Split control from bulk transfer:

- **Control channel** — gRPC over HTTP/2 with TLS (`tonic`), or JSON over WebSocket. Authenticate, list images, start job, report progress, stream log lines.
- **Data channel** — raw TCP, chunked and hashed. **Never** route multi-gigabyte payloads through RPC framing.

### 4.3 Session auth

- **Online:** server-issued session token.
- **Offline:** pre-shared enrolment key with a **hard expiry** and revocability. An offline stick holds a bootable device that can write images and join machines to the domain — treat it as a security asset, not a convenience.

### 4.4 Resumable transfer

Every image and every log entry gets a **client-generated UUID**. A half-finished upload resumes rather than restarts. Local audit entries carry a **monotonic sequence number per device**; on reconnection the server accepts them if the sequence is contiguous and the enrolment key hasn't expired.

---

## 5. Capture Path

Captures go to the admin shared drive as a UNC path — **never** to the stick. This removes the re-capture-into-a-full-stick problem entirely.

### 5.1 Credentials

WinPE has no interactive logon, so credentials must live on the stick and be applied before the agent starts:

```
net use \\fileserver\imaging-captures /user:LAB\svc-imaging <password>
```

**Security posture (no way to make this fully safe — make it uninteresting to abuse):**

- Dedicated service account. **Not** a domain admin.
- Scoped to a single share: **write + create only**, no read back, no traverse elsewhere.
- Rotated on a schedule the admin app reminds you about.
- Anyone who finds the stick can read the password. Document this.

### 5.2 Share reachability

The agent must **probe the share before starting a capture**, not two hours in. If unreachable, refuse to start and say why. Test at agent startup.

### 5.3 Local manifest

The agent writes a small JSON manifest locally (stick `logs/` or temp) describing what was captured, so the admin app can reconcile when the drive next syncs to the server.

---

## 6. Configuration Contract

Written by the admin app, read by the agent. This file is **the contract that holds the system together** — pin it down before writing code.

```json
{
  "schema_version": 1,
  "mode": "online",
  "server": {
    "discovery": "dns",
    "host": "imaging.lab.example.com",
    "port": 7443
  },
  "capture_target": {
    "type": "share",
    "unc_path": "\\\\fileserver\\imaging-captures",
    "account": "LAB\\svc-imaging"
  },
  "image": {
    "name": "Win11-24H2-Base",
    "os_family": "win11",
    "captured_from": "HP EliteBook 840 G9",
    "build_date": "2026-11-10",
    "size_bytes": 21474836480,
    "sha256": "…"
  },
  "built_by": "admin.lab.example",
  "built_at": "2026-11-17T14:02:00Z"
}
```

Notes:

- `mode` is one of `"online" | "offline" | "hybrid"`.
- The `image` block is **absent in online-only mode** — the catalogue comes from the server instead.
- With an all-UEFI fleet, `os_family` is the only compatibility field worth carrying. It earns its place by letting the admin app warn when someone captures a Win10 machine into a slot intended for the Win11 baseline.
- The agent must **log and display which mode it booted in** — a technician holding two sticks needs to tell them apart without plugging into a running machine.

---

## 7. Admin Application

### 7.1 Features

- **Build bootable USB media** — choose mode → build media. Same agent codebase in all modes; only config and partition contents differ.
- **Driver injection** — per-model driver packs, injected offline into the applied image. Tagged by OS family (Win10 vs Win11 driver models diverge in practice).
- **Task sequences** — ordered steps: partition, apply, inject, run script, join domain, reboot.
- **Unattend.xml generation** — computer name, domain join, locale, product key handled per-job.
- **Image catalogue** — one image per stick, but the server holds many: versioning, source hardware fingerprint, size, last used, OS family.
- **Concurrency and locking** — a host mid-deploy must be claimable; two technicians must not apply to the same machine.
- **Audit log** — who imaged what, when, from which image. Non-negotiable in any managed environment.
- **Capture reconciliation** — pull manifests and images off the shared drive, register them in the catalogue.
- **Credential rotation reminders** — for the capture service account.

### 7.2 Stack

- **Tauri** — Rust backend, web frontend. Best UI quality-per-effort, small binaries. Runs on a full Windows host with WebView2 available. **This is the recommended choice.**
- `tokio` for async runtime.
- `rusqlite` or `sqlx` for the catalogue and audit store.
- `serde` + `quick-xml` for manifests and Unattend.xml.

---

## 8. Client Agent (WinPE)

### 8.1 Constraints

- **WinPE is a stripped recovery environment.** Every megabyte added to `boot.wim` slows boot on every machine, every time. Keep the client lean.
- WinPE ships modern .NET and a reasonably current browser engine, but **not WebView2 by default**.
- WinPE's API surface is narrower than full Windows. **Test against real WinPE, not a desktop.** Missing DLLs are the most common failure here.

### 8.2 GUI choice

- Bundling WebView2 into `boot.wim` is possible but adds size.
- **Recommended:** write the agent GUI in **egui** or **iced** — pure Rust, no browser engine added to `boot.wim`.
- Put the visual polish in the **admin console** (Tauri). Keep the client screen clean, fast, unambiguous.

### 8.3 Screens

1. **Header** — mode indicator (Online / Offline / Hybrid), server reachability dot, target machine name.
2. **Image panel** — the one image: name, OS family, build date, size, hash status (verified / not yet checked).
3. **Two actions** — *Image this computer* and *Create image from this computer* as large, unambiguous buttons. No nesting, no wizard unless the task sequence needs parameters.
4. **Progress view** — phase name, percentage, throughput, elapsed, expandable log tail.
5. **Confirm step** — which disk is about to be written, which image, and a typed or held confirmation. Destructive operations deserve friction.

### 8.4 Reference APIs (via `windows-rs`)

| Task | API |
|---|---|
| Volume snapshot (live system) | VSS — Volume Shadow Copy Service |
| Raw disk access | `CreateFile` / `ReadFile` on `\\.\PhysicalDriveN` |
| Volume geometry | `DeviceIoControl` with `FSCTL_*` codes |
| Partition layout | `IOCTL_DISK_SET_DRIVE_LAYOUT_EX_EX`, GPT parsing |
| WIM capture / apply | `WIMGAPI.dll` — `WIMCreateFile`, `WIMCaptureImage`, `WIMApplyImage`, `WIMSetReferenceFile` (plain C DLL, trivial `extern "C"` bindings) |
| BCD | `BcdOpenSystemStore` / `Bcd*` APIs |
| Disk prep | Virtual Disk Service (`IVdsService`) COM, or direct IOCTL |
| Unattend.xml | `serde` + `quick-xml` |

### 8.5 Partition step (single unconditional path)

UEFI/GPT only — no branching:

1. Wipe target disk, write GPT.
2. ESP — FAT32, ~300 MB.
3. MSR — 16 MB.
4. Windows partition — remainder.
5. `bcdboot C:\Windows /s S: /f UEFI`.

---

## 9. Secure Boot (OPEN — decide early)

**This is the remaining unknown with the longest lead time.**

- **Signed media** (EV code-signing cert + Microsoft attestation for anything kernel-adjacent) → boots on a Secure Boot machine with no intervention. **This is where you want to end up.**
- **Unsigned media** → every machine needs Secure Boot disabled. On many modern machines that means entering firmware setup; on some, a recovery-key prompt. That's a manual step on **every** deployment, and it quietly kills adoption.

A Windows 11 fleet almost certainly has Secure Boot on (it's a Win11 requirement). **If you are deploying Win11, assume you need signed media and start that process immediately** — it has lead time and sits on the critical path.

---

## 10. Build Order (vertical slice first)

Build in this order. Step 1 will teach you more about WinPE's limits than any amount of design, and it's a day's work rather than a month's.

1. **A WinPE boot image that boots from USB and runs a Hello World Rust binary.**
2. **That binary connecting to a Rust server over the network and pulling a list of images.**
3. **Apply one image to a blank disk end-to-end.**
4. Only then: capture, driver injection, task sequences, UI polish.

---

## 11. Reference Stack Summary

| Concern | Crate / tool |
|---|---|
| Windows APIs | `windows-rs` |
| Async runtime | `tokio` |
| Server RPC | `tonic` (gRPC) or `axum` + WebSocket |
| Catalogue / audit | `rusqlite` or `sqlx` |
| Serialization | `serde`, `serde_json` |
| XML (Unattend, manifests) | `quick-xml` |
| Admin GUI | Tauri (Rust + web frontend) |
| Agent GUI | `egui` or `iced` |
| Boot media build | Rust binary invoking the ADK's `MakeWinPEMedia`, `dism`, `oscdimg` |

**Note on non-Rust dependencies:** WIM's compression codecs (XPRESS Huffman, LZX, LZMS), the servicing stack (`CBS`), and component store servicing are **not** reimplemented. Call `WIMGAPI` and `DISM`. Reimplementing the imaging stack itself is years of work and Rust changes nothing about its size. This project is: your own logic, your own UI, calling documented Windows APIs.

---

## 12. Open Questions

| # | Question | Why it matters |
|---|---|---|
| 1 | **Signed or unsigned boot media?** | Decides whether a technician touches firmware settings on every machine. Has the longest lead time. Decide first. |
| 2 | Is offline **capture** in scope for v1, or deploy-only? | Simplifies the shipping media and the reconciliation logic. |
| 3 | What's the maximum capture size the shared drive must absorb? | Sizing and retention policy. |
| 4 | Does the admin app need PXE / WDS network boot as well? | Once the USB path works, network boot is a small step and removes the stick entirely. Scope question. |

---

## 13. Glossary

- **WinPE** — Windows Preinstallation Environment. A minimal Windows recovery/boot environment. Where the agent runs.
- **WIM** — Windows Imaging Format. The image file format. Compressed, file-based.
- **ADK** — Assessment and Deployment Kit. Microsoft's toolkit containing WinPE, `dism`, and `MakeWinPEMedia`.
- **ESP** — EFI System Partition. FAT32, holds the bootloader on a UEFI/GPT disk.
- **MSR** — Microsoft Reserved Partition. 16 MB, required on GPT disks for Windows.
- **VSS** — Volume Shadow Copy Service. Takes a consistent point-in-time snapshot of a live volume.
- **BCD** — Boot Configuration Data. Windows' bootloader configuration store.
- **Unattend.xml** — Microsoft's answer file for unattended Windows setup.
