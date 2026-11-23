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

Clients (admin app, WinPE agent) currently accept any cert on the control
channel so the connection is at least encrypted. Fingerprint pinning for
strict verification lands in Phase 18.

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
