//! Migraciones versionadas (ADR-009). Versión en `app_metadata('desktop_schema_version')`.
//! `app_metadata` solo se crea dentro de una ejecución de migración (las DB legacy pueden no tenerla).
use crate::error::AppError;
use crate::models::{MigrationStatus, PendingMigration};
use crate::{repo, write};
use rusqlite::{Connection, OptionalExtension};

pub const VERSION_KEY: &str = "desktop_schema_version";

pub struct Migration {
    pub version: i64,
    pub description: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    description: "Índice ix_juegos_titulo_nocase (orden por título) y ANALYZE",
    sql: "CREATE INDEX IF NOT EXISTS ix_juegos_titulo_nocase ON juegos(titulo COLLATE NOCASE); ANALYZE;",
}];

pub fn latest() -> i64 {
    MIGRATIONS.last().map(|m| m.version).unwrap_or(0)
}

pub fn current_version(conn: &Connection) -> Result<i64, AppError> {
    if !repo::table_exists(conn, "app_metadata")? {
        return Ok(0);
    }
    let v: Option<String> = conn
        .query_row(
            "SELECT valor FROM app_metadata WHERE clave = ?",
            [VERSION_KEY],
            |r| r.get(0),
        )
        .optional()?;
    Ok(v.and_then(|s| s.trim().parse().ok()).unwrap_or(0))
}

pub fn status(conn: &Connection) -> Result<MigrationStatus, AppError> {
    let current = current_version(conn)?;
    Ok(MigrationStatus {
        current,
        latest: latest(),
        pending: MIGRATIONS
            .iter()
            .filter(|m| m.version > current)
            .map(|m| PendingMigration {
                version: m.version,
                description: m.description.into(),
            })
            .collect(),
    })
}

/// Applies the pending steps in one `BEGIN IMMEDIATE` transaction, stores the version, and
/// verifies integrity (SQLite + FTS) BEFORE committing so a failure rolls everything back.
/// The caller makes the backup first. Returns the applied versions.
pub fn run(conn: &Connection) -> Result<Vec<i64>, AppError> {
    let current = current_version(conn)?;
    let pending: Vec<&Migration> = MIGRATIONS.iter().filter(|m| m.version > current).collect();
    if pending.is_empty() {
        return Ok(vec![]);
    }
    write::in_tx(conn, || {
        let mut applied = vec![];
        for m in &pending {
            conn.execute_batch(m.sql).map_err(|e| {
                AppError::Database(format!("Migración v{} fallida: {e}", m.version))
            })?;
            applied.push(m.version);
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS app_metadata (clave TEXT PRIMARY KEY, valor TEXT NOT NULL)",
        )?;
        conn.execute(
            "INSERT INTO app_metadata(clave, valor) VALUES (?1, ?2) \
             ON CONFLICT(clave) DO UPDATE SET valor = excluded.valor",
            rusqlite::params![VERSION_KEY, applied.last().unwrap().to_string()],
        )?;
        write::check_integrity(conn)?;
        Ok(applied)
    })
}
