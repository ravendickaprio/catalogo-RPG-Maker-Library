//! Backups con la API de backup de SQLite (ADR-012). Solo escribe y borra dentro de `<dbDir>/backups/`.
use crate::error::AppError;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const KEEP: usize = 20;
/// Newest `-prerestore` backups kept (rotated apart from the regular ones).
pub const KEEP_PRE_RESTORE: usize = 5;

/// `YYYYMMDD-HHMMSS` in UTC for the given Unix seconds (civil-from-days, no external crates).
pub fn utc_stamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}{mo:02}{d:02}-{h:02}{m:02}{s:02}")
}

/// `YYYY-MM-DDTHH:MM:SSZ` (UTC) for the given Unix seconds.
pub fn iso_utc(secs: u64) -> String {
    let s = utc_stamp(secs); // YYYYMMDD-HHMMSS
    format!(
        "{}-{}-{}T{}:{}:{}Z",
        &s[0..4],
        &s[4..6],
        &s[6..8],
        &s[9..11],
        &s[11..13],
        &s[13..15]
    )
}

/// Segundos desde la época Unix (0 si el reloj es anterior).
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn now_stamp() -> String {
    utc_stamp(now_secs())
}

fn db_err(msg: &str, e: impl std::fmt::Display) -> AppError {
    AppError::Database(format!("Backup fallido: {msg}: {e}"))
}

/// Backup directory next to the database.
pub fn backups_dir(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default()
        .join("backups")
}

pub const PRE_RESTORE_SUFFIX: &str = "-prerestore";
/// Maximum time to keep retrying while another connection holds a lock.
const BUSY_DEADLINE: Duration = Duration::from_secs(3);

/// Runs a backup stepping `pages` at a time (-1 = all in one atomic step); gives up with `Busy`
/// when the other side stays locked longer than `BUSY_DEADLINE` (`run_to_completion` would spin).
fn run_bounded(b: &rusqlite::backup::Backup<'_, '_>, pages: i32) -> Result<(), AppError> {
    use rusqlite::backup::StepResult;
    let start = Instant::now();
    loop {
        match b.step(pages).map_err(AppError::from)? {
            StepResult::Done => return Ok(()),
            StepResult::More => {}
            _ => {
                if start.elapsed() > BUSY_DEADLINE {
                    return Err(AppError::Busy(
                        "La base de datos está ocupada por otra aplicación. Inténtelo de nuevo."
                            .into(),
                    ));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

/// Copies the live database with the SQLite backup API into `<dbDir>/backups/<name>.bak-<stamp>`
/// and rotates old ones. Returns the new file. On failure nothing is left behind.
pub fn create_backup(conn: &Connection, db_path: &Path) -> Result<PathBuf, AppError> {
    create_backup_named(conn, db_path, "")
}

/// Like `create_backup`, appending `suffix` to the file name (`-prerestore`). The new file is
/// never removed by the rotation that follows.
pub fn create_backup_named(
    conn: &Connection,
    db_path: &Path,
    suffix: &str,
) -> Result<PathBuf, AppError> {
    let file_name = db_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::Database("Backup fallido: ruta de base sin nombre.".into()))?;
    let dir = backups_dir(db_path);
    std::fs::create_dir_all(&dir).map_err(|e| db_err("no se pudo crear backups/", e))?;
    let prefix = format!("{file_name}.bak-");
    let stamp = now_stamp();
    let mut dest = dir.join(format!("{prefix}{stamp}{suffix}"));
    let mut n = 1;
    while dest.exists() {
        dest = dir.join(format!("{prefix}{stamp}-{n}{suffix}"));
        n += 1;
    }
    let result = (|| -> Result<(), AppError> {
        let mut dst =
            Connection::open(&dest).map_err(|e| db_err("no se pudo crear el archivo", e))?;
        {
            let b = rusqlite::backup::Backup::new(conn, &mut dst)
                .map_err(|e| db_err("no se pudo iniciar", e))?;
            run_bounded(&b, 256)?;
        }
        let ok: String = dst
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(|e| db_err("verificación", e))?;
        if ok != "ok" {
            return Err(AppError::Database(format!(
                "Backup fallido: la copia no pasó integrity_check ({ok})."
            )));
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&dest);
        return Err(e);
    }
    let keep_name = dest.file_name().map(|n| n.to_string_lossy().into_owned());
    // Regular and pre-restore backups rotate separately so one kind never evicts the other.
    rotate_class(&dir, &prefix, KEEP, keep_name.as_deref(), false);
    rotate_class(&dir, &prefix, KEEP_PRE_RESTORE, keep_name.as_deref(), true);
    Ok(dest)
}

/// Keeps the `keep` newest files in `dir` whose name starts with `prefix` (names sort by
/// timestamp). Never touches anything else.
pub fn rotate(dir: &Path, prefix: &str, keep: usize) {
    rotate_protect(dir, prefix, keep, None)
}

/// Like `rotate` but never deletes the file named `protect`.
pub fn rotate_protect(dir: &Path, prefix: &str, keep: usize, protect: Option<&str>) {
    rotate_filtered(dir, prefix, keep, protect, |_| true)
}

/// Rotates only the regular (`pre == false`) or only the `-prerestore` (`pre == true`) files.
pub fn rotate_class(dir: &Path, prefix: &str, keep: usize, protect: Option<&str>, pre: bool) {
    rotate_filtered(dir, prefix, keep, protect, |n| {
        n.ends_with(PRE_RESTORE_SUFFIX) == pre
    })
}

fn rotate_filtered(
    dir: &Path,
    prefix: &str,
    keep: usize,
    protect: Option<&str>,
    class: impl Fn(&str) -> bool,
) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with(prefix) && class(n))
        .collect();
    names.sort();
    if names.len() > keep {
        for n in &names[..names.len() - keep] {
            if Some(n.as_str()) != protect {
                let _ = std::fs::remove_file(dir.join(n));
            }
        }
    }
}

// ------------------------------------------------------------------ fase 9: listar/verificar/restaurar

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub file_name: String,
    pub size: u64,
    pub modified: String,
    pub pre_restore: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCheck {
    pub ok: bool,
    pub integrity: String,
    pub foreign_keys: i64,
    pub fts_ok: Option<bool>,
    pub games: i64,
    pub schema_ok: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub restored_from: String,
    pub pre_restore_backup: String,
    pub games: i64,
    /// Set when the copy succeeded but the database could not be reopened.
    pub warning: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExportResult {
    pub path: String,
    pub rows: i64,
}

fn db_file_name(db_path: &Path) -> Result<String, AppError> {
    db_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::Database("Ruta de base sin nombre.".into()))
}

/// Backups of this database (`<name>.bak-*`), newest first.
pub fn list_backups(db_path: &Path) -> Result<Vec<BackupEntry>, AppError> {
    let prefix = format!("{}.bak-", db_file_name(db_path)?);
    let dir = backups_dir(db_path);
    let rd = match std::fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(AppError::Database(format!("No se pudo leer backups/: {e}"))),
    };
    let mut out: Vec<(u64, BackupEntry)> = Vec::new();
    for e in rd.filter_map(|e| e.ok()) {
        let Ok(name) = e.file_name().into_string() else {
            continue;
        };
        if !name.starts_with(&prefix) {
            continue;
        }
        let Ok(md) = e.metadata() else { continue };
        if !md.is_file() {
            continue;
        }
        let secs = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        out.push((
            secs,
            BackupEntry {
                pre_restore: name.ends_with(PRE_RESTORE_SUFFIX),
                file_name: name,
                size: md.len(),
                modified: iso_utc(secs),
            },
        ));
    }
    out.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.file_name.cmp(&a.1.file_name))
    });
    Ok(out.into_iter().map(|(_, e)| e).collect())
}

/// Resolves `file_name` (a bare name) inside `<dbDir>/backups/`, confirming with `canonicalize`.
pub fn resolve_backup(db_path: &Path, file_name: &str) -> Result<PathBuf, AppError> {
    let bad = || AppError::InvalidInput("Nombre de backup no válido.".into());
    let p = Path::new(file_name);
    let single_normal = {
        let mut c = p.components();
        matches!(
            (c.next(), c.next()),
            (Some(std::path::Component::Normal(_)), None)
        )
    };
    if file_name.is_empty()
        || file_name.contains(['/', '\\', ':', '\0'])
        || file_name == ".."
        || file_name == "."
        || p.is_absolute()
        || !single_normal
    {
        return Err(bad());
    }
    let dir = std::fs::canonicalize(backups_dir(db_path))
        .map_err(|_| AppError::NotFound("No hay carpeta de backups.".into()))?;
    let full = std::fs::canonicalize(dir.join(file_name))
        .map_err(|_| AppError::NotFound(format!("No existe el backup '{file_name}'.")))?;
    if !full.starts_with(&dir) || full.parent() != Some(dir.as_path()) || !full.is_file() {
        return Err(bad());
    }
    Ok(full)
}

/// Checks a database through any connection (read-only is enough).
pub fn check_conn(conn: &Connection) -> Result<BackupCheck, AppError> {
    let mut lines: Vec<String> = Vec::new();
    {
        let mut st = conn.prepare("PRAGMA integrity_check")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        for r in rows.take(5) {
            lines.push(r?);
        }
    }
    let integrity = if lines.len() == 1 && lines[0] == "ok" {
        "ok".to_string()
    } else {
        lines.join("; ")
    };
    let foreign_keys = {
        let mut st = conn.prepare("PRAGMA foreign_key_check")?;
        let mut n = 0i64;
        let mut rows = st.query([])?;
        while rows.next()?.is_some() {
            n += 1;
        }
        n
    };
    let schema_ok = crate::db::validate_schema(conn).is_ok();
    let games = if schema_ok {
        conn.query_row("SELECT count(*) FROM juegos", [], |r| r.get(0))?
    } else {
        0
    };
    let fts_ok = if crate::repo::table_exists(conn, "juegos_fts")? {
        Some(crate::maintenance::fts_ok(conn)?)
    } else {
        None
    };
    Ok(BackupCheck {
        ok: integrity == "ok" && schema_ok,
        integrity,
        foreign_keys,
        fts_ok,
        games,
        schema_ok,
    })
}

/// Opens `path` read-only and checks it. A file that is not a database yields `ok = false`.
pub fn check_file(path: &Path) -> Result<BackupCheck, AppError> {
    let failed = |msg: String| BackupCheck {
        ok: false,
        integrity: msg,
        foreign_keys: 0,
        fts_ok: None,
        games: 0,
        schema_ok: false,
    };
    let conn = match crate::db::open_read_only(path) {
        Ok(c) => c,
        Err(e) => return Ok(failed(e.to_string())),
    };
    match check_conn(&conn) {
        Ok(c) => Ok(c),
        Err(busy @ AppError::Busy(_)) => Err(busy),
        Err(e) => Ok(failed(e.to_string())),
    }
}

pub fn verify_backup(db_path: &Path, file_name: &str) -> Result<BackupCheck, AppError> {
    check_file(&resolve_backup(db_path, file_name)?)
}

/// Replaces the content of `main` (the open read-write connection) with the backup `file_name`.
/// Order: verify source -> pre-restore backup of the current DB -> atomic backup-API copy.
/// The caller re-opens its state afterwards (FTS detection, caches, data_version).
pub fn restore_into(
    main: &mut Connection,
    db_path: &Path,
    file_name: &str,
) -> Result<RestoreResult, AppError> {
    let src_path = resolve_backup(db_path, file_name)?;
    let check = check_file(&src_path)?;
    if !check.ok {
        let why = if !check.schema_ok {
            "no tiene el esquema del catálogo".to_string()
        } else {
            format!("integrity_check: {}", check.integrity)
        };
        return Err(AppError::InvalidInput(format!(
            "El backup '{file_name}' no es válido ({why}); no se restauró nada."
        )));
    }
    let pre = create_backup_named(main, db_path, PRE_RESTORE_SUFFIX)?;
    let src = crate::db::open_read_only(&src_path)?;
    {
        let b = rusqlite::backup::Backup::new(&src, main)?;
        run_bounded(&b, -1)?;
    }
    let games: i64 = main.query_row("SELECT count(*) FROM juegos", [], |r| r.get(0))?;
    Ok(RestoreResult {
        restored_from: file_name.to_string(),
        pre_restore_backup: pre
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        games,
        warning: None,
    })
}

// ------------------------------------------------------------------ exportación

const EXPORT_COLUMNS: [&str; 12] = [
    "id",
    "titulo",
    "codigo",
    "estado",
    "idioma",
    "tipo",
    "pagina",
    "puntuacion",
    "ruta",
    "descripcion",
    "tags",
    "tiene_portada",
];

fn invalid(msg: &str) -> AppError {
    AppError::InvalidInput(msg.to_string())
}

/// Canonical form of a (possibly not yet existing) file: canonical parent + file name.
fn canon_target(p: &Path) -> Option<PathBuf> {
    match std::fs::canonicalize(p) {
        Ok(c) => Some(c),
        Err(_) => {
            let parent = std::fs::canonicalize(p.parent()?).ok()?;
            Some(parent.join(p.file_name()?))
        }
    }
}

fn validate_export_path(path: &Path, format: &str, db_path: &Path) -> Result<PathBuf, AppError> {
    if !path.is_absolute() {
        return Err(invalid("La ruta de exportación debe ser absoluta."));
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if ext != format {
        return Err(AppError::InvalidInput(format!(
            "La extensión del archivo debe ser .{format}."
        )));
    }
    let parent = path
        .parent()
        .filter(|p| p.is_dir())
        .ok_or_else(|| invalid("La carpeta de destino no existe."))?;
    if path.is_dir() {
        return Err(invalid("La ruta de exportación es una carpeta."));
    }
    let target = canon_target(path).ok_or_else(|| invalid("Ruta de exportación no válida."))?;
    if let Some(db) = canon_target(db_path) {
        // Both the final (symlink-resolved) target and the canonical parent of the requested path
        // must stay out of backups/ and must not be the open database.
        let bd = canon_target(&backups_dir(&db));
        let cparent = std::fs::canonicalize(parent).ok();
        let in_backups = |p: &Path| bd.as_ref().is_some_and(|bd| p.starts_with(bd));
        if target == db {
            return Err(invalid(
                "No se puede exportar sobre la base de datos abierta.",
            ));
        }
        if in_backups(&target) || cparent.as_deref().is_some_and(in_backups) {
            return Err(invalid(
                "No se puede exportar dentro de la carpeta backups/.",
            ));
        }
    }
    Ok(target)
}

fn csv_cell(v: &rusqlite::types::Value) -> String {
    use rusqlite::types::Value;
    let s = match v {
        Value::Null | Value::Blob(_) => String::new(),
        Value::Integer(i) => i.to_string(),
        Value::Real(f) => f.to_string(),
        Value::Text(t) => t.clone(),
    };
    // CSV formula injection (OWASP): neutralize text cells a spreadsheet would evaluate.
    let s = if matches!(v, Value::Text(_)) && s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s
    };
    if s.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

fn json_cell(v: &rusqlite::types::Value) -> String {
    use rusqlite::types::Value;
    match v {
        Value::Null | Value::Blob(_) => "null".into(),
        Value::Integer(i) => i.to_string(),
        Value::Real(f) => serde_json::Number::from_f64(*f)
            .map(|n| n.to_string())
            .unwrap_or_else(|| "null".into()),
        Value::Text(t) => serde_json::to_string(t).unwrap_or_else(|_| "\"\"".into()),
    }
}

/// Exports the catalog metadata (never the BLOBs) to `path` as CSV (UTF-8 with BOM, CRLF) or JSON.
/// Writes a temporary file next to the target and renames it over.
pub fn export_library(
    conn: &Connection,
    db_path: &Path,
    path: &str,
    format: &str,
) -> Result<ExportResult, AppError> {
    use std::io::Write;
    let format = match format {
        "csv" | "json" => format,
        _ => return Err(invalid("Formato de exportación no válido (csv o json).")),
    };
    let path = PathBuf::from(path);
    let target = validate_export_path(&path, format, db_path)?;
    let tmp = {
        let mut n = target.file_name().unwrap_or_default().to_os_string();
        n.push(format!(".tmp-{}", std::process::id()));
        target.with_file_name(n)
    };
    let io =
        |e: std::io::Error| AppError::Database(format!("No se pudo escribir la exportación: {e}"));
    let result = (|| -> Result<i64, AppError> {
        let mut w = std::io::BufWriter::new(std::fs::File::create(&tmp).map_err(io)?);
        let mut st = conn.prepare(
            "SELECT j.id, j.titulo, j.codigo, j.estado, j.idioma, j.tipo, j.pagina, j.puntuacion, \
             j.ruta, j.descripcion, \
             (SELECT group_concat(nombre, ', ') FROM (SELECT t.nombre AS nombre FROM juego_tags jt \
              JOIN tags t ON t.id = jt.tag_id WHERE jt.juego_id = j.id ORDER BY t.nombre)) AS tags, \
             CASE WHEN length(j.imagen) > 0 THEN 1 ELSE 0 END AS tiene_portada \
             FROM juegos j ORDER BY j.id",
        )?;
        let mut rows = st.query([])?;
        let mut n = 0i64;
        if format == "csv" {
            w.write_all(b"\xEF\xBB\xBF").map_err(io)?;
            w.write_all(format!("{}\r\n", EXPORT_COLUMNS.join(",")).as_bytes())
                .map_err(io)?;
        } else {
            w.write_all(b"[").map_err(io)?;
        }
        while let Some(r) = rows.next()? {
            let mut cells = Vec::with_capacity(EXPORT_COLUMNS.len());
            for i in 0..EXPORT_COLUMNS.len() {
                cells.push(r.get::<_, rusqlite::types::Value>(i)?);
            }
            if format == "csv" {
                let line: Vec<String> = cells.iter().map(csv_cell).collect();
                w.write_all(format!("{}\r\n", line.join(",")).as_bytes())
                    .map_err(io)?;
            } else {
                let fields: Vec<String> = EXPORT_COLUMNS
                    .iter()
                    .zip(&cells)
                    .map(|(k, v)| format!("\"{k}\":{}", json_cell(v)))
                    .collect();
                let sep = if n == 0 { "\n" } else { ",\n" };
                w.write_all(format!("{sep}{{{}}}", fields.join(",")).as_bytes())
                    .map_err(io)?;
            }
            n += 1;
        }
        if format == "json" {
            w.write_all(b"\n]\n").map_err(io)?;
        }
        w.flush().map_err(io)?;
        Ok(n)
    })();
    match result {
        Ok(n) => {
            if let Err(e) = std::fs::rename(&tmp, &target) {
                let _ = std::fs::remove_file(&tmp);
                return Err(io(e));
            }
            Ok(ExportResult {
                path: path.to_string_lossy().into_owned(),
                rows: n,
            })
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_known_dates() {
        assert_eq!(utc_stamp(0), "19700101-000000");
        assert_eq!(utc_stamp(951_782_400), "20000229-000000");
        assert_eq!(utc_stamp(1_700_000_000), "20231114-221320");
        assert_eq!(utc_stamp(1_793_000_000 + 3661), "20261026-083421");
        assert_eq!(iso_utc(1_700_000_000), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn rotate_keeps_newest_and_only_prefix() {
        let d = tempfile::TempDir::new().unwrap();
        for i in 0..25 {
            std::fs::write(d.path().join(format!("x.db.bak-20260101-0000{i:02}")), "a").unwrap();
        }
        std::fs::write(d.path().join("otro.txt"), "keep").unwrap();
        std::fs::write(d.path().join("y.db.bak-20260101-000000"), "keep").unwrap();
        rotate(d.path(), "x.db.bak-", 20);
        let n = std::fs::read_dir(d.path())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("x.db.bak-")
            })
            .count();
        assert_eq!(n, 20);
        assert!(d.path().join("otro.txt").exists());
        assert!(d.path().join("y.db.bak-20260101-000000").exists());
        assert!(!d.path().join("x.db.bak-20260101-000000").exists());
        assert!(d.path().join("x.db.bak-20260101-000024").exists());
    }
}
