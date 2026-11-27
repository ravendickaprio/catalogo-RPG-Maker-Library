pub mod backup;
pub mod commands;
pub mod db;
pub mod error;
pub mod migrate;
pub mod models;
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
        .invoke_handler(tauri::generate_handler![
            commands::get_library_info,
            commands::search_games,
            commands::get_game,
            commands::get_cover,
            commands::suggest_game,
            commands::check_data_version,
            commands::get_vocabulary,
            commands::get_session_info,
            commands::create_game,
            commands::update_game,
            commands::delete_game,
            commands::create_backup,
            commands::set_db_path,
            commands::set_read_only,
            commands::get_migration_status,
            commands::run_migrations
        ])
}

pub fn run() {
    app_builder(tauri::Builder::default(), AppState::uninitialized())
        .setup(|app| {
            let settings_path = app
                .path()
                .app_config_dir()
                .ok()
                .map(|d| d.join("settings.json"));
            app.state::<AppState>().init(settings_path);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
