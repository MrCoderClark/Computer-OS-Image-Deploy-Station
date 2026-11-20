use embed_manifest::embed_manifest;
use embed_manifest::manifest::{ExecutionLevel, new_manifest};

fn main() {
    // The admin app invokes diskpart, DISM, Mount-WindowsImage, and writes
    // directly to removable drives — all of which require elevation. Embed a
    // Windows application manifest that requests requireAdministrator so
    // Windows shows the UAC prompt on launch and the user never needs to
    // right-click → Run as Administrator.
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(
            new_manifest("DeployStation.Admin")
                .requested_execution_level(ExecutionLevel::RequireAdministrator),
        )
        .expect("failed to embed admin manifest");
    }

    tauri_build::build();
}
