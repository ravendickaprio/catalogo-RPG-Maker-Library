use crate::error::AppError;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Rejects the primary DB (`juegos.db`) unless explicitly allowed.
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

pub fn resolve_db_path() -> Result<PathBuf, AppError> {
    let raw = match std::env::var_os("RPGLIB_DB_PATH") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dev-data/juegos.dev.db"
        )),
    };
    let path = std::fs::canonicalize(&raw).map_err(|e| {
        AppError::Config(format!(
            "No se encontró la base de datos en '{}': {}. Defina RPGLIB_DB_PATH con la ruta de una copia.",
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
    let allow = std::env::var("RPGLIB_ALLOW_PRIMARY_DB")
        .map(|v| v == "1")
        .unwrap_or(false);
    check_path_allowed(&path, allow)?;
    Ok(path)
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
