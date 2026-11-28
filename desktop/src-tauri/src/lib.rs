pub mod backup;
pub mod commands;
pub mod db;
pub mod error;
pub mod images;
pub mod importer;
pub mod maintenance;
pub mod migrate;
pub mod models;
pub mod passwords;
pub mod protocol;
pub mod repo;
pub mod search;
pub mod settings;
pub mod write;

use commands::AppState;
use tauri::Manager;

/// Construye el builder con el estado y la lista cerrada de comandos permitidos.
/// Genérico sobre el runtime para poder probar el IPC con `tauri::test::MockRuntime`.
pub fn app_builder<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
    state: AppState,
) -> tauri::Builder<R> {
    builder
        .manage(state)
        .register_asynchronous_uri_scheme_protocol("cover", |ctx, request, responder| {
            protocol::handle(ctx.app_handle().clone(), request, responder)
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_library_info,
            commands::get_dashboard,
            commands::search_games,
            commands::get_game,
            commands::get_cover,
            commands::set_cover,
            commands::remove_cover,
            commands::suggest_game,
            commands::check_data_version,
            commands::get_vocabulary,
            commands::get_session_info,
            commands::create_game,
            commands::update_game,
            commands::set_favorite,
            commands::delete_game,
            commands::create_backup,
            commands::set_db_path,
            commands::set_read_only,
            commands::get_migration_status,
            commands::run_migrations,
            commands::scan_import,
            commands::cancel_scan,
            commands::run_import,
            commands::get_import_history,
            commands::undo_last_import,
            commands::maintenance_summary,
            commands::maintenance_list,
            commands::scan_missing_paths,
            commands::cancel_maintenance_scan,
            commands::list_orphan_tags,
            commands::search_index_status,
            commands::preview_batch,
            commands::apply_batch,
            commands::delete_orphan_tags,
            commands::rebuild_search_index,
            commands::list_batches,
            commands::undo_last_batch,
            commands::list_backups,
            commands::verify_backup,
            commands::restore_backup,
            commands::check_database,
            commands::export_library,
            commands::page_passwords_status,
            commands::get_page_password,
            commands::set_page_password,
            commands::delete_page_password,
            commands::copy_page_password
        ])
}

pub fn run() {
    app_builder(tauri::Builder::default(), AppState::uninitialized())
        .plugin(tauri_plugin_dialog::init())
        // Only used from Rust (copy_page_password); no clipboard permission is granted to the webview.
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let settings_path = app
                .path()
                .app_config_dir()
                .ok()
                .map(|d| d.join("settings.json"));
            let state = app.state::<AppState>();
            if let Ok(dir) = app.path().app_data_dir() {
                state.set_data_dir(dir);
            }
            state.init(settings_path);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
