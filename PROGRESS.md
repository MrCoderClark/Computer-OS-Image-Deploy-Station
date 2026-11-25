# Windows Imaging Suite — Progress Tracker

> Updated: 2026-11-25

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
- [x] Admin app auto-elevates via embedded Windows manifest (requireAdministrator) — UAC prompt on launch, no more manual right-click
- [x] WIM file browser dialog: Browse… button in wizard step 2 opens native file picker (tauri-plugin-dialog)

---

### Phase 13b: WinPE Agent GUI — Hyper-V Boot Testing
**Branch:** `phase/13-usb-disk-automation`
**Status:** Complete — agent GUI renders in WinPE (confirmed 2026-11-20)

The agent exited with `STATUS_DLL_NOT_FOUND` (0xC0000135) because
`eframe`/`wgpu` on Direct3D 12 pulled in the Windows desktop composition stack
(`dcomp.dll`, DirectComposition) via a delay-load path that WinPE does not ship.

**Fix applied:**
- [x] Flipped `eframe` features `wgpu` → `glow` in `crates/agent/Cargo.toml`.
      Removes the D3D12/DirectComposition path entirely.
- [x] Added Mesa3D's software OpenGL stack (`opengl32.dll` + `libgallium_wgl.dll`,
      llvmpipe) at `tools/mesa/`. Replaces the host `opengl32.dll` ICD stub so
      the renderer runs entirely in CPU — no GPU driver needed.
- [x] Set `GALLIUM_DRIVER=llvmpipe` in startnet.cmd to skip Mesa's ZINK/Vulkan
      probe and suppress its harmless-but-confusing `vulkan-1.dll` error line.
- [x] Factored the PE-import walker + Mesa install into
      `tools/winpe-inject-deps.ps1`. All three callers use it:
      `tools/test-winpe-hyperv.ps1`, `tools/build-winpe-media.ps1`, and the
      Tauri admin backend (`crates/admin/src-tauri/src/lib.rs`).
- [x] Hyper-V harness pieces kept: UEFI-only `oscdimg` boot
      (`-bootdata:1#pEF,e,b<efisys>`), space-free boot-sector path,
      Secure Boot off + DVD first on every VM provision.

**Test loop:** elevated PowerShell →
`.\tools\test-winpe-hyperv.ps1` → `Start-VM -Name 'WinPE-Agent-Test'` →
`vmconnect localhost 'WinPE-Agent-Test'`. Agent main window renders via
software OpenGL (sluggish but correct).

**Packaging** (resolved in Phase 14): `tools/winpe-inject-deps.ps1`,
`tools/mesa/*.dll`, and `imaging-agent.exe` are bundled via
`tauri.conf.json` `bundle.resources`; `find_tools_dir()` and
`find_agent_binary()` try `app.path().resource_dir()` first and fall
back to the dev walk-up.

---

### Phase 21: Partition-first scratch for real-size downloads
**Branch:** `phase/21-partition-first-cache`
**Status:** Complete

Goal: lift the WinPE 512 MB scratch ceiling that killed Phase 20 Hyper-V
verification at ~492 MB. The agent now partitions the target disk first,
stages the download onto the freshly-formatted Windows partition, then
applies -- so the ceiling is the target drive size, not WinPE's RAM disk.

- [x] Engine: `pipeline::run_deploy` split into `prepare_target_disk` ->
      `apply_and_configure`. `run_deploy` kept as a thin wrapper for the
      local-WIM (Offline) fast path.
- [x] Agent: `start_deploy` orchestrates partition -> (optional download ->
      `W:\DeployStation\deploy.wim`) -> apply, then best-effort deletes the
      cached WIM so it doesn't waste space on the installed system
- [x] Agent: download destination built via `format!` (not `Path::join`) to
      work around Windows's drive-spec path join quirk (`"W:" + "x"` -> `"W:x"`)
- [x] Verified in Hyper-V (2026-11-25): full 4479.7 MB download landed on the
      VHDX (previously capped at ~492 MB in WinPE RAM), SHA-256 matched, and
      WIMGAPI apply ran against the on-disk copy. bcdboot then failed with
      `0xc000000f (STATUS_INVALID_IMAGE_FORMAT)`, but that's a separate
      pre-existing issue with the test WIM / `image_index = 1` hardcode;
      every Phase 21 code path ran clean.
- [x] Bugfix: `enumerate_disks` no longer misreads diskpart's `*` (Gpt/Dyn
      columns) as the boot disk -- false-positive blocked re-deploys onto
      any previously-GPT-formatted target. Proper boot detection via
      `Get-Disk | ? IsBoot` (or WMI) is a follow-up.
- [ ] Follow-up (Phase 22 candidate): investigate bcdboc `0xc000000f` -- WIM
      may be a non-standard/captured image, or `image_index = 1` isn't the
      OS edition we want. Expose index selection in the suite config +
      admin build-media wizard.
- [ ] Follow-up: skip the post-download sha256 rehash when the stream was
      not resumed (slow on 4+ GB over the VHDX-backed cache; the server's
      chunk integrity is sufficient for single-stream downloads)
- [ ] Follow-up: proper boot-disk detection via `Get-Disk | ? IsBoot` or WMI
      from Rust so physical USB-booted hardware is actually protected
- [ ] Follow-up: stream-to-apply (WIMGAPI apply consumes HTTP body directly,
      no intermediate file) as a later optimization -- partition-first
      scratch removes the hard blocker so this can wait
- [ ] Follow-up: Online-mode image picker (carried over from Phase 20)

---

### Phase 20: Agent wires network downloads into apply
**Branch:** `phase/20-agent-network-download`
**Status:** Complete

Goal: turn Phase 19's plumbing into a user-visible feature. The agent's
`NetworkProvider` previously returned a non-existent path and the apply
pipeline blew up at WIMGAPI; `start_deploy` now pre-flights a resumable
download when the provider asks for one, then runs the existing pipeline
against the local file.

- [x] Engine: `ImageProvider::needs_download()` on the trait (default `false`); `NetworkProvider` overrides to `true` so the agent can tell download-required providers from local-only ones
- [x] Agent: `resolve_deploy_provider()` now returns `(path, index, needs_download)`; `start_deploy` accepts the flag
- [x] Agent: new `download_before_deploy` step runs before `run_deploy` when `needs_download`; reuses `ServerClient::download_image` (resumable + sha256-verified)
- [x] Agent: `match_server_image()` picks the catalog entry by `config.image.sha256`, then `name`, falling back to the first catalog entry with a loud warning
- [x] Agent: download progress rides the existing `PipelineProgress` channel as `DeployStatus::Pending` with a `"Downloading image X.X / Y.Y MB"` message, throttled to one event per whole-percent change
- [x] Agent: clear preflight errors on the UI when `[server]` is missing from the suite config or when no server image matches (instead of a cryptic WIMGAPI failure later)
- [x] Verify end-to-end in Hyper-V (2026-11-25): `tools/build-hyperv-online.ps1` builds an ISO with a pinned-cert Online SuiteConfig; agent downloaded the server-registered WIM over HTTPS, SHA-256 matched, pipeline advanced through Validate → Partition → Apply (bcdboot then failed as expected on the synthetic test WIM, which carries no `\Windows\`). Every Phase 20 code path ran clean.
- [ ] Follow-up: downloaded WIM currently caches in `%TEMP%` (RAM-backed on WinPE, capped at 512 MB scratch on ADK 24H2). Real-size WIMs need either stream-to-apply or a disk-backed cache written after the first partitioning step.
- [ ] Follow-up: Online-mode Build Media wizard skips the image picker, so the agent's suite config has no `image` metadata and `match_server_image()` falls through to the first-catalog-entry fallback. Safe when the catalog has one row; wire an Online-mode image picker (admin or agent) before the catalog grows.

---

### Phase 19: Resumable image transfers
**Branch:** `phase/19-resumable-transfers`
**Status:** Complete

Goal: WIM downloads survive network blips. Server streams real files with
HTTP Range support; agent appends to its partial file and resumes with
`Range: bytes=OFFSET-`; SHA-256 verified at the end.

- [x] Server: new `images_dir` config knob (`%ProgramData%\DeployStation\images\` by default; `<image_id>.wim` convention)
- [x] Server: `GET /api/images/{id}/download` streams the file via `axum::Body::from_stream(ReaderStream)`; honors a single open-ended `Range: bytes=START-` (206 Partial Content); returns `Accept-Ranges: bytes` and a strong `ETag` of `"sha256:..."`; 416 when the range starts past EOF
- [x] Server: `Database::get_image(id)` for the per-id lookup the download needs
- [x] Agent: `ServerClient::download_image(id, dest, expected_sha256, progress)` -- opens dest in append mode when bytes exist, sends `Range: bytes=OFFSET-`, chunks via `bytes_stream()`, 4-hour per-attempt timeout; SHA-256 recomputed over the assembled file to catch range mis-stitches
- [x] Agent: handles 416 as "already complete" and proceeds straight to sha256 verify
- [x] Agent deps: `sha2`, `hex`, `futures-util`; server deps: `tokio-util` with `io` feature
- [x] Verify end-to-end: 4.37 GB WIM full-download (34.1 MB/s) matches source; Range split at midpoint + reassembly is bit-exact (`tools/test-resumable-download.ps1`)
- [ ] Next phase: wire `download_image` into the agent's apply flow (NetworkProvider currently returns a path with no download step)

---

### Phase 18: Server cert pinning (TOFU)
**Branch:** `phase/18-cert-pinning`
**Status:** Complete

Goal: close Phase 17's "accept any cert" fallback with a trust-on-first-use
flow. The admin trusts a server's self-signed cert out-of-band (user
compares fingerprint), the trusted PEM is persisted and used for all
future requests, and every USB built after that point bakes the cert into
`[server].cert_pem` so WinPE agents pin to the same server.

- [x] Server: public `GET /cert` (PEM) and `GET /fingerprint` (SHA-256 JSON) routes, backed by `AppState::with_tls` set at startup
- [x] Common: `ServerConfig.cert_pem: Option<String>` field (`#[serde(default)]`, so existing suite configs keep parsing)
- [x] Admin: `fetch_server_cert`, `trust_server`, `get_trusted_server_fingerprint`, `forget_trusted_server` Tauri commands; local-side fingerprint recomputation prevents a swapped cert between fetch and trust
- [x] Admin: `server_client(pinned: Option<&str>)` adds root cert when present, falls back to accept-any with a warning otherwise; all 7 server-facing callsites updated
- [x] Admin: Settings page has a "Server Trust (TLS pinning)" section with Trust / Forget buttons and a status line showing the pinned fingerprint
- [x] Admin: `build_config_inner` bakes the trusted cert into `[server].cert_pem` for every USB built after the admin trusts a server
- [x] Agent: `ServerClient::new(host, port, cert_pem)` pins when supplied by suite config; warns loudly and falls back to accept-any otherwise
- [x] Docs: Pinning section added to `server-deployment.md` covering Trust / Forget / regenerate
- [x] Verified: Trust flow in admin Settings works -- fingerprint dialog shows, confirm persists the cert, status line updates. WinPE-side pin verification pending the next media rebuild.

---

### Phase 17: TLS for the control channel
**Branch:** `phase/17-tls-control-channel`
**Status:** Complete

Goal: all server <-> admin and server <-> agent traffic is encrypted.
Browser-style CA trust is a non-goal; pinning by cert fingerprint is the
production model (lands in Phase 18).

- [x] Server speaks HTTPS via `axum-server` + `rustls`; TCP/plain HTTP fallback (`[tls] enabled = false`) kept for dev
- [x] First-run cert generation (`rcgen`): self-signed, SANs for localhost / 127.0.0.1 / ::1 / hostname, stored under `%ProgramData%\DeployStation\tls\`
- [x] `imaging-server fingerprint` subcommand prints the cert's SHA-256 (colon-separated hex), also logged on startup
- [x] Graceful shutdown adapted to `axum_server::Handle::graceful_shutdown`
- [x] Service-mode logging fix (preexisting Phase 16 gap -- service_main now initializes `tracing-subscriber`)
- [x] Agent client flips to `https://`, accepts any cert for now, logs a warning on construction
- [x] Admin client: `server_client()` / `server_base_url()` helper, all 7 server-facing callsites use HTTPS + accept-any-cert; Microsoft ADK download stays strict
- [x] Docs: TLS section in `server-deployment.md` covering fingerprint command, cert swap, and HTTP escape hatch
- [x] Verified: service rebuilt + reinstalled on HTTPS; admin dashboard shows "connected" against the self-signed cert. WinPE agent verification pending the next media rebuild.
- [ ] Phase 18 preview: carry the server fingerprint in the enrollment key / admin config and enforce it client-side

---

### Phase 16: Server Packaging (Windows Service)
**Branch:** `phase/16-server-packaging`
**Status:** Complete

Goal: `imaging-server.exe` deploys as a proper Windows Service with a
config file, log rotation, and install/uninstall scripts.

- [x] Dual-mode binary: console (default, Ctrl-C to stop) and Windows Service (`--service`, SCM-invoked)
- [x] Service install/uninstall subcommands via `windows-service` crate (`imaging-server install` / `uninstall`)
- [x] `server.toml` config (bind_addr, db_path, log_filter, log_dir) with lookup order: `--config` → `%ProgramData%\DeployStation\` → next-to-exe → defaults
- [x] Daily-rotated file logging via `tracing-appender` when `log_dir` is set
- [x] Graceful shutdown: console mode on Ctrl-C, service mode on SCM Stop/Shutdown
- [x] `tools/install-server-service.ps1` and `tools/uninstall-server-service.ps1`
- [x] `docs/server-deployment.md` with build, install, configure, uninstall, firewall
- [x] Verified: service installs via `install-server-service.ps1`, SCM starts it (LocalSystem, auto-start), and `/health` returns `{"status":"ok","version":"0.1.0"}`

---

### Phase 15: First-run bootstrapper (ADK auto-install)
**Branch:** `phase/15-first-run-bootstrap`
**Status:** Complete

Goal: the installed admin app should not require users to pre-install the
Windows ADK. On launch, the app detects missing ADK / WinPE add-on and
silently installs them from Microsoft's public bootstrappers.

- [x] Backend: `check_prerequisites` command returns which ADK pieces are installed
- [x] Backend: `install_prerequisites` downloads Microsoft's `adksetup.exe` + `adkwinpesetup.exe` (fwlink IDs 2289980 / 2289981) and runs them silently with `/features OptionId.DeploymentTools` and `OptionId.WindowsPreinstallationEnvironment`
- [x] Frontend: first-run overlay blocks the app until install completes or user explicitly skips; progress is emitted as `prereq-progress` events
- [x] Verified end-to-end on a clean machine (no ADK pre-installed): overlay fired, install ran, next launch skipped the prompt
- [ ] Consider: silent MSI/offline install mode where the ADK layout is pre-staged in `resources/adk-layout/`

---

### Phase 14: Installer & Packaging
**Branch:** `phase/14-installer-packaging`
**Status:** Complete

- [x] Tauri MSI/NSIS installer for admin app (publisher/copyright/description metadata, resources map bundles tools/ + mesa/ + agent.exe, perMachine NSIS install)
- [x] Admin runtime resolves bundled resources via `app.path().resource_dir()`
- [ ] Verify `cargo tauri build` produces working MSI + NSIS installers
- [ ] Install on a clean VM and run the build-media flow end-to-end from the installed app
- [ ] Server packaging (standalone .exe via `cargo build --release -p imaging-server` — no installer, document deploy)
- [x] Agent binary — no installer needed (bundled in admin installer AND embedded in WinPE boot media by build-winpe-media.ps1)

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
