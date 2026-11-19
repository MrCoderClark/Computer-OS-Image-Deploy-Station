# Windows Imaging Suite — Progress Tracker

> Updated: 2026-11-19

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
**Status:** Complete

- [x] Pre-shared enrollment key system (common types, server CRUD, hash-based lookup)
- [x] Enrollment key validation in server auth flow (reject expired/revoked, require key when any exist)
- [x] Key management API endpoints (POST /api/keys, GET /api/keys, POST /api/keys/{id}/revoke)
- [x] Agent sends enrollment key from USB config during authentication
- [x] Admin UI: enrollment key management (create with expiry, revoke, status badges)
- [x] Admin UI: credential rotation alert on dashboard (keys expiring within 14 days)
- [x] Build Media wizard includes enrollment key field in config step
- [x] SuiteConfig updated with enrollment_key field + offline mode validation
- [x] Secure Boot investigation doc (docs/secure-boot-signing.md)
- [x] Code-signing scaffold script (tools/sign-agent.ps1)

---

### Phase 12: Testing & Hardening
**Branch:** `phase/12-testing-hardening`
**Status:** Complete

- [x] Unit tests: config parsing & validation (16 tests — modes, schema, discovery, enrollment key)
- [x] Unit tests: enrollment key security (14 tests — generate, hash, validity, expiry, serialization)
- [x] Unit tests: task sequence validation (12 tests — ordering constraints, empty checks, roundtrip)
- [x] Unit tests: image provider logic (5 tests — offline/online/hybrid, custom paths, descriptions)
- [x] Unit tests: unattend.xml generation (12 tests — locale, timezone, domain join, admin password, OOBE)
- [x] Unit tests: server DB operations (12 tests — images, deploy jobs, audit log, enrollment keys CRUD)
- [x] Removed unused import warning in server routes
- [x] Manual test checklists (docs/test-checklists.md — WinPE, multi-model hardware, DPI, network failure, offline, hybrid, enrollment key, capture, admin smoke test)
- [ ] Integration tests against real WinPE environment *(manual — requires ADK + VM)*
- [ ] Multi-model hardware testing *(manual — see docs/test-checklists.md)*
- [ ] DPI scaling visual tests *(manual — see docs/test-checklists.md)*

---

### Phase 13: USB Disk Automation
**Branch:** `phase/13-usb-disk-automation`
**Status:** In Progress

- [x] Physical USB disk enumeration via `Get-Disk` (replaces volume-based `Get-Volume`)
- [x] Automatic disk cleaning and GPT initialization (`Clear-Disk`, `Initialize-Disk`)
- [x] Automatic partitioning: FAT32 boot (2GB) + NTFS payload for offline/hybrid, single FAT32 for online
- [x] Full end-to-end USB build from admin app: disk prep → WinPE → agent inject → config → WIM copy
- [x] Progress events from backend to frontend via Tauri event system
- [x] WIM file path input in wizard step 2 for offline/hybrid mode
- [x] Confirmation dialog before disk wipe
- [x] Frontend shows physical disks (disk number, model, size, partition count)
- [x] Agent WIM path scanning: searches all volumes for deploy.wim if default path not found
- [x] Fixed build-winpe-media.ps1: set ADK environment variables (WinPERoot, DISMRoot, etc.)
- [ ] Admin app elevation prompt (currently requires manual "Run as Administrator")
- [ ] WIM file browser dialog (currently manual path entry)

---

### Phase 13b: WinPE Agent GUI — Hyper-V Boot Testing (ACTIVE — BLOCKED)
**Branch:** `phase/13-usb-disk-automation`
**Status:** In Progress — agent GUI does not yet start in WinPE

Added `tools/test-winpe-hyperv.ps1` to iterate on the agent in a Hyper-V VM
instead of physical USB (build WinPE ISO → inject agent + DLLs → create/boot
Gen2 VM). This surfaced a real blocker that also affects the production USB.

**What works now (fixed this session):**
- [x] ISO creation via `oscdimg` — use `-bootdata:1#pEF,e,b<efisys>` (UEFI-only),
      drop `-n` (conflicts with `-u2`); copy `efisys_noprompt.bin` to a space-free
      path so no quote-escaping is needed.
- [x] Gen2 VM boots the ISO — Secure Boot **off** + DVD first boot device, applied
      on *every* run (WinPE media is unsigned).
- [x] DLL auto-injection: `test-winpe-hyperv.ps1` parses the agent's PE import
      table and transitively injects every missing **real** DLL from the host
      (API-Set `api-ms-win-*`/`ext-ms-*` contracts are skipped — WinPE's apiset
      schema resolves them).
- [x] Agent binary itself is fine (`crt-static` is set in `.cargo/config.toml`).

**The blocker:** agent exits immediately with `-1073741515` (`0xC0000135`,
`STATUS_DLL_NOT_FOUND`) before any Rust code runs.

**Diagnosis (confirmed, not guesswork):**
- The agent's **complete static dependency closure is 25 real DLLs** — all 25 are
  present in WinPE after injection (verified in-VM). So the *static* chain is NOT
  the problem.
- The failure is a **delay-loaded** DLL triggered during window / GPU-surface
  creation. A delay-load that can't find its DLL raises `STATUS_DLL_NOT_FOUND` as
  an unhandled exception → process exits `0xC0000135`.
- Prime suspect: **`dcomp.dll` (DirectComposition)** — `dwmapi.dll` delay-loads it
  and wgpu's D3D12 surface uses a DirectComposition visual. WinPE has no DWM /
  DirectComposition / DirectX runtime. (⚠️ left off mid-step: still need to run the
  in-VM check `for %D in (dcomp.dll dxcore.dll d3d11.dll d2d1.dll dwrite.dll
  windowscodecs.dll cryptsp.dll twinapi.appcore.dll) do @if not exist
  X:\Windows\System32\%D echo MISSING: %D` to confirm which delay DLL is missing.)

**Root cause:** the agent's GUI stack (`winit` + `eframe`/**wgpu on Direct3D 12**)
expects the Windows desktop composition + DirectX runtime, which WinPE omits by
design. Grafting host DLLs in chases an ever-deeper dependency tree and risks
host-vs-WinPE version skew (host is 26300, ADK WinPE is older).

**DECISION NEEDED → recommended fix (do this next):**
- [ ] Switch agent `eframe` backend from **`wgpu` → `glow`** (OpenGL):
      edit `crates/agent/Cargo.toml` line 20 features `["default_fonts", "wgpu"]`
      → `["default_fonts", "glow"]`, then `cargo build --release -p imaging-agent`.
      Removes the entire D3D12 / DirectComposition path so `dcomp` is never
      triggered. (Check `crates/agent/src/main.rs` NativeOptions still compiles;
      glow is eframe's default renderer.)
- [ ] Drop in **Mesa3D's software `opengl32.dll`** (llvmpipe, `mesa3d` releases,
      the `...-mesa3d-...` package with `x64/opengl32.dll`). It is self-contained
      (deps: kernel32/user32/gdi32 only — no apiset chase, no GPU, no version skew).
      Inject it in place of the host `opengl32.dll` in `test-winpe-hyperv.ps1`.
- [ ] Optionally set `WGPU_BACKEND=gl` in startnet if any wgpu path remains.
- [ ] Rebuild ISO, boot VM, confirm the egui window renders (software rendering).
- [ ] Once proven: port the final DLL set + Mesa into the **production** builder
      (`tools/build-winpe-media.ps1` and `crates/admin/src-tauri/src/lib.rs:519`,
      which currently inject only `api-ms-win-core-synch-l1-2-0.dll`).

**Alternative (NOT recommended):** keep injecting the composition stack
(`dcomp.dll` + `d3d11`/`d2d1`/`dwrite`/…). Fragile, bloats boot.wim, version-skew
prone.

**Fallback if glow+Mesa still needs `dwmapi`/`uxtheme` quirks:** build a tiny stub
`dwmapi.dll` exporting the few functions winit calls as no-ops (zero heavy deps).

**Test loop:** elevated PowerShell →
`.\tools\test-winpe-hyperv.ps1` → `Start-VM -Name 'WinPE-Agent-Test'` →
`vmconnect localhost 'WinPE-Agent-Test'`. On agent failure, `startnet.cmd` prints
a DLL presence table. The admin app does NOT need to be running for this test.

---

### Future: Installer & Packaging
**Status:** TODO

- [ ] Tauri MSI/NSIS installer for admin app (configure tauri.conf.json: app metadata, icons, bundle settings)
- [ ] Server packaging (standalone .exe, optionally as a Windows Service)
- [ ] Agent binary — no installer needed (embedded in WinPE boot media by build-winpe-media.ps1)

---

## Open Questions (from spec §12)

| # | Question | Status |
|---|---|---|
| 1 | Signed or unsigned boot media? | **Resolved** — WinPE boot chain is MS-signed; agent EXE signing is optional (see docs/secure-boot-signing.md) |
| 2 | Offline capture in scope for v1? | **Open** — affects Phase 5 |
| 3 | Max capture size for shared drive? | **Open** — sizing/retention |
| 4 | PXE / WDS network boot in scope? | **Open** — post-USB stretch goal |

---

## Notes

- Phase order follows spec §10: vertical slice first, then breadth.
- Phases 1–3 are the critical learning path — WinPE constraints, network, and end-to-end apply.
- Phases 6–10 can partially overlap once the core pipeline works.
- Phase 11 (Secure Boot) has the longest external lead time — start investigation early.
