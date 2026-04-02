mod bridge;
mod commands;
mod progress;
mod volume;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::run_copy,
            commands::run_copy_preview,
            commands::run_watch_start,
            commands::run_watch_stop,
            commands::run_scan,
            commands::run_verify,
            commands::run_repair,
            commands::run_cert_create,
            commands::run_cert_verify,
            commands::run_export,
            commands::run_report,
            commands::get_history,
            commands::search_history,
            commands::get_profiles,
            commands::save_profile,
            commands::get_volumes,
            commands::pick_folder,
            commands::pick_file,
            commands::pick_save_location,
            commands::quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ferr-app");
}
