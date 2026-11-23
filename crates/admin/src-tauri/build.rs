fn main() {
    // The admin app invokes diskpart, DISM, Mount-WindowsImage, and writes
    // directly to removable drives -- all of which require elevation. Replace
    // Tauri's default asInvoker manifest with one that requests
    // requireAdministrator so Windows shows the UAC prompt on launch and the
    // user never needs to right-click -> Run as Administrator.
    let windows = tauri_build::WindowsAttributes::new()
        .app_manifest(include_str!("app.manifest"));
    tauri_build::try_build(
        tauri_build::Attributes::new().windows_attributes(windows),
    )
    .expect("failed to run tauri-build");
}
