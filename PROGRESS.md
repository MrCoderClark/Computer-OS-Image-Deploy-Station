# Windows Imaging Suite — Progress Tracker

> Updated: 2026-11-17

## Build Order

Per spec §10, build in vertical slices. Each phase gets its own git branch.

---

### Phase 0: Project Foundation
**Branch:** `phase/00-project-foundation`
**Status:** Not started

- [ ] Initialize Cargo workspace (multi-crate)
- [ ] Create crate structure: `admin`, `agent`, `server`, `common`
- [ ] Set up Tauri project for admin app
- [ ] Configure shared dependencies in workspace Cargo.toml
- [ ] Add CLAUDE.md, PROGRESS.md, .gitignore
- [ ] Initial commit on main

---

### Phase 1: WinPE Hello World
**Branch:** `phase/01-winpe-hello-world`
**Status:** Not started

- [ ] Build a minimal Rust binary that runs in WinPE
- [ ] Create a WinPE boot image (ADK 24H2)
- [ ] Boot from USB and confirm the binary executes
- [ ] Add basic egui/iced window (renders in WinPE)
- [ ] Document WinPE-specific gotchas and missing DLLs

---

### Phase 2: Network Connectivity
**Branch:** `phase/02-network-connectivity`
**Status:** Not started

- [ ] Server skeleton (axum or tonic) — health endpoint
- [ ] Agent connects to server, pulls image list
- [ ] Discovery protocol (DNS SRV → DHCP → config fallback)
- [ ] TLS setup for control channel
- [ ] Session authentication (server-issued token)

---

### Phase 3: Image Apply (End-to-End)
**Branch:** `phase/03-image-apply`
**Status:** Not started

- [ ] Partition target disk (GPT: ESP + MSR + Windows)
- [ ] Apply WIM image to Windows partition via WIMGAPI
- [ ] Configure BCD boot entry
- [ ] Data channel for bulk image transfer (chunked, hashed)
- [ ] Resumable transfer with client-generated UUIDs
- [ ] Full deploy on a blank disk — boot to Windows

---

### Phase 4: Configuration Contract
**Branch:** `phase/04-config-contract`
**Status:** Not started

- [ ] Define config.json schema (serde types)
- [ ] Admin app writes config.json to USB
- [ ] Agent reads and validates config.json at boot
- [ ] Mode detection and display (Online/Offline/Hybrid)
- [ ] Image provider abstraction (network vs local-disk)

---

### Phase 5: Capture Path
**Branch:** `phase/05-capture-path`
**Status:** Not started

- [ ] UNC share probe at agent startup
- [ ] VSS snapshot of live volume
- [ ] WIM capture via WIMGAPI
- [ ] Upload to UNC share (never to stick)
- [ ] Local manifest written to stick logs/
- [ ] Capture reconciliation in admin app

---

### Phase 6: Admin App — Core UI
**Branch:** `phase/06-admin-core-ui`
**Status:** Not started

- [ ] App shell: sidebar navigation, titlebar, routing
- [ ] Dashboard: stats, recent activity, alerts
- [ ] Image catalogue: grid/list view, search, filter
- [ ] Image detail panel
- [ ] Settings page (server config, credentials, preferences)

---

### Phase 7: Admin App — Media Builder
**Branch:** `phase/07-media-builder`
**Status:** Not started

- [ ] USB device detection and enumeration
- [ ] Build Media wizard (5-step flow)
- [ ] Partition USB (FAT32 boot + optional NTFS payload)
- [ ] Copy WinPE boot.wim, agent binary, config.json
- [ ] Mode selection → config generation
- [ ] Progress reporting during build

---

### Phase 8: Admin App — Task Sequences & Drivers
**Branch:** `phase/08-task-sequences-drivers`
**Status:** Not started

- [ ] Task sequence data model and editor UI
- [ ] Step types: partition, apply, inject, script, domain join, reboot
- [ ] Drag-to-reorder with validation constraints
- [ ] Driver pack management (per-model, tagged by OS family)
- [ ] Offline driver injection into applied image
- [ ] Unattend.xml generation (computer name, domain, locale, key)

---

### Phase 9: Deployment Monitoring & Audit
**Branch:** `phase/09-monitoring-audit`
**Status:** Not started

- [ ] Deployment tracking table (live WebSocket updates)
- [ ] Concurrency and locking (claim a host mid-deploy)
- [ ] Audit log: who imaged what, when, from which image
- [ ] Audit log search, filter, CSV export
- [ ] Credential rotation reminders
- [ ] Capture reconciliation from shared drive

---

### Phase 10: Agent UI Polish
**Branch:** `phase/10-agent-ui-polish`
**Status:** Not started

- [ ] Main screen: mode badge, connectivity dot, image panel, two action buttons
- [ ] Confirm screen: disk details, typed confirmation gate
- [ ] Progress screen: phase tracker, throughput, log tail
- [ ] Error screen: full-screen with actionable message + retry
- [ ] Reconnection overlay for network drops
- [ ] Offline fallback display (Hybrid mode)

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
