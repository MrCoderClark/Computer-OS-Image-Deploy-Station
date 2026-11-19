# How to Test an Image Deploy

Step-by-step guide to deploy a Windows image to a target computer using DeployStation.

---

## What You Need

| Item | Details |
|------|---------|
| **Admin PC** | Windows 10/11 x64 with Rust toolchain installed |
| **Windows 11 ADK 24H2+** | Installed with the **WinPE add-on** ([Download ADK](https://learn.microsoft.com/en-us/windows-hardware/get-started/adk-install)) |
| **USB stick** | 8 GB minimum (32 GB+ for offline/hybrid with a WIM on the stick) |
| **Target computer** | UEFI-capable x64 machine you are OK wiping completely |
| **Windows WIM file** | Either a vanilla `install.wim` from Windows ISO media, or a captured custom image |
| **Network** | Both admin PC and target on the same LAN (for online/hybrid mode) |

> **WARNING:** The deploy process erases the target computer's disk entirely. Do not test on a machine with data you need.

---

## Step 1 — Get a WIM File

You need a `.wim` file containing the Windows image you want to deploy. Pick one:

### Option A: Extract from a Windows ISO (vanilla install)

1. Download a Windows 11 ISO from [microsoft.com](https://www.microsoft.com/software-download/windows11)
2. Mount the ISO (double-click it in Explorer)
3. Copy `D:\sources\install.wim` (where D: is the mounted ISO) to a working folder, e.g. `C:\Images\install.wim`
4. Note: Microsoft's `install.wim` contains multiple editions (Home, Pro, Enterprise) as separate indexes. DeployStation applies **index 1** by default.

To see what's in the WIM:

```powershell
dism /Get-ImageInfo /ImageFile:C:\Images\install.wim
```

### Option B: Capture from a reference machine (custom image)

If you have a "golden" reference machine already configured with Windows + apps + settings, you can capture it using DeployStation's capture feature. That workflow is outside the scope of this guide — for now, use Option A.

---

## Step 2 — Build the Rust Binaries

Open a terminal in the DeployStation repo root and build all three components.

**Build the server:**

```powershell
cargo build --release -p imaging-server
```

**Build the agent:**

```powershell
cargo build --release -p imaging-agent
```

**Build the admin app:**

```powershell
cd crates\admin\src-tauri
cargo tauri build
cd ..\..\..
```

The compiled binaries will be at:

- Server: `target\release\imaging-server.exe`
- Agent: `target\release\imaging-agent.exe`
- Admin: `crates\admin\src-tauri\target\release\DeployStation.exe` (or the generated MSI installer)

> **Tip:** For development/testing you can run the admin app without a full build using `cd crates\admin\src-tauri && cargo tauri dev`.

---

## Step 3 — Start the Imaging Server

The server holds the image catalogue, handles agent authentication, and tracks deploy status.

```powershell
.\target\release\imaging-server.exe
```

You should see:

```
Imaging server listening on 0.0.0.0:7443
Seeded sample images into database
```

Note your admin PC's IP address — the agent will connect to it:

```powershell
ipconfig | findstr "IPv4"
```

Leave the server running in this terminal.

---

## Step 4 — Start the Admin App

Open a **second** terminal. The admin app must run as Administrator (it needs elevated
privileges for disk operations and DISM).

**For development/testing:**

```powershell
cd crates\admin\src-tauri
cargo tauri dev
```

> This runs the app in dev mode with hot-reload. It opens the admin window automatically.

**For a release build:**

If you ran `cargo tauri build` in Step 2, the installer/executable is at:

```
crates\admin\src-tauri\target\release\bundle\msi\DeployStation_0.1.0_x64_en-US.msi
```

Install the MSI, then right-click the DeployStation shortcut → **Run as administrator**.

Or run the exe directly as admin:

```powershell
Start-Process -Verb RunAs .\crates\admin\src-tauri\target\release\DeployStation.exe
```

---

## Step 5 — Locate Your WIM File

For **offline or hybrid mode** (image on the USB stick), you will copy the WIM onto the USB stick in Step 6. For now, just know where your WIM file is (e.g. `C:\Images\install.wim`).

For **online mode**, the server's image download endpoint is currently a stub — real file streaming is not yet implemented. Use **offline** or **hybrid** mode for testing.

**Recommended for first test: Offline mode.** It is the simplest — no network dependency during deploy.

---

## Step 6 — Build the WinPE Boot USB

The admin app handles the entire USB build process automatically: disk preparation, WinPE
creation, agent injection, config writing, and WIM copying.

### Option A: Use the Admin App (recommended)

1. **Run the admin app as Administrator** (right-click → Run as administrator)
2. Navigate to **Build Media** in the sidebar
3. Walk through the wizard:
   - **Step 1 — Mode**: Choose Offline (simplest for first test), Hybrid, or Online
   - **Step 2 — Image**: Enter the path to your WIM file (e.g. `C:\Images\install.wim`)
   - **Step 3 — Target**: Select your USB disk (shows physical disks by model and size)
   - **Step 4 — Config**: Set server host/port (hybrid/online), capture share, enrollment key
   - **Step 5 — Build**: Review and click Build Media
4. The app will automatically:
   - Clean the USB disk and create GPT partitions
   - Build a WinPE working directory using the ADK
   - Inject the imaging agent into boot.wim
   - Copy boot files to the USB
   - Write config.json
   - Copy the WIM file (offline/hybrid mode)

The USB layout after build:

```
Partition 1 — FAT32 "DEPLOY_BOOT" (2 GB)
  EFI\Boot\bootx64.efi
  sources\boot.wim               (WinPE + agent)
  imaging\config.json             (your config)

Partition 2 — NTFS "DEPLOY_DATA" (rest of disk, offline/hybrid only)
  images\deploy.wim               (the Windows image)
```

### Option B: Use the PowerShell script (manual)

If you prefer the command line, open an **Administrator PowerShell** prompt:

```powershell
# Clean the USB disk first (replace X with your disk number from Get-Disk)
diskpart
# list disk → select disk X → clean → exit

# Then run the media builder
.\tools\build-winpe-media.ps1 -AgentPath .\target\release\imaging-agent.exe -UsbDriveLetter G
```

You will need to manually create `config.json` and copy the WIM file afterward.
See the `tools\build-winpe-media.ps1` script comments for details.

---

## Step 7 — Boot the Target Computer from USB

1. Plug the USB stick into the target computer
2. Power on and enter the UEFI boot menu (usually F12, F2, or Del — varies by manufacturer)
3. Select the USB drive as the boot device
4. WinPE will load and the imaging agent will launch automatically

You should see the agent GUI with:
- **Mode badge** showing "Offline" (or "Hybrid")
- **Image panel** showing the image name from config.json
- **Deploy** and **Capture** buttons

---

## Step 8 — Run the Deploy

1. Click **Deploy**
2. The confirm screen will show:
   - Target disk details (make sure it is the right disk!)
   - Image name and source
3. Type the confirmation text exactly as shown (this prevents accidental deploys)
4. Click **Confirm**

The deploy pipeline will run these phases:
1. **Partitioning** — creates GPT layout: EFI System Partition (300 MB) + MSR (16 MB) + Windows partition
2. **Applying** — WIMGAPI applies the WIM image to the Windows partition
3. **Configuring** — runs `bcdboot` to set up the UEFI boot configuration
4. **Rebooting** — the machine is ready to restart

The progress screen shows each phase, throughput, and a log tail.

---

## Step 9 — Verify

1. Remove the USB stick when prompted
2. The target computer should reboot into Windows Setup (OOBE) or directly to the desktop, depending on the image
3. If using a vanilla `install.wim`, expect the Windows Setup experience (language, region, account creation)
4. If using a captured custom image with an Unattend.xml, it should boot straight through

---

## Troubleshooting

| Problem | Likely cause | Fix |
|---------|-------------|-----|
| USB doesn't appear in boot menu | Secure Boot or boot order | Enter UEFI setup, make sure UEFI boot from USB is enabled |
| Agent shows "No Configuration" | Missing `config.json` | Check that `X:\imaging\config.json` exists on the USB (the USB's FAT32 partition is mounted as X: in WinPE) |
| Agent shows validation errors | Bad config.json | Check JSON syntax, ensure `schema_version` is `1` and required fields are present |
| "WIMGAPI.dll not found" | WinPE missing DLL | The ADK's WinPE should include WIMGAPI; ensure you used the Windows 11 ADK 24H2+ |
| Partitioning fails | Disk locked or missing | Make sure the target has a physical disk; if dual-booting, disconnect other disks |
| Machine doesn't boot after deploy | BCD misconfigured | Verify UEFI boot entry was created; try `bcdboot W:\Windows /s S: /f UEFI` manually from WinPE |
| Black screen after reboot | Driver issue | The vanilla WIM may lack drivers for the target hardware's display adapter; wait a few minutes for Windows to configure |

---

## Quick Reference

| Task | Command |
|------|---------|
| Build server | `cargo build --release -p imaging-server` |
| Build agent | `cargo build --release -p imaging-agent` |
| Build admin | `cd crates\admin\src-tauri && cargo tauri build` |
| Run server | `.\target\release\imaging-server.exe` |
| Run admin (dev) | `cd crates\admin\src-tauri && cargo tauri dev` |
| Build USB media | `.\tools\build-winpe-media.ps1 -AgentPath .\target\release\imaging-agent.exe -UsbDriveLetter E` |
| Check WIM contents | `dism /Get-ImageInfo /ImageFile:C:\Images\install.wim` |
| Get your IP | `ipconfig \| findstr "IPv4"` |
