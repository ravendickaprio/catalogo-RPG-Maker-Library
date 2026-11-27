//! Utilidades compartidas por las pruebas de integración.
#![allow(dead_code)]
use rusqlite::Connection;
use std::path::PathBuf;
use tempfile::TempDir;

const SCHEMA: &str = include_str!("../fixtures/schema.sql");

/// Crea `dir/name` con el DDL real (fixtures/schema.sql) y ejecuta `seed` (SQL).
pub fn make_db_with(dir: &TempDir, name: &str, seed: &str) -> PathBuf {
    let path = dir.path().join(name);
    let c = Connection::open(&path).unwrap();
    c.execute_batch(SCHEMA).unwrap();
    c.execute_batch(seed).unwrap();
    path
}
