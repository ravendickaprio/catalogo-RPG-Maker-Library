//! Backups con la API de backup de SQLite (ADR-012). Solo escribe y borra dentro de `<dbDir>/backups/`.
use crate::error::AppError;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const KEEP: usize = 20;

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

/// Copies the live database with the SQLite backup API into `<dbDir>/backups/<name>.bak-<stamp>`
/// and rotates old ones. Returns the new file. On failure nothing is left behind.
pub fn create_backup(conn: &Connection, db_path: &Path) -> Result<PathBuf, AppError> {
    let file_name = db_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::Database("Backup fallido: ruta de base sin nombre.".into()))?;
    let dir = backups_dir(db_path);
    std::fs::create_dir_all(&dir).map_err(|e| db_err("no se pudo crear backups/", e))?;
    let prefix = format!("{file_name}.bak-");
    let stamp = now_stamp();
    let mut dest = dir.join(format!("{prefix}{stamp}"));
    let mut n = 1;
    while dest.exists() {
        dest = dir.join(format!("{prefix}{stamp}-{n}"));
        n += 1;
    }
    let result = (|| -> Result<(), AppError> {
        let mut dst =
            Connection::open(&dest).map_err(|e| db_err("no se pudo crear el archivo", e))?;
        {
            let b = rusqlite::backup::Backup::new(conn, &mut dst)
                .map_err(|e| db_err("no se pudo iniciar", e))?;
            b.run_to_completion(256, Duration::from_millis(5), None)
                .map_err(|e| db_err("copia interrumpida", e))?;
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
    rotate(&dir, &prefix, KEEP);
    Ok(dest)
}

/// Keeps the `keep` newest files in `dir` whose name starts with `prefix` (names sort by
/// timestamp). Never touches anything else.
pub fn rotate(dir: &Path, prefix: &str, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with(prefix))
        .collect();
    names.sort();
    if names.len() > keep {
        for n in &names[..names.len() - keep] {
            let _ = std::fs::remove_file(dir.join(n));
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
