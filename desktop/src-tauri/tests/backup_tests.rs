//! Fase 9: listar/verificar/restaurar backups, exportación y primer inicio sin base.
mod common;
use common::make_db_with;
use rpg_library_lib::backup;
use rpg_library_lib::commands::{AppState, Ready};
use rpg_library_lib::db;
use rpg_library_lib::error::AppError;
use rpg_library_lib::settings::Settings;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const SEED: &str = "INSERT INTO juegos(titulo, codigo, estado, tipo, descripcion, imagen) VALUES
  ('Alpha, \"Quest\"', 'RY-1', 'Sin empezar', 'RPG', 'linea1
linea2, con \"comillas\"', X'52494646'),
  ('Beta', 'RY-2', 'Terminado', 'RPG', '', NULL),
  ('Gamma', 'RY-3', 'Empezado', 'RPG', 'x', X'');
INSERT INTO tags(nombre) VALUES ('b'), ('a');
INSERT INTO juego_tags(juego_id, tag_id) VALUES (1,1),(1,2),(2,2);";

fn state_rw(path: &Path) -> AppState {
    AppState::with_ready(Ready::open(path, false).unwrap())
}

fn titles(path: &Path) -> Vec<String> {
    let c = Connection::open(path).unwrap();
    let mut st = c.prepare("SELECT titulo FROM juegos ORDER BY id").unwrap();
    let v = st
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    v
}

/// DB with 3 games and one backup of that content; then the live DB gets a 4th game.
fn with_backup() -> (TempDir, PathBuf, String) {
    let d = TempDir::new().unwrap();
    let p = make_db_with(&d, "t.db", SEED);
    let st = state_rw(&p);
    let b = st.ready().unwrap().manual_backup().unwrap();
    let name = b.file_name().unwrap().to_string_lossy().into_owned();
    drop(st);
    Connection::open(&p)
        .unwrap()
        .execute("INSERT INTO juegos(titulo) VALUES ('Delta')", [])
        .unwrap();
    (d, p, name)
}

#[test]
fn list_verify_restore_roundtrip() {
    let (_d, p, name) = with_backup();
    let list = backup::list_backups(&p).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].file_name, name);
    assert!(!list[0].pre_restore);
    assert!(list[0].modified.ends_with('Z'));

    let chk = backup::verify_backup(&p, &name).unwrap();
    assert!(chk.ok && chk.schema_ok);
    assert_eq!(chk.integrity, "ok");
    assert_eq!(chk.games, 3);
    assert_eq!(chk.foreign_keys, 0);
    assert_eq!(chk.fts_ok, Some(true));

    let st = state_rw(&p);
    assert_eq!(titles(&p).len(), 4);
    let r = st.restore_backup(&name).unwrap();
    assert_eq!(r.restored_from, name);
    assert_eq!(r.games, 3);
    assert!(r.pre_restore_backup.ends_with("-prerestore"));
    assert_eq!(titles(&p), vec!["Alpha, \"Quest\"", "Beta", "Gamma"]);

    // pre-restore kept, with the 4-game content, and flagged in the list
    let list = backup::list_backups(&p).unwrap();
    assert_eq!(list.len(), 2);
    let pre = list.iter().find(|e| e.pre_restore).unwrap();
    assert_eq!(pre.file_name, r.pre_restore_backup);
    let pre_chk = backup::verify_backup(&p, &pre.file_name).unwrap();
    assert_eq!(pre_chk.games, 4);

    // FTS still works on the swapped-in state, and the DB check is clean
    let ready = st.ready().unwrap();
    assert!(ready.fts_available);
    let conn = ready.lock_conn().unwrap();
    let page = rpg_library_lib::repo::search(
        &conn,
        &serde_json::from_value(
            serde_json::json!({ "query": "Alpha", "sort": "title_asc", "offset": 0, "limit": 10 }),
        )
        .unwrap(),
        true,
    )
    .unwrap();
    assert_eq!(page.total, 1);
    let c = backup::check_conn(&conn).unwrap();
    assert!(c.ok);
    assert_eq!(c.games, 3);
    assert_eq!(c.fts_ok, Some(true));
}

#[test]
fn restore_read_only_is_refused() {
    let (_d, p, name) = with_backup();
    let st = AppState::with_ready(Ready::open(&p, true).unwrap());
    let e = st.restore_backup(&name).unwrap_err();
    assert_eq!(e.kind(), "ReadOnly");
    assert_eq!(titles(&p).len(), 4);
    assert_eq!(backup::list_backups(&p).unwrap().len(), 1);
    // check_database-style verification still works read-only
    let ready = st.ready().unwrap();
    assert!(backup::check_conn(&ready.lock_conn().unwrap()).unwrap().ok);
}

#[test]
fn bad_names_are_rejected() {
    let (d, p, name) = with_backup();
    std::fs::write(d.path().join("secret.db"), "x").unwrap();
    let abs = d.path().join("secret.db").to_string_lossy().into_owned();
    let st = state_rw(&p);
    for bad in [
        "../t.db", "a/b", "a\\b", "..", ".", "", &abs, "C:evil", "x\0y",
    ] {
        for e in [
            backup::verify_backup(&p, bad).unwrap_err(),
            st.restore_backup(bad).unwrap_err(),
        ] {
            assert_eq!(e.kind(), "InvalidInput", "name {bad:?}");
        }
    }
    let e = backup::verify_backup(&p, "no-existe").unwrap_err();
    assert_eq!(e.kind(), "NotFound");
    assert!(backup::verify_backup(&p, &name).unwrap().ok);
    assert_eq!(titles(&p).len(), 4);
}

#[test]
fn corrupt_backup_fails_verify_and_restore() {
    let (_d, p, _name) = with_backup();
    let bdir = backup::backups_dir(&p);
    let bad = "t.db.bak-29990101-000000";
    std::fs::write(bdir.join(bad), vec![b'z'; 8192]).unwrap();
    let chk = backup::verify_backup(&p, bad).unwrap();
    assert!(!chk.ok && !chk.schema_ok);
    // an empty-schema valid sqlite file is not a catalog either
    let other = "t.db.bak-29990101-000001";
    Connection::open(bdir.join(other))
        .unwrap()
        .execute_batch("CREATE TABLE x(a)")
        .unwrap();
    assert!(!backup::verify_backup(&p, other).unwrap().ok);
    let st = state_rw(&p);
    for n in [bad, other] {
        let e = st.restore_backup(n).unwrap_err();
        assert_eq!(e.kind(), "InvalidInput");
    }
    assert_eq!(titles(&p).len(), 4);
    assert!(!backup::list_backups(&p)
        .unwrap()
        .iter()
        .any(|e| e.pre_restore));
}

#[test]
fn restore_is_busy_when_another_connection_holds_the_lock() {
    let (_d, p, name) = with_backup();
    let st = state_rw(&p);
    let other = Connection::open(&p).unwrap();
    other.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let e = st.restore_backup(&name).unwrap_err();
    assert_eq!(e.kind(), "Busy", "{e}");
    other.execute_batch("ROLLBACK").unwrap();
    assert_eq!(titles(&p).len(), 4);
    // and works once released
    assert_eq!(st.restore_backup(&name).unwrap().games, 3);
}

#[test]
fn prerestore_survives_rotation() {
    let (_d, p, name) = with_backup();
    let bdir = backup::backups_dir(&p);
    // 25 fake backups far in the future would sort after the new ones; use old ones instead
    for i in 0..30 {
        std::fs::write(bdir.join(format!("t.db.bak-19990101-0000{i:02}")), "x").unwrap();
    }
    let st = state_rw(&p);
    let r = st.restore_backup(&name).unwrap();
    assert!(bdir.join(&r.pre_restore_backup).exists());
    let n = backup::list_backups(&p).unwrap().len();
    assert!(n <= backup::KEEP + 1, "{n}");
    // protection holds even if the new file would sort among the oldest
    backup::rotate_protect(&bdir, "t.db.bak-", 1, Some(&r.pre_restore_backup));
    assert!(bdir.join(&r.pre_restore_backup).exists());
}

fn export_to(p: &Path, out: &Path, fmt: &str) -> Result<backup::ExportResult, AppError> {
    let ready = Ready::open(p, true).unwrap();
    let conn = ready.lock_conn().unwrap();
    backup::export_library(&conn, p, &out.to_string_lossy(), fmt)
}

#[test]
fn export_csv_and_json() {
    let d = TempDir::new().unwrap();
    let p = make_db_with(&d, "t.db", SEED);
    let out_dir = TempDir::new().unwrap();

    let csv = out_dir.path().join("lib.CSV");
    let r = export_to(&p, &csv, "csv").unwrap();
    assert_eq!(r.rows, 3);
    let bytes = std::fs::read(&csv).unwrap();
    assert_eq!(&bytes[..3], b"\xEF\xBB\xBF");
    let text = String::from_utf8(bytes[3..].to_vec()).unwrap();
    assert!(text.starts_with(
        "id,titulo,codigo,estado,idioma,tipo,pagina,puntuacion,ruta,descripcion,tags,tiene_portada\r\n"
    ));
    assert!(text.contains("1,\"Alpha, \"\"Quest\"\"\",RY-1,"));
    assert!(text.contains("\"linea1\nlinea2, con \"\"comillas\"\"\""));
    assert!(text.contains("\"a, b\",1\r\n"));
    assert!(text.contains(",,0\r\n") || text.contains(",0\r\n"));
    assert!(!text.contains("RIFF"));
    // no temp leftovers
    assert_eq!(std::fs::read_dir(out_dir.path()).unwrap().count(), 1);

    let js = out_dir.path().join("lib.json");
    assert_eq!(export_to(&p, &js, "json").unwrap().rows, 3);
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&js).unwrap()).unwrap();
    let a = v.as_array().unwrap();
    assert_eq!(a.len(), 3);
    assert_eq!(a[0]["id"], 1);
    assert_eq!(a[0]["titulo"], "Alpha, \"Quest\"");
    assert_eq!(a[0]["tags"], "a, b");
    assert_eq!(a[0]["tiene_portada"], 1);
    assert_eq!(a[1]["tiene_portada"], 0);
    assert_eq!(a[2]["tiene_portada"], 0, "empty blob is not a cover");
    assert_eq!(a[0].as_object().unwrap().len(), 12);
    assert_eq!(a[0]["descripcion"], "linea1\nlinea2, con \"comillas\"");
    // key order is the column order
    let raw = std::fs::read_to_string(&js).unwrap();
    assert!(raw.find("\"id\"").unwrap() < raw.find("\"titulo\"").unwrap());

    // empty library -> valid empty outputs
    let e = make_db_with(&d, "e.db", "");
    let ej = out_dir.path().join("e.json");
    assert_eq!(export_to(&e, &ej, "json").unwrap().rows, 0);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(&ej).unwrap())
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn export_refuses_bad_paths() {
    let d = TempDir::new().unwrap();
    let p = make_db_with(&d, "t.db", SEED);
    let before = std::fs::read(&p).unwrap();
    let out = TempDir::new().unwrap();
    let st = state_rw(&p);
    st.ready().unwrap().manual_backup().unwrap();
    let bdir = backup::backups_dir(&p);
    let cases: Vec<(PathBuf, &str)> = vec![
        (PathBuf::from("rel.csv"), "csv"),
        (out.path().join("x.json"), "csv"),
        (out.path().join("x"), "csv"),
        (out.path().join("no/existe/x.csv"), "csv"),
        (p.clone(), "csv"),
        (d.path().join("t.db"), "json"),
        (bdir.join("x.csv"), "csv"),
        (bdir.join("..").join("backups").join("y.json"), "json"),
        (out.path().join("ok.csv"), "xml"),
    ];
    for (path, fmt) in cases {
        let e = export_to(&p, &path, fmt).unwrap_err();
        assert_eq!(e.kind(), "InvalidInput", "{path:?}");
    }
    assert_eq!(std::fs::read(&p).unwrap(), before, "DB untouched");
    assert!(!bdir.join("x.csv").exists());
}

#[test]
fn first_run_without_db_is_config_and_set_db_path_recovers() {
    let e = db::resolve_app_db_path_with(&Settings::default(), None, false).unwrap_err();
    assert_eq!(e.kind(), "Config");
    assert!(e.to_string().contains("No hay ninguna base de datos"));

    let st = AppState::with_error(e);
    assert_eq!(st.ready().err().unwrap().kind(), "Config");
    let d = TempDir::new().unwrap();
    let p = make_db_with(&d, "t.db", SEED);
    let r = st.set_db_path(&p.to_string_lossy()).unwrap();
    assert_eq!(r.db_file_name, "t.db");
    assert!(st.ready().is_ok());

    // a configured path wins over "no fallback"
    let s = Settings {
        db_path: Some(p.to_string_lossy().into_owned()),
        ..Default::default()
    };
    assert!(db::resolve_app_db_path_with(&s, None, false).is_ok());
}

/// Interop finding (Fase 9): a DB exclusively locked by another app (Python `BEGIN EXCLUSIVE`)
/// must open as `Busy`, not as "No es una base válida".
#[test]
fn open_while_exclusively_locked_is_busy_not_invalid() {
    let dir = TempDir::new().unwrap();
    let path = make_db_with(&dir, "locked.db", "");
    let holder = Connection::open(&path).unwrap();
    holder.execute_batch("BEGIN EXCLUSIVE").unwrap();
    holder
        .execute("INSERT INTO tags(nombre) VALUES ('lock-test')", [])
        .unwrap();
    match Ready::open(&path, false) {
        Err(AppError::Busy(_)) => {}
        Err(other) => panic!("expected Busy, got {other:?}"),
        Ok(_) => panic!("expected Busy, opened instead"),
    }
    holder.execute_batch("ROLLBACK").unwrap();
    assert!(Ready::open(&path, false).is_ok());
}

#[test]
fn csv_neutralizes_formula_cells_but_not_numbers_or_json() {
    let d = TempDir::new().unwrap();
    let seed = "INSERT INTO juegos(titulo, codigo, estado, tipo, descripcion, puntuacion) VALUES
      ('=HYPERLINK(\"x\")', '+1', '-2', '@x', X'09' || 'tab', -3);";
    let p = make_db_with(&d, "t.db", seed);
    let out = TempDir::new().unwrap();
    let csv = out.path().join("f.csv");
    export_to(&p, &csv, "csv").unwrap();
    let text = std::fs::read_to_string(&csv).unwrap();
    let row = text.lines().nth(1).unwrap();
    assert!(
        row.starts_with("1,\"'=HYPERLINK(\"\"x\"\")\",'+1,'-2,"),
        "{row}"
    );
    assert!(row.contains(",'@x,"), "{row}");
    assert!(row.contains(",-3,"), "negative numbers stay numeric: {row}");
    let js = out.path().join("f.json");
    export_to(&p, &js, "json").unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&js).unwrap()).unwrap();
    assert_eq!(v[0]["titulo"], "=HYPERLINK(\"x\")");
    assert_eq!(v[0]["puntuacion"].as_f64(), Some(-3.0));
}

#[test]
fn export_refuses_symlink_into_backups_or_db() {
    let d = TempDir::new().unwrap();
    let p = make_db_with(&d, "t.db", SEED);
    let st = state_rw(&p);
    st.ready().unwrap().manual_backup().unwrap();
    let bdir = backup::backups_dir(&p);
    let out = TempDir::new().unwrap();
    let mk = |target: &Path, link: &Path| -> bool {
        #[cfg(windows)]
        let r = std::os::windows::fs::symlink_file(target, link);
        #[cfg(unix)]
        let r = std::os::unix::fs::symlink(target, link);
        r.is_ok()
    };
    let victim = bdir.join("victim.csv");
    std::fs::write(&victim, "keep").unwrap();
    let link_b = out.path().join("lb.csv");
    if !mk(&victim, &link_b) {
        eprintln!("symlink creation not permitted; skipping");
        return;
    }
    let before = std::fs::read(&p).unwrap();
    let e = export_to(&p, &link_b, "csv").unwrap_err();
    assert_eq!(e.kind(), "InvalidInput");
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep");
    let link_db = out.path().join("ld.db");
    assert!(mk(&p, &link_db));
    let e = export_to(&p, &link_db, "db").unwrap_err();
    assert_eq!(e.kind(), "InvalidInput");
    assert_eq!(std::fs::read(&p).unwrap(), before);
}

#[test]
fn prerestore_rotates_separately_over_many_cycles() {
    let (_d, p, name) = with_backup();
    let bdir = backup::backups_dir(&p);
    let st = state_rw(&p);
    for _ in 0..8 {
        st.restore_backup(&name).unwrap();
        // make the live db differ so every restore has something to protect
        Connection::open(&p)
            .unwrap()
            .execute("INSERT INTO juegos(titulo) VALUES ('Z')", [])
            .unwrap();
    }
    let all = backup::list_backups(&p).unwrap();
    let pre = all.iter().filter(|e| e.pre_restore).count();
    assert_eq!(pre, backup::KEEP_PRE_RESTORE);
    assert!(bdir.join(&name).exists(), "regular backup untouched");
    // regular rotation (20) must not evict pre-restore files
    for i in 0..30 {
        std::fs::write(bdir.join(format!("t.db.bak-20300101-0000{i:02}")), "x").unwrap();
    }
    st.ready().unwrap().manual_backup().unwrap();
    let all = backup::list_backups(&p).unwrap();
    assert_eq!(
        all.iter().filter(|e| e.pre_restore).count(),
        backup::KEEP_PRE_RESTORE
    );
    // the freshly created backup is protected even if it sorts among the oldest
    let regular = all.iter().filter(|e| !e.pre_restore).count();
    assert!(
        regular == backup::KEEP || regular == backup::KEEP + 1,
        "{regular}"
    );
}

#[test]
fn restored_state_is_used_by_next_command() {
    let (_d, p, name) = with_backup();
    let st = state_rw(&p);
    let r = st.restore_backup(&name).unwrap();
    assert!(r.warning.is_none());
    let ready = st.ready().unwrap();
    let n: i64 = ready
        .lock_conn()
        .unwrap()
        .query_row("SELECT count(*) FROM juegos", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 3);
    assert!(ready
        .session_info()
        .last_backup
        .unwrap()
        .ends_with("-prerestore"));
}
