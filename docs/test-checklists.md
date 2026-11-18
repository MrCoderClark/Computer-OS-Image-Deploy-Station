# DeployStation — Manual Test Checklists

These checklists cover scenarios that require real hardware, WinPE, or
visual inspection. Automated unit tests live in each crate's `#[cfg(test)]`
modules.

---

## 1. WinPE Integration

**Prerequisites:** Windows 11 ADK 24H2+, USB stick, test VM or bare metal.

- [ ] `build-winpe-media.ps1` completes without error
- [ ] USB boots to WinPE on UEFI hardware
- [ ] Agent binary launches and displays main screen
- [ ] Config.json loads from `X:\imaging\config.json`
- [ ] Mode badge shows correct mode (Online / Offline / Hybrid)
- [ ] Agent connects to server (Online/Hybrid)
- [ ] Agent shows offline fallback (Hybrid, server unreachable)
- [ ] Agent shows "No Configuration" if config.json is missing

---

## 2. Multi-Model Hardware

Test on at least one device from each vendor.

| Vendor | Model | WinPE Boot | Display | NIC Loaded | Deploy | Capture |
|--------|-------|------------|---------|------------|--------|---------|
| HP     | EliteBook 840 G9 | [ ] | [ ] | [ ] | [ ] | [ ] |
| Dell   | Latitude 5540    | [ ] | [ ] | [ ] | [ ] | [ ] |
| Lenovo | ThinkPad T14     | [ ] | [ ] | [ ] | [ ] | [ ] |

**Notes:**
- WinPE may need NIC/storage drivers injected per model
- Check that disk detection finds the correct target drive
- Verify BCD boot config works (machine boots to Windows after deploy)

---

## 3. DPI Scaling

Test the agent UI at different display scales. Run on a Windows host
with `cargo run -p imaging-agent` and change display scaling in Settings.

| Scale | Main Screen | Confirm | Progress | Error | Complete |
|-------|-------------|---------|----------|-------|----------|
| 100%  | [ ] | [ ] | [ ] | [ ] | [ ] |
| 125%  | [ ] | [ ] | [ ] | [ ] | [ ] |
| 150%  | [ ] | [ ] | [ ] | [ ] | [ ] |
| 200%  | [ ] | [ ] | [ ] | [ ] | [ ] |

**Check for:**
- Text truncation or overflow
- Button sizing (clickable area large enough)
- Card layout integrity
- Scrollbar appearance on small viewports

---

## 4. Network Failure & Resume

| Scenario | Expected | Pass |
|----------|----------|------|
| Pull network cable during deploy | Reconnect overlay appears, retries 3x | [ ] |
| Server process killed mid-deploy | Error screen with actionable message | [ ] |
| DNS unreachable (Online mode) | "Server unreachable" status | [ ] |
| Slow network (throttle to 1 Mbps) | Progress still updates, no timeout | [ ] |
| Server restart during idle | Agent reconnects on next action | [ ] |

---

## 5. Offline Mode Full Cycle

- [ ] Build offline USB with image + enrollment key via admin app
- [ ] Boot target machine from USB
- [ ] Agent shows "Offline" mode badge, no server connection attempted
- [ ] Image panel shows local image metadata from config
- [ ] Confirm deploy → typed confirmation → deploy starts
- [ ] Partition → Apply → BCD → Reboot completes
- [ ] Machine boots to Windows

---

## 6. Hybrid Fallback Path

- [ ] Build hybrid USB with server config + fallback image
- [ ] Boot with server available → agent connects, shows server images
- [ ] Deploy using server image → completes normally
- [ ] Boot with server down → offline fallback banner appears
- [ ] "From USB" source label shows on image panel
- [ ] Deploy using fallback image → completes normally

---

## 7. Enrollment Key Auth

- [ ] Create key via admin Settings → key is displayed once
- [ ] Copy key → build USB with key → agent authenticates successfully
- [ ] Build USB without key → agent gets "Enrollment key required" error
- [ ] Build USB with wrong key → agent gets "Invalid enrollment key" error
- [ ] Revoke key in admin → agent using that key gets "revoked" error
- [ ] Expired key → agent gets "expired" error
- [ ] Dashboard shows rotation alert when key expires within 14 days

---

## 8. Capture Path

- [ ] UNC share probe succeeds (net use with service account)
- [ ] WIM capture runs (progress updates in UI)
- [ ] SHA-256 hash is computed and written to manifest
- [ ] Manifest JSON written to `X:\imaging\logs\`
- [ ] Server reconciliation registers the image in catalogue
- [ ] Image appears in admin Image Catalogue after capture

---

## 9. Admin App Smoke Test

- [ ] Dashboard loads with server stats
- [ ] Image Catalogue: grid/list toggle, search, OS filter, detail panel
- [ ] Task Sequences: create, edit steps, reorder, save, delete
- [ ] Driver Packs: add, delete
- [ ] Build Media wizard: mode → image → USB → config → build
- [ ] Deployments: table loads, refresh works
- [ ] Audit Log: search, level filter, CSV export
- [ ] Settings: enrollment key create, revoke, rotation alert
