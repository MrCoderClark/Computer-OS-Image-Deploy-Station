# WinPE Build & Testing Guide

## Prerequisites

1. **Windows 11 ADK (24H2 or newer)**  
   Download from Microsoft: Assessment and Deployment Kit  
   Install the "Deployment Tools" feature.

2. **WinPE Add-on for the ADK**  
   Separate download from the same page.  
   Install after the ADK.

3. **Rust toolchain**  
   Target: `x86_64-pc-windows-msvc`

## Building the Agent

```powershell
cargo build --release -p imaging-agent
```

The binary lands at `target\release\imaging-agent.exe`.

### Static Linking Notes

The agent uses egui with the `glow` OpenGL backend by default. In WinPE:

- **OpenGL:** WinPE includes a software OpenGL renderer (`opengl32.dll`). The glow backend works but is slow. For production, consider wgpu with the D3D12 backend if the target machines have GPU drivers in WinPE.
- **CRT:** The MSVC target links the C runtime dynamically (`vcruntime140.dll`). WinPE includes this DLL. If it's missing, build with `+crt-static`:

```powershell
$env:RUSTFLAGS = "-C target-feature=+crt-static"
cargo build --release -p imaging-agent
```

## Building WinPE Media

### ISO (for testing in a VM)

```powershell
# Run as Administrator
.\tools\build-winpe-media.ps1 -AgentPath .\target\release\imaging-agent.exe
```

Output: `.\output\DeployStation-WinPE.iso`

### USB Drive

```powershell
# Run as Administrator — WARNING: erases the USB drive
.\tools\build-winpe-media.ps1 -AgentPath .\target\release\imaging-agent.exe -UsbDriveLetter E
```

## Testing in a VM

1. Create a Hyper-V Gen 2 VM (UEFI, Secure Boot off for now)
2. Mount the ISO as a DVD drive
3. Boot from DVD
4. WinPE boots → `startnet.cmd` runs → agent launches

### Quick Hyper-V Setup

```powershell
$vmName = "DeployStation-Test"
New-VM -Name $vmName -Generation 2 -MemoryStartupBytes 2GB
Set-VMFirmware -VMName $vmName -EnableSecureBoot Off
Add-VMDvdDrive -VMName $vmName -Path ".\output\DeployStation-WinPE.iso"
$dvd = Get-VMDvdDrive -VMName $vmName
Set-VMFirmware -VMName $vmName -FirstBootDevice $dvd
Start-VM -Name $vmName
```

## Known WinPE Constraints

### Available in WinPE
- `kernel32.dll`, `ntdll.dll`, `user32.dll`, `gdi32.dll`
- `ws2_32.dll` (Winsock — networking works)
- `advapi32.dll` (registry, security)
- `ole32.dll`, `oleaut32.dll` (COM)
- `vcruntime140.dll`, `ucrtbase.dll` (C runtime)
- `opengl32.dll` (software renderer)
- `wimgapi.dll` (WIM operations)
- Basic TCP/IP stack with DHCP

### Missing or Limited in WinPE
- **No WebView2** — no Chromium, no Edge. This is why we use egui.
- **No Explorer shell** — no taskbar, no start menu, no desktop.
- **No user profile** — no `%APPDATA%`, no roaming profiles.
- **No audio** — no sound subsystem by default.
- **No .NET Framework** — only .NET is available via optional package.
- **Limited GPU** — software rendering only unless drivers are injected.
- **RAM disk** — WinPE runs from a RAM disk. The X: drive is RAM. Don't write large files to X:.
- **No persistent storage** — anything written to the RAM disk is gone on reboot.

### DLL Dependencies to Watch

When adding new Rust crates, check their DLL dependencies:

```powershell
# Check what DLLs the agent needs
dumpbin /dependents target\release\imaging-agent.exe
```

Expected dependencies for the base agent:
- `KERNEL32.dll`
- `ADVAPI32.dll`
- `WS2_32.dll`
- `USERENV.dll`
- `USER32.dll`
- `GDI32.dll`
- `OPENGL32.dll`
- `bcrypt.dll`

If `dumpbin` shows a DLL not in the WinPE list above, either:
1. Inject the DLL into boot.wim (adds size)
2. Find an alternative crate that doesn't need it
3. Use static linking where possible

### Display Scaling

WinPE typically runs at the native display resolution with no DPI scaling. Test at:
- 1920x1080 (100% scale — most common)
- 1366x768 (older laptops)
- 3840x2160 (4K — text may be tiny without scaling)

egui handles high-DPI via winit's scale factor detection, but WinPE may not report it correctly. Consider a command-line flag to override the scale factor.
