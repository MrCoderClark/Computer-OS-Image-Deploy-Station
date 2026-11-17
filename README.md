# DeployStation — Windows Imaging Suite

A two-part Windows imaging system built in Rust for enterprise deployment workflows.

## What It Does

**Admin Console** — A Tauri desktop application for managing WIM images, building bootable USB media, orchestrating deployments, and maintaining a full audit trail.

**Field Agent** — A lightweight Rust GUI that boots from USB inside WinPE. Connects to the admin server (or runs offline), and performs image capture or apply operations against bare-metal hardware.

The USB stick is a thin boot shim — it delivers WinPE, the agent binary, and a configuration file. Image payload travels over the network in online mode, or sits on the stick in offline mode. Hybrid mode tries the server first and falls back to the stick.

## Architecture

```
USB (FAT32 boot partition)
  └── /EFI/Boot/bootx64.efi
        └── /sources/boot.wim        (WinPE + Rust agent)
              └── startnet.cmd       (maps capture share, launches agent)
                    └── agent.exe    (GUI + network + imaging engine)
```

- **UEFI only** — no Legacy/BIOS, no MBR. Single unconditional GPT layout.
- **One image per stick** — mode and image are build-time properties.
- **Three modes:** Online (stream from server), Offline (read from stick), Hybrid (server with stick fallback).

## Tech Stack

| Component | Technology |
|---|---|
| Language | Rust (2021 edition) |
| Admin GUI | Tauri (Rust + WebView2 frontend) |
| Agent GUI | egui or iced (pure Rust, no browser engine) |
| Async | tokio |
| Windows APIs | windows-rs |
| RPC | tonic (gRPC) or axum + WebSocket |
| Database | rusqlite or sqlx |
| WIM Operations | WIMGAPI.dll via FFI |

## Target Platform

- Windows 10 / 11, x64 only
- Windows 11 ADK (24H2+) for WinPE builds
- UEFI firmware (Secure Boot support planned)

## Project Structure

```
DeployStation/
├── docs/                      # Specifications and UI design
│   ├── windows-imaging-suite-spec.md
│   └── Imaging Suite UI Spec.md
├── CLAUDE.md                  # AI assistant project context
├── PROGRESS.md                # Phase tracking
└── README.md
```

Workspace crates (planned):
- `admin` — Tauri admin console
- `agent` — WinPE field agent
- `server` — Imaging server
- `common` — Shared types, config, protocols

## Development

```powershell
# Admin app (requires WebView2 runtime)
cargo tauri dev

# Agent (test binary — real testing requires WinPE)
cargo build --release -p agent

# Server
cargo run -p server
```

## Documentation

- [Project Specification](docs/windows-imaging-suite-spec.md) — Full architecture, protocols, constraints, and API reference
- [UI Specification](docs/Imaging%20Suite%20UI%20Spec.md) — Screen mockups, design tokens, component specs, interaction patterns

## Status

**Design phase** — requirements converged, implementation not started. See [PROGRESS.md](PROGRESS.md) for the phase-by-phase build plan.

## License

Proprietary. All rights reserved.
