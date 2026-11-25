# imaging-server — deployment

The DeployStation imaging-server is a single Rust binary that runs in two modes:

| Mode    | Invoked as                   | Use                                        |
|---------|------------------------------|--------------------------------------------|
| Console | `imaging-server`             | Development, interactive debugging         |
| Service | `imaging-server --service`   | Production — registered as a Windows Service by the SCM |

## Build

From the repo root:

```powershell
cargo build --release -p imaging-server
```

Output: `target\release\imaging-server.exe`.

## Install (production, Windows Service)

From an **elevated** PowerShell:

```powershell
.\tools\install-server-service.ps1
```

The script:

1. Copies `imaging-server.exe` to `%ProgramFiles%\DeployStation\`
2. Creates `%ProgramData%\DeployStation\` (holds config, database, logs)
3. Writes a default `server.toml` into that directory if none exists
4. Registers the Windows Service `DeployStationServer` (LocalSystem, auto-start)
5. Starts the service

Verify:

```powershell
Get-Service DeployStationServer
# -SkipCertificateCheck because the auto-generated cert is self-signed.
Invoke-WebRequest https://localhost:7443/health -SkipCertificateCheck
```

## TLS

The server is HTTPS by default. On first start it generates a self-signed
cert at `%ProgramData%\DeployStation\tls\server.crt` (+ `server.key`),
covering `localhost`, `127.0.0.1`, `::1`, and the machine's hostname.

Print the cert's SHA-256 fingerprint:

```powershell
& "$env:ProgramFiles\DeployStation\imaging-server.exe" fingerprint
```

### Pinning (Phase 18)

By default clients don't verify the self-signed cert, so an attacker on
the network could MITM the connection. To fix that, pin each client to
the exact cert:

1. On the server host, print the cert fingerprint and compare out-of-band:
   ```powershell
   & "$env:ProgramFiles\DeployStation\imaging-server.exe" fingerprint
   ```
2. In the admin app: **Settings &rarr; Server Trust &rarr; Trust current
   server**. The dialog shows the fingerprint; verify it matches, then
   confirm. The cert is persisted in the admin database.
3. Every USB built from here on bakes that same cert into the suite config
   under `[server] cert_pem`, so the WinPE agent pins to it automatically.
4. Hit **Forget** to stop pinning (new USBs lose the pin; existing USBs
   are unaffected).

Pinned verification uses `rustls`' trust-store semantics with the single
self-signed cert as the sole root. The SAN must match the hostname the
client connects to (the cert ships SANs for `localhost`, `127.0.0.1`,
`::1`, and `COMPUTERNAME`); use a matching name or regenerate the cert.

To replace the auto-generated cert with your own (public CA-issued or
enterprise PKI), overwrite `server.crt` / `server.key` and restart the
service.

To disable TLS (plain HTTP) for development only, set in `server.toml`:

```toml
[tls]
enabled = false
```

## Configure

Edit `%ProgramData%\DeployStation\server.toml` and restart the service:

```powershell
Restart-Service DeployStationServer
```

Options (see `crates/server/server.sample.toml` for the shipped defaults):

| Key           | Default                                        | Notes                                              |
|---------------|------------------------------------------------|----------------------------------------------------|
| `bind_addr`   | `0.0.0.0:7443`                                 | TCP listener. Change port here, not in the client. |
| `db_path`     | `imaging-server.db`                            | Relative to `%ProgramData%\DeployStation\`.        |
| `log_filter`  | `imaging_server=info,tower_http=info`          | `tracing-subscriber` env-filter.                   |
| `log_dir`     | *(unset)*                                      | Set to e.g. `logs` to enable daily file logging.   |
| `images_dir`  | `images`                                       | Folder of `<image_id>.wim` files served by `/api/images/{id}/download` with HTTP Range (resumable). |

## Uninstall

```powershell
.\tools\uninstall-server-service.ps1          # keeps db + logs
.\tools\uninstall-server-service.ps1 -Purge   # removes everything
```

## Run in console mode (dev)

Straight from the workspace, no install:

```powershell
cargo run -p imaging-server
# or with a specific config:
cargo run -p imaging-server -- --config .\crates\server\server.sample.toml
```

Ctrl-C triggers graceful shutdown.

## Firewall

The installer does **not** touch Windows Firewall. Open the chosen port
inbound if the admin app or WinPE agents connect from other hosts:

```powershell
New-NetFirewallRule -DisplayName "DeployStation imaging-server" `
    -Direction Inbound -Protocol TCP -LocalPort 7443 -Action Allow
```

## Logs

- `log_dir` unset → stdout only (visible in console mode; captured by SCM
  in service mode but not easily readable).
- `log_dir` set → daily-rotated files named `imaging-server.log.YYYY-MM-DD`
  under the configured directory.
