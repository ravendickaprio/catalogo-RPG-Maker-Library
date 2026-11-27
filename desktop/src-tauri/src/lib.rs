pub mod commands;
pub mod db;
pub mod error;
pub mod models;
pub mod repo;
pub mod search;

use commands::{AppState, Ready};
use error::AppError;
use std::sync::{Arc, Mutex};

fn init_state() -> Result<Ready, AppError> {
    let path = db::resolve_db_path()?;
    let conn = db::open_read_only(&path)?;
    let (fts_available, _) = repo::detect_fts(&conn)?;
    let db_file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Ready {
        conn: Arc::new(Mutex::new(conn)),
        fts_available,
        db_file_name,
    })
}

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
            commands::suggest_game
        ])
}

pub fn run() {
    let state = AppState {
        inner: init_state(),
    };
    app_builder(tauri::Builder::default(), state)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
