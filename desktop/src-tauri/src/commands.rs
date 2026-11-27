use crate::error::AppError;
use crate::models::*;
use crate::repo;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tauri::State;

#[derive(Clone)]
pub struct Ready {
    pub conn: Arc<Mutex<Connection>>,
    pub fts_available: bool,
    pub db_file_name: String,
    /// Cached `LibraryInfo`: the connection is read-only for the whole session, so the data
    /// cannot change under us. The Phase 3 `PRAGMA data_version` refresh must call
    /// `invalidate_info()` when it detects an external write.
    pub info_cache: Arc<Mutex<Option<LibraryInfo>>>,
}

impl Ready {
    pub fn new(conn: Connection, fts_available: bool, db_file_name: String) -> Self {
        Ready {
            conn: Arc::new(Mutex::new(conn)),
            fts_available,
            db_file_name,
            info_cache: Arc::new(Mutex::new(None)),
        }
    }

    pub fn invalidate_info(&self) {
        if let Ok(mut c) = self.info_cache.lock() {
            *c = None;
        }
    }
}

/// App state. If startup failed, every command returns the stored Config error.
pub struct AppState {
    pub inner: Result<Ready, AppError>,
}

impl AppState {
    fn ready(&self) -> Result<Ready, AppError> {
        self.inner.clone()
    }
}

async fn run_blocking<T, F>(state: &AppState, f: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce(&Connection, &Ready) -> Result<T, AppError> + Send + 'static,
{
    let ready = state.ready()?;
    tauri::async_runtime::spawn_blocking(move || {
        let conn = ready
            .conn
            .lock()
            .map_err(|_| AppError::Database("Conexión no disponible (lock envenenado).".into()))?;
        f(&conn, &ready)
    })
    .await
    .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

#[tauri::command]
pub async fn get_library_info(state: State<'_, AppState>) -> Result<LibraryInfo, AppError> {
    run_blocking(&state, |c, r| {
        let mut cache = r
            .info_cache
            .lock()
            .map_err(|_| AppError::Database("Caché no disponible (lock envenenado).".into()))?;
        if let Some(info) = cache.as_ref() {
            return Ok(info.clone());
        }
        let info = repo::library_info(c, &r.db_file_name)?;
        *cache = Some(info.clone());
        Ok(info)
    })
    .await
}

#[tauri::command]
pub async fn search_games(
    state: State<'_, AppState>,
    req: SearchRequest,
) -> Result<SearchPage, AppError> {
    run_blocking(&state, move |c, r| repo::search(c, &req, r.fts_available)).await
}

#[tauri::command]
pub async fn get_game(state: State<'_, AppState>, id: i64) -> Result<GameDetail, AppError> {
    run_blocking(&state, move |c, _| repo::get_game(c, id)).await
}

#[tauri::command]
pub async fn get_cover(state: State<'_, AppState>, id: i64) -> Result<Option<Cover>, AppError> {
    run_blocking(&state, move |c, _| repo::get_cover(c, id)).await
}

#[tauri::command]
pub async fn suggest_game(
    state: State<'_, AppState>,
    req: SuggestRequest,
) -> Result<Suggestion, AppError> {
    run_blocking(&state, move |c, _| repo::suggest(c, &req)).await
}
