use crate::error::AppError;
use crate::models::*;
use crate::settings::Settings;
use crate::{backup, db, images, importer, maintenance, migrate, passwords, repo, write};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tauri::State;

#[derive(Clone)]
pub struct Ready {
    pub conn: Arc<Mutex<Connection>>,
    pub fts_available: bool,
    pub db_file_name: String,
    /// Full path of the open database (None only for injected read-only test connections).
    pub db_path: Option<PathBuf>,
    pub read_only: bool,
    /// Cached `LibraryInfo`; invalidated by own writes and by `check_data_version` when
    /// `PRAGMA data_version` shows an external write.
    pub info_cache: Arc<Mutex<Option<LibraryInfo>>>,
    /// Last `PRAGMA data_version` seen on the app connection (-1 = unknown). SQLite does not
    /// bump it for commits made on the same connection, so own writes never look "external".
    pub data_version: Arc<AtomicI64>,
    /// True once the automatic backup of this session (this opened connection) was made.
    pub backed_up: Arc<AtomicBool>,
    pub last_backup: Arc<Mutex<Option<PathBuf>>>,
}

/// Result of `check_data_version` (camelCase on the wire).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataVersion {
    pub version: i64,
    pub changed: bool,
}

impl Ready {
    /// Wraps an already-open connection (read-only, no path). Used by tests.
    pub fn new(conn: Connection, fts_available: bool, db_file_name: String) -> Self {
        Self::build(conn, fts_available, db_file_name, None, true)
    }

    fn build(
        conn: Connection,
        fts_available: bool,
        db_file_name: String,
        db_path: Option<PathBuf>,
        read_only: bool,
    ) -> Self {
        let v = conn
            .query_row("PRAGMA data_version", [], |r| r.get::<_, i64>(0))
            .unwrap_or(-1);
        Ready {
            data_version: Arc::new(AtomicI64::new(v)),
            conn: Arc::new(Mutex::new(conn)),
            fts_available,
            db_file_name,
            db_path,
            read_only,
            info_cache: Arc::new(Mutex::new(None)),
            backed_up: Arc::new(AtomicBool::new(false)),
            last_backup: Arc::new(Mutex::new(None)),
        }
    }

    /// Opens `path` (validating the schema; never creates or alters anything).
    pub fn open(path: &Path, read_only: bool) -> Result<Self, AppError> {
        let conn = db::open_validated(path, read_only)?;
        let (fts, _) = repo::detect_fts(&conn)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Self::build(
            conn,
            fts,
            name,
            Some(path.to_path_buf()),
            read_only,
        ))
    }

    pub fn invalidate_info(&self) {
        if let Ok(mut c) = self.info_cache.lock() {
            *c = None;
        }
    }

    pub fn ensure_writable(&self) -> Result<(), AppError> {
        if self.read_only {
            return Err(AppError::ReadOnly(
                "La base de datos está abierta en modo solo lectura.".into(),
            ));
        }
        Ok(())
    }

    pub fn session_info(&self) -> SessionInfo {
        SessionInfo {
            db_file_name: self.db_file_name.clone(),
            db_dir: self
                .db_path
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            read_only: self.read_only,
            last_backup: self
                .last_backup
                .lock()
                .ok()
                .and_then(|l| l.as_ref().map(|p| p.to_string_lossy().into_owned())),
        }
    }

    pub fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AppError> {
        self.conn
            .lock()
            .map_err(|_| AppError::Database("Conexión no disponible (lock envenenado).".into()))
    }

    fn make_backup(&self, conn: &Connection) -> Result<PathBuf, AppError> {
        let path = self.db_path.as_ref().ok_or_else(|| {
            AppError::Database("Backup fallido: se desconoce la ruta de la base.".into())
        })?;
        let b = backup::create_backup(conn, path)?;
        if let Ok(mut l) = self.last_backup.lock() {
            *l = Some(b.clone());
        }
        Ok(b)
    }

    /// Runs a write: refuses in read-only mode, makes the automatic backup before the first write
    /// of the session (abort on failure), then runs `f` and invalidates the info cache.
    pub fn with_write<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        self.write_inner(false, f)
    }

    /// Like `with_write` but always makes a fresh backup first (migrations).
    pub fn with_write_forced_backup<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        self.write_inner(true, f)
    }

    fn write_inner<T>(
        &self,
        force: bool,
        f: impl FnOnce(&Connection) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        self.ensure_writable()?;
        let conn = self.lock_conn()?;
        if force || !self.backed_up.load(Ordering::SeqCst) {
            self.make_backup(&conn)?;
            self.backed_up.store(true, Ordering::SeqCst);
        }
        let r = f(&conn);
        if r.is_ok() {
            self.invalidate_info();
        }
        r
    }

    /// Manual backup (allowed in read-only mode: it only reads the database).
    pub fn manual_backup(&self) -> Result<PathBuf, AppError> {
        let conn = self.lock_conn()?;
        self.make_backup(&conn)
    }
}

/// App state: the connection can be swapped (`set_db_path`, `set_read_only`). If opening failed,
/// every command returns the stored Config error.
pub struct AppState {
    inner: RwLock<Result<Ready, AppError>>,
    settings: Mutex<Settings>,
    settings_path: Mutex<Option<PathBuf>>,
    /// `app_data_dir` (historial de importaciones). `None` en tests que no lo fijan.
    data_dir: Mutex<Option<PathBuf>>,
}

impl AppState {
    fn make(inner: Result<Ready, AppError>) -> Self {
        AppState {
            inner: RwLock::new(inner),
            settings: Mutex::new(Settings::default()),
            settings_path: Mutex::new(None),
            data_dir: Mutex::new(None),
        }
    }

    pub fn set_data_dir(&self, dir: PathBuf) {
        if let Ok(mut d) = self.data_dir.lock() {
            *d = Some(dir);
        }
    }

    /// Carpeta de los registros de deshacer de los lotes de mantenimiento.
    pub fn undo_dir(&self) -> Option<PathBuf> {
        self.data_dir
            .lock()
            .ok()
            .and_then(|d| d.as_ref().map(|d| d.join(maintenance::UNDO_DIR)))
    }

    pub fn history_path(&self) -> Option<PathBuf> {
        self.data_dir
            .lock()
            .ok()
            .and_then(|d| d.as_ref().map(|d| d.join(importer::HISTORY_FILE)))
    }

    /// Injects an already-open `Ready` (tests); settings are not persisted.
    pub fn with_ready(r: Ready) -> Self {
        Self::make(Ok(r))
    }

    pub fn with_error(e: AppError) -> Self {
        Self::make(Err(e))
    }

    /// Placeholder until `init` runs in `.setup()`.
    pub fn uninitialized() -> Self {
        Self::make(Err(AppError::Config(
            "La aplicación aún no se ha inicializado.".into(),
        )))
    }

    /// Loads settings (when a path is given) and opens the resolved database.
    pub fn init(&self, settings_path: Option<PathBuf>) {
        let settings = settings_path
            .as_deref()
            .map(Settings::load)
            .unwrap_or_default();
        let ro = settings.read_only || db::env_read_only();
        let opened = db::resolve_app_db_path(&settings).and_then(|p| Ready::open(&p, ro));
        if let Ok(mut s) = self.settings.lock() {
            *s = settings;
        }
        if let Ok(mut p) = self.settings_path.lock() {
            *p = settings_path;
        }
        self.replace(opened);
    }

    fn replace(&self, r: Result<Ready, AppError>) {
        match self.inner.write() {
            Ok(mut g) => *g = r,
            Err(p) => *p.into_inner() = r,
        }
    }

    pub fn ready(&self) -> Result<Ready, AppError> {
        match self.inner.read() {
            Ok(g) => g.clone(),
            Err(p) => p.into_inner().clone(),
        }
    }

    fn persist(&self, s: Settings) -> Result<(), AppError> {
        let path = self.settings_path.lock().ok().and_then(|p| p.clone());
        if let Some(p) = path {
            s.save(&p)?;
        }
        if let Ok(mut cur) = self.settings.lock() {
            *cur = s;
        }
        Ok(())
    }

    fn settings_now(&self) -> Settings {
        self.settings.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Validates, persists and reopens on `path` (absolute, existing file).
    pub fn set_db_path(&self, path: &str) -> Result<Ready, AppError> {
        let p = PathBuf::from(path);
        if !p.is_absolute() {
            return Err(AppError::InvalidInput(
                "La ruta de la base de datos debe ser absoluta.".into(),
            ));
        }
        if !p.is_file() {
            return Err(AppError::InvalidInput(format!(
                "No existe un archivo en '{}'.",
                p.display()
            )));
        }
        let canon = db::resolve_file(&p)?;
        let mut s = self.settings_now();
        let ro = s.read_only || db::env_read_only();
        let ready = Ready::open(&canon, ro)?;
        s.db_path = Some(canon.to_string_lossy().into_owned());
        self.persist(s)?;
        self.replace(Ok(ready.clone()));
        Ok(ready)
    }

    /// Restores `file_name` over the open database (see `backup::restore_into`) and swaps in a
    /// freshly opened `Ready` (re-detects FTS, drops caches, re-baselines `data_version`).
    /// The state's write lock is held from the copy through the reopen, so no other command can
    /// obtain (or keep using) the old `Ready` in between: `ready()` blocks until this returns.
    /// If the copy succeeded but the reopen fails, the result carries a `warning` and the old
    /// connection (which already sees the restored content) stays in place.
    pub fn restore_backup(&self, file_name: &str) -> Result<backup::RestoreResult, AppError> {
        let mut guard = match self.inner.write() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let old = guard.clone()?;
        old.ensure_writable()?;
        let db_path = db_path_of(&old)?;
        let mut res = {
            let mut conn = old.lock_conn()?;
            backup::restore_into(&mut conn, &db_path, file_name)?
        };
        match Ready::open(&db_path, false) {
            Ok(fresh) => {
                fresh.backed_up.store(true, Ordering::SeqCst);
                if let Ok(mut l) = fresh.last_backup.lock() {
                    *l = Some(backup::backups_dir(&db_path).join(&res.pre_restore_backup));
                }
                *guard = Ok(fresh);
            }
            Err(e) => {
                res.warning = Some(format!(
                    "La copia se restauró, pero no se pudo reabrir la base: {e}; reinicie la aplicación."
                ));
            }
        }
        Ok(res)
    }

    /// Persists the flag and reopens the same database in the requested mode.
    pub fn set_read_only(&self, read_only: bool) -> Result<Ready, AppError> {
        let mut s = self.settings_now();
        let path = match self.ready() {
            Ok(r) if r.db_path.is_some() => r.db_path.unwrap(),
            _ => db::resolve_app_db_path(&s)?,
        };
        let ready = Ready::open(&path, read_only || db::env_read_only())?;
        s.read_only = read_only;
        self.persist(s)?;
        self.replace(Ok(ready.clone()));
        Ok(ready)
    }
}

async fn run_blocking<T, F>(state: &AppState, f: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce(&Connection, &Ready) -> Result<T, AppError> + Send + 'static,
{
    let ready = state.ready()?;
    tauri::async_runtime::spawn_blocking(move || {
        let conn = ready.lock_conn()?;
        f(&conn, &ready)
    })
    .await
    .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

/// Write path: read-only check, backup-before-first-write, then `f` (blocking thread).
async fn run_write<T, F>(state: &AppState, f: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce(&Connection) -> Result<T, AppError> + Send + 'static,
{
    let ready = state.ready()?;
    ready.ensure_writable()?;
    tauri::async_runtime::spawn_blocking(move || ready.with_write(f))
        .await
        .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

fn library_info_cached(c: &Connection, r: &Ready) -> Result<LibraryInfo, AppError> {
    let mut cache = r
        .info_cache
        .lock()
        .map_err(|_| AppError::Database("Caché no disponible (lock envenenado).".into()))?;
    if let Some(info) = cache.as_ref() {
        return Ok(info.clone());
    }
    let info = repo::library_info(c, &r.db_file_name, r.read_only)?;
    *cache = Some(info.clone());
    Ok(info)
}

#[tauri::command]
pub async fn get_library_info(state: State<'_, AppState>) -> Result<LibraryInfo, AppError> {
    run_blocking(&state, library_info_cached).await
}

#[tauri::command]
pub async fn get_dashboard(state: State<'_, AppState>) -> Result<Dashboard, AppError> {
    run_blocking(&state, |c, _| repo::dashboard(c)).await
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

fn header_str<'a>(h: &'a tauri::http::HeaderMap, name: &str) -> Option<&'a str> {
    h.get(name).and_then(|v| v.to_str().ok())
}

/// Replaces the cover. Raw image bytes in the IPC body (no base64); headers `x-game-id` and
/// `x-expected-revision`. Processing (decode, resize, WebP) runs outside the DB lock.
#[tauri::command]
pub async fn set_cover(
    state: State<'_, AppState>,
    request: tauri::ipc::Request<'_>,
) -> Result<GameDetail, AppError> {
    let ready = state.ready()?;
    ready.ensure_writable()?;
    let id: i64 = header_str(request.headers(), "x-game-id")
        .and_then(|v| v.trim().parse().ok())
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::InvalidInput("Falta la cabecera x-game-id válida.".into()))?;
    let revision = header_str(request.headers(), "x-expected-revision")
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::InvalidInput("Falta la cabecera x-expected-revision.".into()))?
        .to_string();
    let bytes = match request.body() {
        tauri::ipc::InvokeBody::Raw(b) => b.clone(),
        _ => {
            return Err(AppError::InvalidInput(
                "La portada debe enviarse como bytes en bruto.".into(),
            ))
        }
    };
    let opts = WriteOptions {
        allow_duplicate_code: false,
        expected_revision: Some(revision),
    };
    let webp = tauri::async_runtime::spawn_blocking(move || images::process_cover(&bytes))
        .await
        .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))??;
    run_write(&state, move |c| write::set_cover(c, id, &webp, &opts)).await
}

#[tauri::command]
pub async fn remove_cover(
    state: State<'_, AppState>,
    id: i64,
    opts: Option<WriteOptions>,
) -> Result<GameDetail, AppError> {
    let opts = opts.unwrap_or_default();
    run_write(&state, move |c| write::remove_cover(c, id, &opts)).await
}

#[tauri::command]
pub async fn suggest_game(
    state: State<'_, AppState>,
    req: SuggestRequest,
) -> Result<Suggestion, AppError> {
    run_blocking(&state, move |c, _| repo::suggest(c, &req)).await
}

/// Compares `PRAGMA data_version` with the last seen value (ADR-010). When it changed
/// (another connection committed), the cached `LibraryInfo` is invalidated.
#[tauri::command]
pub async fn check_data_version(state: State<'_, AppState>) -> Result<DataVersion, AppError> {
    run_blocking(&state, |c, r| {
        let v: i64 = c.query_row("PRAGMA data_version", [], |row| row.get(0))?;
        let prev = r.data_version.swap(v, Ordering::SeqCst);
        let changed = prev != v;
        if changed {
            r.invalidate_info();
        }
        Ok(DataVersion {
            version: v,
            changed,
        })
    })
    .await
}

#[tauri::command]
pub async fn get_vocabulary(state: State<'_, AppState>) -> Result<Vocabulary, AppError> {
    run_blocking(&state, |c, _| write::vocabulary(c)).await
}

#[tauri::command]
pub async fn get_session_info(state: State<'_, AppState>) -> Result<SessionInfo, AppError> {
    Ok(state.ready()?.session_info())
}

#[tauri::command]
pub async fn create_game(
    state: State<'_, AppState>,
    input: GameInput,
    opts: Option<WriteOptions>,
) -> Result<GameDetail, AppError> {
    let opts = opts.unwrap_or_default();
    state.ready()?.ensure_writable()?;
    let input = write::validate_input(&input)?;
    run_write(&state, move |c| write::create_game(c, &input, &opts)).await
}

#[tauri::command]
pub async fn update_game(
    state: State<'_, AppState>,
    id: i64,
    input: GameInput,
    opts: Option<WriteOptions>,
) -> Result<GameDetail, AppError> {
    let opts = opts.unwrap_or_default();
    state.ready()?.ensure_writable()?;
    let input = write::validate_input(&input)?;
    run_write(&state, move |c| write::update_game(c, id, &input, &opts)).await
}

#[tauri::command]
pub async fn set_favorite(
    state: State<'_, AppState>,
    id: i64,
    favorite: bool,
    opts: Option<WriteOptions>,
) -> Result<GameDetail, AppError> {
    let opts = opts.unwrap_or_default();
    run_write(&state, move |c| write::set_favorite(c, id, favorite, &opts)).await
}

#[tauri::command]
pub async fn delete_game(
    state: State<'_, AppState>,
    id: i64,
    opts: Option<WriteOptions>,
) -> Result<(), AppError> {
    let opts = opts.unwrap_or_default();
    run_write(&state, move |c| write::delete_game(c, id, &opts)).await
}

#[tauri::command]
pub async fn create_backup(state: State<'_, AppState>) -> Result<BackupResult, AppError> {
    let ready = state.ready()?;
    tauri::async_runtime::spawn_blocking(move || {
        ready.manual_backup().map(|p| BackupResult {
            path: p.to_string_lossy().into_owned(),
        })
    })
    .await
    .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

fn db_path_of(ready: &Ready) -> Result<PathBuf, AppError> {
    ready
        .db_path
        .clone()
        .ok_or_else(|| AppError::Database("Se desconoce la ruta de la base.".into()))
}

#[tauri::command]
pub async fn list_backups(
    state: State<'_, AppState>,
) -> Result<Vec<backup::BackupEntry>, AppError> {
    let ready = state.ready()?;
    let p = db_path_of(&ready)?;
    tauri::async_runtime::spawn_blocking(move || backup::list_backups(&p))
        .await
        .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

#[tauri::command]
pub async fn verify_backup(
    state: State<'_, AppState>,
    file_name: String,
) -> Result<backup::BackupCheck, AppError> {
    let ready = state.ready()?;
    let p = db_path_of(&ready)?;
    tauri::async_runtime::spawn_blocking(move || backup::verify_backup(&p, &file_name))
        .await
        .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

#[tauri::command]
pub async fn restore_backup<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    file_name: String,
) -> Result<backup::RestoreResult, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        app.state::<AppState>().restore_backup(&file_name)
    })
    .await
    .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

#[tauri::command]
pub async fn check_database(state: State<'_, AppState>) -> Result<backup::BackupCheck, AppError> {
    run_blocking(&state, |c, _| backup::check_conn(c)).await
}

#[tauri::command]
pub async fn export_library(
    state: State<'_, AppState>,
    path: String,
    format: String,
) -> Result<backup::ExportResult, AppError> {
    run_blocking(&state, move |c, r| {
        backup::export_library(c, &db_path_of(r)?, &path, &format)
    })
    .await
}

// --- Contraseñas por página (ADR-019). Keyring calls can block (D-Bus, unlock prompts).

async fn keyring_blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::SecureStorage(format!("Tarea interrumpida: {e}")))?
}

#[tauri::command]
pub async fn page_passwords_status(paginas: Vec<String>) -> Result<Vec<String>, AppError> {
    if paginas.len() > 500 {
        return Err(AppError::InvalidInput("Demasiadas páginas.".into()));
    }
    keyring_blocking(move || passwords::with_password(passwords::SERVICE, &paginas)).await
}

/// Only for an explicit «Mostrar»; copying never sends the secret to the webview.
#[tauri::command]
pub async fn get_page_password(pagina: String) -> Result<Option<String>, AppError> {
    keyring_blocking(move || passwords::get(passwords::SERVICE, &pagina)).await
}

#[tauri::command]
pub async fn set_page_password(pagina: String, password: String) -> Result<(), AppError> {
    keyring_blocking(move || passwords::set(passwords::SERVICE, &pagina, &password)).await
}

#[tauri::command]
pub async fn delete_page_password(pagina: String) -> Result<(), AppError> {
    keyring_blocking(move || passwords::delete(passwords::SERVICE, &pagina)).await
}

#[tauri::command]
pub async fn copy_page_password<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    pagina: String,
) -> Result<(), AppError> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    keyring_blocking(move || {
        let pw = passwords::get(passwords::SERVICE, &pagina)?.ok_or_else(|| {
            AppError::NotFound("Esta página no tiene contraseña guardada.".into())
        })?;
        app.clipboard()
            .write_text(pw)
            .map_err(|_| AppError::SecureStorage("No se pudo copiar al portapapeles.".into()))
    })
    .await
}

async fn reopened_info(ready: Ready) -> Result<LibraryInfo, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let conn = ready.lock_conn()?;
        library_info_cached(&conn, &ready)
    })
    .await
    .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

#[tauri::command]
pub async fn set_db_path(
    state: State<'_, AppState>,
    path: String,
) -> Result<LibraryInfo, AppError> {
    let ready = state.set_db_path(&path)?;
    reopened_info(ready).await
}

#[tauri::command]
pub async fn set_read_only(
    state: State<'_, AppState>,
    read_only: bool,
) -> Result<SessionInfo, AppError> {
    Ok(state.set_read_only(read_only)?.session_info())
}

#[tauri::command]
pub async fn get_migration_status(state: State<'_, AppState>) -> Result<MigrationStatus, AppError> {
    run_blocking(&state, |c, _| migrate::status(c)).await
}

/// Only by explicit user action. Backup first, then one transaction; no-op without pending steps.
#[tauri::command]
pub async fn run_migrations(state: State<'_, AppState>) -> Result<MigrationRun, AppError> {
    let ready = state.ready()?;
    ready.ensure_writable()?;
    tauri::async_runtime::spawn_blocking(move || run_migrations_sync(&ready))
        .await
        .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

pub fn run_migrations_sync(ready: &Ready) -> Result<MigrationRun, AppError> {
    ready.ensure_writable()?;
    let pending = {
        let c = ready.lock_conn()?;
        migrate::status(&c)?.pending.len()
    };
    if pending == 0 {
        return Ok(MigrationRun {
            applied: vec![],
            backup_path: String::new(),
        });
    }
    let applied = ready.with_write_forced_backup(migrate::run)?;
    let backup_path = ready.session_info().last_backup.unwrap_or_default();
    Ok(MigrationRun {
        applied,
        backup_path,
    })
}

// ------------------------------------------------------------------ Fase 6: importador

/// Analiza `root` (solo lectura: nombres de entradas) y devuelve la vista previa. Emite progreso
/// por `on_progress`; se cancela con `cancel_scan`.
#[tauri::command]
pub async fn scan_import(
    state: State<'_, AppState>,
    root: String,
    on_progress: tauri::ipc::Channel<importer::ScanProgress>,
) -> Result<importer::ImportPreview, AppError> {
    let ready = state.ready()?;
    let root_path = PathBuf::from(root.trim());
    let guard = importer::ScanGuard::acquire()?;
    tauri::async_runtime::spawn_blocking(move || {
        let cancel = guard.cancel_flag();
        let _guard = guard;
        let raw = importer::scan_tree(&root_path, cancel, &mut |p| {
            let _ = on_progress.send(p.clone());
        })?;
        if raw.cancelled {
            return Ok(importer::ImportPreview {
                root: root_path.to_string_lossy().into_owned(),
                items: vec![],
                defaults: Default::default(),
                counts: Default::default(),
                cancelled: true,
                truncated: false,
                scanned: raw.scanned,
                unreadable: raw.unreadable,
            });
        }
        let conn = ready.lock_conn()?;
        importer::build_preview(&conn, &root_path.to_string_lossy(), raw)
    })
    .await
    .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

#[tauri::command]
pub async fn cancel_scan() -> Result<(), AppError> {
    importer::request_cancel();
    Ok(())
}

#[tauri::command]
pub async fn run_import(
    state: State<'_, AppState>,
    req: importer::ImportRequest,
) -> Result<importer::ImportResult, AppError> {
    let ready = state.ready()?;
    let history = state.history_path();
    tauri::async_runtime::spawn_blocking(move || {
        importer::run_import_sync(&ready, &req, history.as_deref())
    })
    .await
    .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

#[tauri::command]
pub async fn get_import_history(
    state: State<'_, AppState>,
) -> Result<Vec<importer::HistoryEntry>, AppError> {
    let Some(path) = state.history_path() else {
        return Ok(vec![]);
    };
    let key = importer::db_key(&state.ready()?);
    tauri::async_runtime::spawn_blocking(move || importer::read_history(&path, key.as_deref(), 20))
        .await
        .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))
}

#[tauri::command]
pub async fn undo_last_import(
    state: State<'_, AppState>,
) -> Result<importer::UndoResult, AppError> {
    let ready = state.ready()?;
    let path = state
        .history_path()
        .ok_or_else(|| AppError::NotFound("No hay historial de importaciones.".into()))?;
    tauri::async_runtime::spawn_blocking(move || importer::undo_last_import_sync(&ready, &path))
        .await
        .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

// ------------------------------------------------------------------ Fase 7: mantenimiento

#[tauri::command]
pub async fn maintenance_summary(
    state: State<'_, AppState>,
) -> Result<Vec<maintenance::CheckCount>, AppError> {
    run_blocking(&state, |c, _| maintenance::summary(c)).await
}

#[tauri::command]
pub async fn maintenance_list(
    state: State<'_, AppState>,
    req: maintenance::ListRequest,
) -> Result<maintenance::IssuePage, AppError> {
    run_blocking(&state, move |c, _| maintenance::list(c, &req)).await
}

/// Comprueba la existencia de las rutas guardadas (solo metadatos). Las filas se leen con el
/// bloqueo de la DB y se suelta antes de tocar el disco.
#[tauri::command]
pub async fn scan_missing_paths(
    state: State<'_, AppState>,
    on_progress: tauri::ipc::Channel<maintenance::MissingProgress>,
) -> Result<maintenance::MissingScan, AppError> {
    let ready = state.ready()?;
    let guard = maintenance::MaintGuard::acquire()?;
    tauri::async_runtime::spawn_blocking(move || {
        let cancel = guard.cancel_flag();
        let _guard = guard;
        let rows = {
            let conn = ready.lock_conn()?;
            maintenance::collect_rutas(&conn)?
        };
        Ok(maintenance::check_rutas(rows, cancel, &mut |p| {
            let _ = on_progress.send(p.clone());
        }))
    })
    .await
    .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

#[tauri::command]
pub async fn cancel_maintenance_scan() -> Result<(), AppError> {
    maintenance::request_cancel();
    Ok(())
}

#[tauri::command]
pub async fn list_orphan_tags(
    state: State<'_, AppState>,
) -> Result<Vec<maintenance::OrphanTag>, AppError> {
    run_blocking(&state, |c, _| maintenance::list_orphan_tags(c)).await
}

#[tauri::command]
pub async fn search_index_status(
    state: State<'_, AppState>,
) -> Result<maintenance::IndexStatus, AppError> {
    run_blocking(&state, |c, _| maintenance::search_index_status(c)).await
}

#[tauri::command]
pub async fn preview_batch(
    state: State<'_, AppState>,
    req: maintenance::BatchRequest,
) -> Result<maintenance::BatchPreview, AppError> {
    run_blocking(&state, move |c, _| maintenance::preview_batch(c, &req)).await
}

#[tauri::command]
pub async fn apply_batch(
    state: State<'_, AppState>,
    req: maintenance::BatchRequest,
    revisions: std::collections::HashMap<i64, String>,
) -> Result<maintenance::BatchResult, AppError> {
    let ready = state.ready()?;
    let dir = state.undo_dir();
    tauri::async_runtime::spawn_blocking(move || {
        maintenance::apply_batch(&ready, &req, &revisions, dir.as_deref())
    })
    .await
    .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

#[tauri::command]
pub async fn delete_orphan_tags(
    state: State<'_, AppState>,
) -> Result<maintenance::BatchResult, AppError> {
    let ready = state.ready()?;
    let dir = state.undo_dir();
    tauri::async_runtime::spawn_blocking(move || {
        maintenance::delete_orphan_tags(&ready, dir.as_deref())
    })
    .await
    .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

#[tauri::command]
pub async fn rebuild_search_index(
    state: State<'_, AppState>,
) -> Result<maintenance::IndexStatus, AppError> {
    let ready = state.ready()?;
    tauri::async_runtime::spawn_blocking(move || maintenance::rebuild_search_index(&ready))
        .await
        .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}

#[tauri::command]
pub async fn list_batches(
    state: State<'_, AppState>,
) -> Result<Vec<maintenance::BatchEntry>, AppError> {
    let ready = state.ready()?;
    let Some(dir) = state.undo_dir() else {
        return Ok(vec![]);
    };
    tauri::async_runtime::spawn_blocking(move || maintenance::list_batches(&ready, &dir))
        .await
        .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))
}

#[tauri::command]
pub async fn undo_last_batch(
    state: State<'_, AppState>,
) -> Result<maintenance::UndoBatchResult, AppError> {
    let ready = state.ready()?;
    let dir = state
        .undo_dir()
        .ok_or_else(|| AppError::NotFound("No hay lotes que deshacer.".into()))?;
    tauri::async_runtime::spawn_blocking(move || maintenance::undo_last_batch(&ready, &dir))
        .await
        .map_err(|e| AppError::Database(format!("Tarea interrumpida: {e}")))?
}
