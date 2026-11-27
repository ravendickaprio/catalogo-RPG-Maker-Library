use crate::error::AppError;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Guard used ONLY by the bench/parity command-line tools (CLAUDE.md: benchmarks never run on the
/// real DB). The app itself no longer rejects `juegos.db` (ADR-012).
pub fn check_path_allowed(path: &Path, allow_primary: bool) -> Result<(), AppError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if name == "juegos.db" && !allow_primary {
        return Err(AppError::Config(
            "Se rechaza abrir 'juegos.db' (base principal). Use una copia o defina RPGLIB_ALLOW_PRIMARY_DB=1."
                .into(),
        ));
    }
    Ok(())
}

fn strip_verbatim(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        if !rest.starts_with("UNC\\") {
            return PathBuf::from(rest);
        }
    }
    p
}

pub fn resolve_file(raw: &Path) -> Result<PathBuf, AppError> {
    canonical_file(raw)
}

fn canonical_file(raw: &Path) -> Result<PathBuf, AppError> {
    let path = std::fs::canonicalize(raw).map_err(|e| {
        AppError::Config(format!(
            "No se encontró la base de datos en '{}': {}.",
            raw.display(),
            e
        ))
    })?;
    let path = strip_verbatim(path);
    if !path.is_file() {
        return Err(AppError::Config(format!(
            "La ruta '{}' no es un archivo.",
            path.display()
        )));
    }
    Ok(path)
}

/// `RPGLIB_DB_PATH` or the dev copy (used by the bench/parity tools; the app uses
/// `resolve_app_db_path`).
pub fn resolve_db_path() -> Result<PathBuf, AppError> {
    let raw = match std::env::var_os("RPGLIB_DB_PATH") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dev-data/juegos.dev.db"
        )),
    };
    canonical_file(&raw)
}

/// Resolution order for the app (ADR-012): `RPGLIB_DB_PATH` -> `settings.dbPath` -> dev copy.
pub fn resolve_app_db_path(settings: &crate::settings::Settings) -> Result<PathBuf, AppError> {
    let env_set = std::env::var_os("RPGLIB_DB_PATH").is_some_and(|v| !v.is_empty());
    if !env_set {
        if let Some(p) = settings.db_path.as_deref().filter(|p| !p.trim().is_empty()) {
            return canonical_file(Path::new(p));
        }
    }
    resolve_db_path()
}

/// True when `RPGLIB_READ_ONLY=1`.
pub fn env_read_only() -> bool {
    std::env::var("RPGLIB_READ_ONLY").is_ok_and(|v| v == "1")
}

/// `rpg_fold(text)`: Unicode lowercase (NULL -> ''), used by the LIKE fallback for non-ASCII queries.
pub fn register_functions(conn: &Connection) -> Result<(), AppError> {
    use rusqlite::functions::FunctionFlags;
    conn.create_scalar_function(
        "rpg_fold",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            Ok(ctx
                .get::<Option<String>>(0)?
                .map(|s| s.to_lowercase())
                .unwrap_or_default())
        },
    )?;
    Ok(())
}

pub fn open_read_only(path: &Path) -> Result<Connection, AppError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_URI;
    let conn = Connection::open_with_flags(path, flags)
        .map_err(|e| AppError::Config(format!("No se pudo abrir la base en solo lectura: {e}")))?;
    conn.busy_timeout(Duration::from_millis(2000))?;
    conn.execute_batch("PRAGMA query_only = ON;")?;
    register_functions(&conn)?;
    Ok(conn)
}

/// Opens read-write WITHOUT `SQLITE_OPEN_CREATE` (never creates a new file).
pub fn open_read_write(path: &Path) -> Result<Connection, AppError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_URI;
    let conn = Connection::open_with_flags(path, flags).map_err(|e| {
        AppError::Config(format!(
            "No se pudo abrir la base en lectura/escritura: {e}"
        ))
    })?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.busy_timeout(Duration::from_millis(5000))?;
    register_functions(&conn)?;
    Ok(conn)
}

/// Columns the legacy app writes/reads in `juegos`.
pub const JUEGOS_COLUMNS: [&str; 11] = [
    "id",
    "titulo",
    "descripcion",
    "estado",
    "idioma",
    "imagen",
    "ruta",
    "codigo",
    "pagina",
    "tipo",
    "puntuacion",
];

/// Verifies the catalog schema (never creates or alters anything). Error lists what is missing.
pub fn validate_schema(conn: &Connection) -> Result<(), AppError> {
    let mut missing: Vec<String> = Vec::new();
    let mut have: Vec<String> = Vec::new();
    {
        let mut st = conn
            .prepare("SELECT name FROM pragma_table_info('juegos')")
            .map_err(|e| AppError::Config(format!("No es una base válida: {e}")))?;
        let rows = st
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| AppError::Config(format!("No es una base válida: {e}")))?;
        for r in rows {
            have.push(
                r.map_err(|e| AppError::Config(format!("No es una base válida: {e}")))?
                    .to_lowercase(),
            );
        }
    }
    if have.is_empty() {
        missing.push("tabla juegos".into());
    } else {
        for c in JUEGOS_COLUMNS {
            if !have.iter().any(|h| h == c) {
                missing.push(format!("columna juegos.{c}"));
            }
        }
    }
    for t in ["tags", "juego_tags"] {
        if !crate::repo::table_exists(conn, t)
            .map_err(|e| AppError::Config(format!("No es una base válida: {e}")))?
        {
            missing.push(format!("tabla {t}"));
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(AppError::Config(format!(
            "La base de datos no tiene el esquema del catálogo. Falta: {}.",
            missing.join(", ")
        )))
    }
}

/// Opens (validating the schema) in the requested mode.
pub fn open_validated(path: &Path, read_only: bool) -> Result<Connection, AppError> {
    let conn = if read_only {
        open_read_only(path)?
    } else {
        open_read_write(path)?
    };
    validate_schema(&conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_rejects_primary() {
        assert!(check_path_allowed(Path::new("x/juegos.db"), false).is_err());
        assert!(check_path_allowed(Path::new("x/JUEGOS.DB"), false).is_err());
        assert!(check_path_allowed(Path::new("x/juegos.db"), true).is_ok());
        assert!(check_path_allowed(Path::new("x/juegos.dev.db"), false).is_ok());
        assert!(check_path_allowed(Path::new("x/otra.db"), false).is_ok());
    }
}
