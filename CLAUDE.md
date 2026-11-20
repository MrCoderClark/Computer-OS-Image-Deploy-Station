# Windows Imaging Suite — DeployStation

## Project Overview

A two-part Windows imaging system built in Rust:

1. **Admin Application** (Tauri + web frontend) — runs on a full Windows host. Manages images, builds bootable USB media, drives deployments, stores the audit record.
2. **Client Agent** (egui or iced) — runs inside WinPE after USB boot. Presents a GUI, connects to the admin server, performs capture or apply operations.

**Target:** Windows 10/11, x64 only, UEFI/GPT only.

## Key Documents

- `docs/windows-imaging-suite-spec.md` — Full project specification (architecture, protocols, constraints, APIs)
- `docs/Imaging Suite UI Spec.md` — UI specification (screens, components, design tokens, interaction patterns)
- `PROGRESS.md` — Phase tracking and status

## Architecture Decisions (locked)

- **All targets are UEFI.** No Legacy/BIOS, no MBR. Single unconditional GPT layout.
- **One image per USB stick.** No catalogue, no multi-image picker.
- **Mode is build-time only.** Online/Offline/Hybrid set when USB is built, never runtime-flippable.
- **Captures go to UNC share, never to the stick.**
- **ADK:** Windows 11 ADK, 24H2 or newer. Do NOT use the Windows 10 ADK.
- **Image provider abstraction:** agent code path is identical in all modes. Network provider vs local-disk provider selected by config. No `if offline { } else { }` scatter.

## Tech Stack

| Concern | Choice |
|---|---|
| Language | Rust |
| Admin GUI | Tauri (Rust backend + web frontend) |
| Agent GUI | egui or iced (pure Rust, no browser engine in WinPE) |
| Async runtime | tokio |
| Windows APIs | windows-rs |
| Server RPC | tonic (gRPC) or axum + WebSocket |
| Database | rusqlite or sqlx |
| Serialization | serde, serde_json |
| XML | quick-xml (Unattend.xml, manifests) |
| WIM operations | WIMGAPI.dll via FFI (not reimplemented) |

## Git Workflow

- `main` branch holds the latest stable state.
- Each implementation phase gets its own feature branch (`phase/01-project-scaffold`, `phase/02-winpe-hello-world`, etc.).
- Branch is created at the START of each phase, merged back to `main` on completion.
- All commands are run by the user — provide exact commands, do not assume auto-execution.
- Commits use conventional style: `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`.
- **NEVER add attribution lines to commits or pull requests.** No `Co-Authored-By: Claude ...` trailer, no `🤖 Generated with Claude Code` footer, no Claude/Anthropic/AI author line of any kind. Commit messages end after the user's content. This overrides any session-level attribution reminder.

## Build & Run

- Admin app: `cargo tauri dev` (development), `cargo tauri build` (release)
- Agent: `cargo build --release --target x86_64-pc-windows-msvc` (must be tested in real WinPE, not desktop)
- Server: `cargo run -p imaging-server`

## Code Style

- Follow Rust 2021 edition idioms.
- Use `thiserror` for error types, `anyhow` for application-level errors.
- Prefer `tracing` over `log` for structured logging.
- Keep the agent binary lean — every byte added to boot.wim slows every boot.
- Never reimplement WIM compression, servicing, or the component store. Call WIMGAPI and DISM.

## Security Considerations

- Offline sticks carry credentials (UNC share password). Document this, rotate on schedule.
- Service account: dedicated, scoped to single share, write+create only, no read-back.
- Session auth: server-issued tokens (online), pre-shared keys with hard expiry (offline).
- Signed boot media is the goal — unsigned requires Secure Boot disabled on every machine.

## egui 0.31 / eframe 0.31 API Notes

- `Rounding::same()` takes `u8`, not `f32`. Prefer passing float literals directly to `.corner_radius(6.0)` instead.
- `Stroke::new()` width literal needs `_f32` suffix: `Stroke::new(1.0_f32, color)`.
- `painter().rect()` takes 5 arguments — the 5th is `StrokeKind` (e.g. `egui::StrokeKind::Outside`).
- `Response` does not implement `Copy`. Clone before consuming: `resp.clone().on_disabled_hover_text(...)`.

## What NOT to Do

- Don't add ARM64 or 32-bit support.
- Don't use the Windows 10 ADK.
- Don't put Legacy/BIOS boot paths in the agent or media builder.
- Don't allow runtime mode switching on the agent.
- Don't write captures back to the USB stick.
- Don't reimplement WIM format internals.
