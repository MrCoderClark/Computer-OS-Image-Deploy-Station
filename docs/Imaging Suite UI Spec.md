UI Specification

# Windows Imaging Suite

User interface specification for a two-part Windows imaging system: a Tauri-based admin console and a lightweight WinPE client agent. Designed as a modern enterprise Windows desktop application targeting Windows 11 visual language.

v1.0 November 2026 Rust + Tauri + egui Windows 10 / 11 x64

Design System

## Visual Language

The suite follows Microsoft's WinUI 3 / Fluent Design language adapted for a 2026 enterprise imaging tool. Surfaces use layered elevation with subtle material effects. Typography is clean and functional. Color conveys system state, not decoration.

### Color Tokens

Primary#005FB8

Primary Light#60CDFF

Teal Accent#00B7C3

Background#F3F3F3

Surface#FFFFFF

Foreground#1A1A1A

Success#0F7B0F

Warning#9D5D00

Critical#C42B1C

### Typography Scale

Display

Segoe UI Variable Display

Title

Section Title

Subtitle

Subtitle / Card Header

Body

Body text at 14px. Segoe UI Variable or Inter as fallback.

Caption

Caption and metadata — 12px, secondary color

Mono

Cascadia Code — logs, hashes, paths

### Elevation & Surfaces

Three surface levels following WinUI 3 layering. Base layer uses Mica-style tinted background. Cards and panels sit on Surface (1dp elevation). Flyouts and dialogs use Raised (4dp). All surfaces carry 6px border-radius. Borders are 1px with subtle opacity.

i

**Platform note:** The admin app renders in WebView2 (Tauri) and inherits Windows 11 system acrylic and Mica material when available. The client agent uses egui/iced with software rendering — surfaces are solid color fills, no transparency compositing in WinPE.

Admin Application

## Application Shell

The admin app uses a persistent left sidebar for navigation with a compact top bar. The window supports standard Windows chrome (minimize, maximize, close) integrated into a custom title bar. The sidebar collapses to icons on narrow viewports.

### Navigation Structure

| Section | Nav Item | Purpose |
| --- | --- | --- |
| — | Dashboard | System overview, recent activity, alerts |
| Library | Image Catalogue | Browse, search, manage WIM images |
| Driver Packs | Per-model driver management |  |
| Task Sequences | Ordered deployment step editor |  |
| Operations | Build Media | USB creation wizard |
| Deployments | Active and historical deployment tracking |  |
| System | Audit Log | Full system audit trail |
| Settings | Server config, credentials, preferences |  |

### Dashboard

The landing view. Shows system health at a glance: active deployments, image stats, recent captures, and credential rotation alerts. No charts for the sake of charts — every element answers a question a technician has when they open the app.

Windows Imaging Suite — Dashboard

—

☐

✕

Dashboard

Library

Image Catalogue

Driver Packs

Task Sequences

Operations

Build Media

Deployments

System

Audit Log

Settings

JC

admin.lab Connected

Dashboard

Active Deploys

3

2 in progress

Images in Catalogue

14

4 Win11 · 10 Win10

USB Media Built

47

+3 this week

Captures Pending

1

Reconcile needed

⚠

**Credential rotation due:** svc-imaging account password expires in 4 days. Rotate and rebuild active media.

Recent Activity

| Time | Event | Target | Status |
| --- | --- | --- | --- |
| 14:22 | Deploy — Win11-24H2-Base | DESK-PC-0147 | Active |
| 13:58 | Media built — Hybrid USB | USB-0023 | Complete |
| 11:45 | Capture — HP-840-G9-Lab | \\\\fileserver\\imaging | Reconcile |

### Image Catalogue

Grid and list views. Each image card shows the name, OS family tag, source hardware, build date, compressed size, and hash verification status. Supports search and OS family filtering. The server holds many images; a USB carries exactly one.

Windows Imaging Suite — Image Catalogue

—

☐

✕

Dashboard

Library

Image Catalogue

Driver Packs

Task Sequences

Operations

Build Media

Deployments

Image Catalogue

\+ Import Image

Search images...

OS: All

Sort: Newest

▦

≡

W

Win11-24H2-Base

Win11 HP EliteBook 840 G9 20.0 GB · Nov 10, 2026 SHA-256 verified

W

Win11-24H2-Dev

Win11 Dell Latitude 5540 24.5 GB · Nov 8, 2026 SHA-256 verified

W

Win10-22H2-Legacy

Win10 Lenovo ThinkPad T14 18.2 GB · Oct 22, 2026 Not verified

#### Image Card Fields

Component

| Field | Source | Display |
| --- | --- | --- |
| Name | `image.name` | Primary label, semibold |
| OS Family | `image.os_family` | Color-coded badge (blue = Win11, amber = Win10) |
| Source Hardware | `image.captured_from` | Secondary text |
| Size | `image.size_bytes` | Human-readable (GB), tabular-nums |
| Build Date | `image.build_date` | Formatted date |
| Hash Status | `image.sha256` | Green "verified" or muted "not verified" |

### Build Media Wizard

A stepped wizard for creating bootable USB media. Mode is a build-time property — once built, the stick's mode cannot change without a rebuild. The wizard enforces this by making mode selection the first irrevocable choice.

Windows Imaging Suite — Build Media

—

☐

✕

Dashboard

Library

Image Catalogue

Driver Packs

Task Sequences

Operations

Build Media

Deployments

Build Bootable USB

✓

Mode

2

Image

3

Target

4

Config

5

Build

Select Image

Choose the WIM image to include on this USB stick. In Hybrid mode, this image is both the network fallback and the offline payload.

W

Win11-24H2-Base

20.0 GBNov 10Win11

●

W

Win11-24H2-Dev

24.5 GBNov 8Win11

○

Back

Next: Select Target

#### Wizard Steps

Flow

| Step | Content | Validation |
| --- | --- | --- |
| 1. Mode | Radio: Online / Offline / Hybrid (recommended) | Required. Tooltip explains each mode's tradeoffs. |
| 2. Image | Image picker from catalogue | Offline/Hybrid requires an image. Online skips to step 3. |
| 3. Target | USB device picker (shows drive letter, size, label) | Warns if drive has data. Blocks internal disks. |
| 4. Config | Server host, capture UNC, credentials, task sequence | Pre-filled from settings. Test connection button. |
| 5. Build | Summary + progress bar. Partitions, copies WinPE, writes config. | Confirm dialog before write begins. |

### Task Sequences

Ordered step lists that define a deployment workflow. Each step has a type (partition, apply, inject, script, domain join, reboot) and configurable parameters. Steps are reorderable by drag handle. The editor validates ordering constraints (e.g., partition before apply).

Task Sequence Editor — Standard Deploy

—

☐

✕

Standard Deploy

\+ Add Step

Save

1

Partition Disk

partition

GPT · ESP + MSR + Windows

2

Apply Image

apply

Win11-24H2-Base → C:

3

Inject Drivers

inject

Auto-detect model → driver pack

4

Apply Unattend.xml

unattend

Generate from job parameters

5

Configure Boot (BCD)

script

bcdboot C:\\Windows /s S: /f UEFI

6

Join Domain

domain

LAB.EXAMPLE.COM

7

Reboot

reboot

Into Windows OOBE

### Deployments

Live monitoring view for all active and recent deployments. Each row shows the target machine, assigned image, current phase, progress, and the technician who started it. Rows in progress update in real time via the control channel WebSocket. A machine mid-deploy is claimed — the row shows a lock icon to prevent dual-write.

Windows Imaging Suite — Deployments

—

☐

✕

Deployments

Search...

Status: All

| Machine | Image | Mode | Phase | Progress | Status |
| --- | --- | --- | --- | --- | --- |
| 🔒 DESK-PC-0147 | Win11-24H2-Base | Hybrid | Applying image | 67% · 1.2 GB/s | Active |
| 🔒 LAB-WS-0032 | Win11-24H2-Dev | Online | Injecting drivers | 85% | Active |
| CONF-PC-0089 | Win10-22H2-Legacy | Offline | — | Complete | Done |
| DESK-PC-0091 | Win11-24H2-Base | Hybrid | Partitioning | Failed — disk locked | Error |

### Audit Log

Searchable, filterable system audit trail. Every imaging action — deploy, capture, media build, config change — is logged with timestamp, actor, target, and outcome. Non-negotiable in a managed environment. Supports export for compliance.

Windows Imaging Suite — Audit Log

—

☐

✕

Search audit log...

Level: All

Date: Last 7 days

Export CSV

Nov 17 14:22:03INFODeploy started — Win11-24H2-Base → DESK-PC-0147 by admin.lab

Nov 17 13:58:41INFOMedia built — Hybrid USB (USB-0023) with Win11-24H2-Base

Nov 17 11:45:12WARNCapture uploaded to \\\\fileserver\\imaging but not reconciled to catalogue

Nov 16 16:33:08ERRDeploy failed — DESK-PC-0091: target disk locked by another process

Nov 16 15:20:00INFOImage imported — Win10-22H2-Legacy (18.2 GB) from \\\\fileserver\\imaging

Nov 16 09:14:52INFOSettings updated — server.host changed to imaging.lab.example.com

Client Agent (WinPE)

## Field Agent Interface

The agent runs inside WinPE after USB boot. Built with egui or iced — pure Rust, no browser engine. The UI is deliberately minimal: a technician holding a USB stick in a server room needs two buttons, clear status, and an unmistakable confirmation step before writing a disk. No wizards. No nesting. No ambiguity.

i

**Rendering:** egui/iced renders with software or basic GPU. No transparency compositing, no acrylic, no Mica. Surfaces are solid fills. Font is the system default bundled with egui (or Segoe UI if injected into WinPE). Keep all UI above 12px for readability on varied display scaling.

### Main Screen

The primary view after boot. Shows mode, connectivity, the loaded image, and two large action buttons. The technician must be able to identify what this stick does without plugging it into another machine — the mode badge and image name are the first things visible.

Windows Imaging Agent

—

☐

✕

◆ HYBRID

DESK-PC-0147

Server connected

Win11-24H2-Base

OS Family

Windows 11

Build Date

Nov 10, 2026

Size

20.0 GB

Hash

SHA-256 verified

↓

Image This Computer

Apply image to this machine's disk

↑

Create Image

Capture this machine to the server

#### Header Bar Indicators

Agent

| Element | States | Purpose |
| --- | --- | --- |
| Mode Badge | ONLINE (blue), OFFLINE (amber), HYBRID (teal) | Instant stick identification at a glance |
| Server Dot | Green (connected), Amber (connecting), Red (unreachable) | Network state before starting a job |
| Machine Name | DNS hostname or hardware serial | Target identity confirmation |

### Confirmation Step

Destructive operations deserve friction. Before writing a disk, the agent shows exactly which physical disk will be erased, which image will be applied, and requires a typed confirmation. No single-click path to data loss.

Windows Imaging Agent — Confirm

—

☐

✕

◆ HYBRID

DESK-PC-0147

Connected

!

Erase disk and apply image?

**Target disk:** Disk 0 — Samsung SSD 980 PRO 1TB\
**Image:** Win11-24H2-Base (20.0 GB)\
**Partitions:** ESP (300 MB) + MSR (16 MB) + Windows\
\
All existing data on Disk 0 will be permanently erased.

Type DEPLOY to confirm

Cancel

Erase & Deploy

⚠

**Design rule:** The "Erase & Deploy" button remains disabled (50% opacity) until the typed confirmation matches. No hold-to-confirm alternative — typed input is the only path. This catches fat-fingered Enter presses after a habit of clicking through dialogs.

### Progress View

During imaging, the agent shows the current phase name, a progress bar, throughput rate, elapsed time, and an expandable log tail. The phase list is derived from the task sequence. If the network drops mid-transfer (online/hybrid), the progress view shows the reconnection state rather than failing silently.

Windows Imaging Agent — Deploying

—

☐

✕

◆ HYBRID

DESK-PC-0147

Connected

Applying Image

67% complete 13.4 / 20.0 GB 1.2 GB/s 4m 12s elapsed

Task Sequence Progress

✓ Partition ● Apply Image ○ Inject Drivers ○ Unattend ○ Boot Config ○ Reboot

Log

Expand ▾

14:22:03 \[INFO\] Partitioning Disk 0 — GPT layout\
14:22:04 \[INFO\] ESP created (300 MB, FAT32)\
14:22:04 \[INFO\] MSR created (16 MB)\
14:22:04 \[INFO\] Windows partition created (remaining)\
14:22:05 \[INFO\] Applying Win11-24H2-Base to C:\
14:26:17 \[INFO\] Transferred 13.4 GB / 20.0 GB (67%)

Component Reference

## Shared Component Spec

### Buttons

Primary Action

Secondary

Destructive

Disabled

| Variant | Use | Spec |
| --- | --- | --- |
| Primary | Main call to action per view | Accent fill, white text, 4px radius, 6px 14px padding |
| Secondary | Auxiliary actions | Surface fill, border, default text |
| Destructive | Erase, delete, overwrite | Error fill, white text. Requires confirmation gate. |
| Disabled | Precondition not met | 50% opacity, no pointer events |

### Status Badges

Active Complete Warning Error

| State | Color | Dot | Use |
| --- | --- | --- | --- |
| Active / Online | Green bg + text | Animated pulse | In-progress operations, server connected |
| Info / Complete | Blue bg + text | None | Completed state, informational tags |
| Warning | Amber bg + text | Static | Needs attention but not blocking |
| Error / Critical | Red bg + text | Static | Failed state, requires intervention |

### Data Tables

| Property | Value |
| --- | --- |
| Header | 10px uppercase, 600 weight, tertiary color, subtle background |
| Row height | 40px minimum. Vertically centered content. |
| Row border | 1px bottom, subtle. No alternating row color. |
| Row hover | Surface-alt background (admin app only, not agent) |
| Numeric columns | `font-variant-numeric: tabular-nums` |
| Sortable columns | Arrow indicator in header. Click to toggle asc/desc. |

Interaction Patterns

## Navigation & Behavior

#### Sidebar Navigation

Persistent left rail, 220px wide. Collapses to 48px icon-only on narrow viewports or via toggle. Active item highlighted with accent-tinted background and left accent border. Section headers group nav items. User avatar and connection status pinned to bottom.

#### Notifications

Toast notifications from bottom-right. Auto-dismiss after 5s for info, persist for warnings and errors. Credential rotation warnings appear as a persistent banner below the top bar until dismissed or resolved. Deploy failures trigger both a toast and an in-table error state.

#### Keyboard Navigation

Full keyboard support in admin app. Tab order follows visual layout. Escape closes modals and flyouts. Enter confirms primary action in dialogs. Ctrl+N opens Build Media wizard. All interactive elements have visible focus rings (2px accent outline, 2px offset).

#### Error States

Errors explain what went wrong and how to fix it. "Could not reach \\\\fileserver\\imaging — check network or update the UNC path in Settings" beats "Connection failed." Agent errors are full-screen with a retry button. Admin errors are inline or toast depending on scope.

### Agent-Specific Patterns

#### Share Reachability Probe

Agent

At startup, the agent probes the capture share UNC path before enabling the "Create Image" button. If unreachable, the button is disabled with a tooltip: "Capture share unreachable — \\\\fileserver\\imaging." The probe runs again on retry tap. Never start a 2-hour capture to discover the share is gone.

#### Resumable Transfer

Agent

If the network drops during an online/hybrid transfer, the progress view shows "Reconnecting..." with an amber indicator. The progress bar pauses but does not reset. On reconnect, transfer resumes from the last confirmed chunk (UUID-based). After 3 failed reconnection attempts, the agent offers "Retry" or "Cancel."

#### Offline Fallback

Agent

In Hybrid mode, if the server is unreachable at start, the agent shows "Server unreachable — using local image from USB" with an amber mode badge change. The local image path is automatic. No user decision required. The image panel notes the source: "From USB" vs "From server."

#### Disk Selection Safety

Agent

The agent auto-selects the target disk (excluding the USB boot device). If multiple candidate disks exist, the agent shows a picker listing each by model, serial, and size — never by drive letter alone. The USB boot disk is visually excluded and grayed with "Boot device — cannot overwrite."

Screen Map

## Complete Screen Inventory

#### Admin Application

Tauri

DashboardLanding

Image CatalogueGrid + List

Image DetailSide panel

Driver PacksTree + list

Task Sequence EditorStep list

Build Media Wizard5-step wizard

DeploymentsTable + live

Deployment DetailSide panel

Audit LogFiltered log

SettingsTabbed form

Capture ReconciliationImport flow

#### Client Agent

egui/iced

Boot / ConnectingSplash

Main ScreenAction select

Disk SelectionList (if multi)

Confirm DeployTyped confirm

Confirm CaptureTyped confirm

ProgressPhase + log

CompleteSummary

ErrorFull screen

ReconnectingOverlay

Technical Notes

## Implementation Guidance

| Concern | Admin App (Tauri) | Client Agent (egui/iced) |
| --- | --- | --- |
| Rendering | WebView2 — full CSS/JS, inherits Windows system theme | Software/GPU rasterizer — solid fills only, no CSS |
| Theme | System light/dark via `prefers-color-scheme` + Windows Mica | Fixed dark theme (high contrast on varied WinPE displays) |
| Typography | Segoe UI Variable (system), Inter fallback | Bundled font (egui default or embedded Segoe UI) |
| Min resolution | 1024 × 768 (responsive down to 800px content) | 800 × 600 (WinPE minimum) |
| DPI scaling | WebView2 handles automatically | egui handles via winit. Test at 100%, 125%, 150%. |
| Accessibility | ARIA roles, keyboard nav, focus visible, screen reader labels | Keyboard nav, high-contrast colors, min 14px body text |
| Animation | Subtle transitions (150ms ease). Respect `prefers-reduced-motion`. | None. Static UI only. Progress bar updates on repaint. |