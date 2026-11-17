use tauri::Manager;

#[tauri::command]
fn get_mode() -> String {
    "No configuration loaded".to_string()
}

#[tauri::command]
fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            tracing_subscriber::fmt()
                .with_env_filter("imaging_admin=debug")
                .init();

            tracing::info!("DeployStation Admin v{}", app.package_info().version);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_mode, get_version])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
