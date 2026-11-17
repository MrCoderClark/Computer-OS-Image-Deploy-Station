# Windows Imaging Suite — Progress Tracker

> Updated: 2026-11-17

## Build Order

Per spec §10, build in vertical slices. Each phase gets its own git branch.

---

### Phase 0: Project Foundation
**Branch:** `phase/00-project-foundation`
**Status:** Complete

- [x] Initialize Cargo workspace (multi-crate)
- [x] Create crate structure: `admin`, `agent`, `server`, `common`
- [x] Set up Tauri project for admin app
- [x] Configure shared dependencies in workspace Cargo.toml
- [x] Add CLAUDE.md, PROGRESS.md, .gitignore
- [x] Initial commit on main

---

### Phase 1: WinPE Hello World
**Branch:** `phase/01-winpe-hello-world`
**Status:** Complete

- [x] Build a minimal Rust binary that runs in WinPE
- [x] Create a WinPE boot image (ADK 24H2)
- [ ] Boot from USB and confirm the binary executes *(manual — requires ADK + VM)*
- [x] Add basic egui/iced window (renders in WinPE)
- [x] Document WinPE-specific gotchas and missing DLLs

---

### Phase 2: Network Connectivity
**Branch:** `phase/02-network-connectivity`
**Status:** Complete

- [x] Server skeleton (axum or tonic) — health endpoint
- [x] Agent connects to server, pulls image list
- [x] Discovery protocol (config fallback; DNS SRV + DHCP deferred)
- [ ] TLS setup for control channel *(deferred — HTTP for dev, TLS in Phase 11)*
- [x] Session authentication (server-issued token)

---

### Phase 3: Image Apply (End-to-End)
**Branch:** `phase/03-image-apply`
**Status:** Complete

- [x] Engine crate with disk partitioning (diskpart GPT: ESP + MSR + Windows)
- [x] WIMGAPI FFI bindings via libloading (apply + capture)
- [x] BCD boot configuration via bcdboot wrapper
- [x] Deploy pipeline (partition → apply → BCD → complete)
- [x] Capture pipeline (WIM capture to output path)
- [x] Agent wired to engine: confirm screen launches pipeline, progress screen shows live status
- [x] Server: image download endpoint stub, deploy status reporting endpoint
- [ ] Real image file storage and streaming download *(deferred — stub returns 204)*
- [ ] Resumable transfer with client-generated UUIDs *(deferred — needs real file I/O)*
- [ ] Full deploy on a blank disk — boot to Windows *(manual test in WinPE)*

---

### Phase 4: Configuration Contract
**Branch:** `phase/04-config-contract`
**Status:** Complete

- [x] Config schema with validation (schema_version, mode constraints, discovery rules)
- [x] Admin app Tauri commands: `build_config` and `validate_config`
- [x] Agent validates config at boot with structured error reporting
- [x] Mode detection already displayed (mode badge from Phase 1)
- [x] Image provider abstraction: `ImageProvider` trait with `LocalDiskProvider` and `NetworkProvider`
- [x] Provider factory (`create_provider`) selects based on mode, with Hybrid fallback logic
- [x] Agent confirm screen resolves WIM path through provider instead of hardcoding
- [x] Added `wim_path` field to `ImageMeta` for explicit WIM location override

---

### Phase 5: Capture Path
**Branch:** `phase/05-capture-path`
**Status:** Complete

- [x] UNC share probe before capture (net use with service account, reachability check)
- [x] WIM capture via WIMGAPI (from Phase 3 engine)
- [x] SHA-256 hashing of captured WIM via certutil
- [x] Local manifest written to logs/ (JSON with id, machine, hash, size, UNC path)
- [x] Enhanced capture pipeline: probe → capture → hash → manifest
- [x] Server reconciliation endpoint (POST /api/captures/reconcile)
- [x] Agent client reconcile_capture method
- [ ] VSS snapshot of live volume *(deferred — requires additional Win32 bindings)*
- [ ] Capture reconciliation UI in admin app *(deferred to Phase 6)*

---

### Phase 6: Admin App — Core UI
**Branch:** `phase/06-admin-core-ui`
**Status:** Complete

- [x] App shell: sidebar navigation with section headers, page routing
- [x] Dashboard: live stats from server (deploys, images, media, captures), server status indicator
- [x] Image catalogue: grid + list views with toggle, search, OS family filter
- [x] Image detail panel: click card to expand full metadata
- [x] Settings page: server connection, capture credentials, default mode
- [x] Tauri backend: get_dashboard_stats, get_images commands with server integration
- [x] Placeholder pages for Driver Packs, Task Sequences, Build Media, Deployments, Audit Log
- [x] Dark mode support via prefers-color-scheme
- [x] WinUI 3 design language (Segoe UI Variable, color tokens, card surfaces)

---

### Phase 7: Admin App — Media Builder
**Branch:** `phase/07-media-builder`
**Status:** Complete

- [x] USB device detection and enumeration (PowerShell Get-Volume)
- [x] Build Media wizard (5-step flow: Mode → Image → Target → Config → Build)
- [ ] Partition USB (FAT32 boot + optional NTFS payload) *(deferred — requires ADK elevated tools)*
- [ ] Copy WinPE boot.wim, agent binary, config.json *(deferred — build-winpe-media.ps1 handles this)*
- [x] Mode selection → config generation (reuses build_config command)
- [x] Progress reporting during build
- [x] Wizard frontend: step navigation, image picker, USB picker, summary, build trigger
- [x] Tauri backend: list_usb_devices, build_media commands

---

### Phase 8: Admin App — Task Sequences & Drivers
**Branch:** `phase/08-task-sequences-drivers`
**Status:** Complete

- [x] Task sequence data model (common crate: TaskSequence, TaskStep, StepType, StepConfig)
- [x] Step types: partition, apply_image, inject_drivers, run_script, domain_join, generate_unattend, reboot
- [x] Validation constraints (apply after partition, inject/unattend after apply)
- [x] Task sequence editor UI with reorder (up/down), enable/disable, add/remove steps
- [x] Driver pack data model (common crate: DriverPack with hardware_model, os_family, path)
- [x] Driver pack management UI (grid view, add form, delete)
- [x] Offline driver injection via DISM wrapper (engine crate: drivers.rs)
- [x] Unattend.xml generation via quick-xml (engine crate: unattend.rs, specialize + oobeSystem passes)
- [x] Unattend preview command in admin backend
- [x] Server API: CRUD endpoints for sequences and driver packs
- [x] Admin Tauri backend: get/save/delete sequences, get/save/delete driver packs, new_sequence, new_step, preview_unattend
- [x] Unit tests for Unattend.xml generation

---

### Phase 9: Deployment Monitoring & Audit
**Branch:** `phase/09-monitoring-audit`
**Status:** Complete

- [x] SQLite persistence for server (images, deploy_jobs, audit_log, sequences, driver_packs)
- [x] SQLite persistence for admin app (sequences, driver_packs, settings)
- [x] All CRUD operations backed by database (replaced in-memory stores)
- [x] Deploy job tracking: server stores status updates, admin fetches and displays table
- [x] Deployment monitoring UI: table with machine, image, status badge, progress bar, timestamps
- [x] Audit log: automatic entries on auth, deploy, capture, sequence/driver CRUD
- [x] Audit log viewer UI: search by text, filter by level, tabular display
- [x] Audit log CSV export (server endpoint + admin download)
- [x] Server seeds sample images on first run (empty database)
- [ ] Live WebSocket updates for deployment status *(deferred — polling via refresh button)*
- [ ] Concurrency and locking (claim a host mid-deploy) *(deferred)*
- [ ] Credential rotation reminders *(deferred)*

---

### Phase 10: Agent UI Polish
**Branch:** `phase/10-agent-ui-polish`
**Status:** Complete

- [x] Main screen: mode badge, connectivity dot, image panel, two action buttons
- [x] Confirm screen: disk details, typed confirmation gate
- [x] Progress screen: phase tracker, throughput, log tail
- [x] Error screen: full-screen with actionable message + retry
- [x] Reconnection overlay for network drops
- [x] Offline fallback display (Hybrid mode)

---

### Phase 11: Security & Signing
**Branch:** `phase/11-security-signing`
**Status:** Not started

- [ ] Secure Boot investigation and decision
- [ ] EV code-signing cert acquisition (if applicable)
- [ ] Signed bootx64.efi for WinPE media
- [ ] Pre-shared enrollment key with hard expiry
- [ ] Key revocation mechanism
- [ ] Credential rotation workflow in admin app

---

### Phase 12: Testing & Hardening
**Branch:** `phase/12-testing-hardening`
**Status:** Not started

- [ ] Unit tests for config parsing, image provider, partition logic
- [ ] Integration tests against real WinPE environment
- [ ] Multi-model hardware testing (HP, Dell, Lenovo)
- [ ] DPI scaling tests (100%, 125%, 150%)
- [ ] Network failure and resume testing
- [ ] Offline mode full cycle test
- [ ] Hybrid fallback path testing

---

## Open Questions (from spec §12)

| # | Question | Status |
|---|---|---|
| 1 | Signed or unsigned boot media? | **Open** — decides Phase 11 scope |
| 2 | Offline capture in scope for v1? | **Open** — affects Phase 5 |
| 3 | Max capture size for shared drive? | **Open** — sizing/retention |
| 4 | PXE / WDS network boot in scope? | **Open** — post-USB stretch goal |

---

## Notes

- Phase order follows spec §10: vertical slice first, then breadth.
- Phases 1–3 are the critical learning path — WinPE constraints, network, and end-to-end apply.
- Phases 6–10 can partially overlap once the core pipeline works.
- Phase 11 (Secure Boot) has the longest external lead time — start investigation early.
