//! Pruebas de extremo a extremo del IPC con `tauri::test::MockRuntime`: verifican
//! nombres de comandos, forma de los argumentos (`{ req }` / `{ id }`) y JSON en
//! camelCase exactamente como los invoca el frontend (docs/CONTRACTS.md).

use rpg_library_lib::commands::{AppState, Ready};
use rpg_library_lib::{app_builder, db, error::AppError};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{WebviewWindow, WebviewWindowBuilder};
use tempfile::TempDir;

fn fixture_db(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("ipc.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(include_str!("fixtures/schema.sql"))
        .unwrap();
    let rows = [
        ("Alpha Quest", "RY-1000", "Sin empezar", "Japonés"),
        ("Beta Land", "RY-2000", "Terminado", "Inglés"),
        ("Gamma Saga", "RY-3000", "Empezado", "Inglés"),
    ];

    for (t, c, e, i) in rows {
        conn.execute(
            "INSERT INTO juegos(titulo, codigo, estado, idioma, descripcion, pagina, tipo, imagen)
             VALUES (?1, ?2, ?3, ?4, 'desc', 'ryuugames', 'RPG', CASE WHEN ?2='RY-1000' THEN X'52494646000000005745425056503820' END)",
            rusqlite::params![t, c, e, i],
        )
        .unwrap();
    }
    conn.execute_batch(
        "INSERT INTO tags(nombre) VALUES ('rpg'); INSERT INTO juego_tags(juego_id, tag_id) VALUES (1, 1);",
    )
    .unwrap();
    path
}

fn setup(path: &Path) -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let conn = db::open_read_only(path).unwrap();
    let state = AppState::with_ready(Ready::new(conn, true, "ipc.db".into()));
    let app = app_builder(mock_builder(), state)
        .build(mock_context(noop_assets()))
        .unwrap();
    let w = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    (app, w)
}

fn invoke(w: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
    tauri::test::get_ipc_response(
        w,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|b| b.deserialize::<Value>().unwrap())
}

#[test]
fn ipc_contract_end_to_end_and_read_only() {
    let dir = TempDir::new().unwrap();
    let path = fixture_db(&dir);
    let before = std::fs::read(&path).unwrap();
    let (_app, w) = setup(&path);

    let info = invoke(&w, "get_library_info", json!({})).unwrap();
    assert_eq!(info["total"], 3);
    assert_eq!(info["readOnly"], true);
    assert_eq!(info["ftsAvailable"], true);
    assert_eq!(info["dbFileName"], "ipc.db");
    assert!(info["tags"].is_array());
    assert_eq!(info["tags"][0]["value"], "rpg");
    assert_eq!(info["tags"][0]["count"], 1);

    let page = invoke(
        &w,
        "search_games",
        json!({ "req": { "query": "ry-2", "estado": null, "idioma": null, "sort": "title_asc", "offset": 0, "limit": 100 } }),
    )
    .unwrap();
    assert_eq!(page["total"], 1);
    assert_eq!(page["strategy"], "fts");
    assert_eq!(page["items"][0]["titulo"], "Beta Land");
    assert!(page["items"][0].get("hasCover").is_some());
    assert!(page["items"][0].get("imagen").is_none());

    // Fase 2: filtro por tag (ausente = sin filtro; presente = NOCASE).
    let tagged = invoke(
        &w,
        "search_games",
        json!({ "req": { "query": "", "tag": "RPG", "sort": "title_asc", "offset": 0, "limit": 10 } }),
    )
    .unwrap();
    assert_eq!(tagged["total"], 1);
    assert_eq!(tagged["items"][0]["titulo"], "Alpha Quest");
    let too_long = invoke(
        &w,
        "search_games",
        json!({ "req": { "query": "", "tag": "x".repeat(101), "sort": "title_asc", "offset": 0, "limit": 10 } }),
    )
    .unwrap_err();
    assert_eq!(too_long["kind"], "InvalidInput");

    let bad = invoke(
        &w,
        "search_games",
        json!({ "req": { "query": "", "sort": "title_asc", "offset": 0, "limit": 0 } }),
    )
    .unwrap_err();
    assert_eq!(bad["kind"], "InvalidInput");

    let id = page["items"][0]["id"].as_i64().unwrap();
    let detail = invoke(&w, "get_game", json!({ "id": id })).unwrap();
    assert_eq!(detail["codigo"], "RY-2000");
    assert!(detail["tags"].is_array());
    assert_eq!(
        invoke(&w, "get_game", json!({ "id": 9999 })).unwrap_err()["kind"],
        "NotFound"
    );

    let alpha = invoke(
        &w,
        "search_games",
        json!({ "req": { "query": "alpha", "sort": "recent", "offset": 0, "limit": 10 } }),
    )
    .unwrap();
    let alpha_id = alpha["items"][0]["id"].as_i64().unwrap();
    let cover = invoke(&w, "get_cover", json!({ "id": alpha_id })).unwrap();
    assert_eq!(cover["mime"], "image/webp");
    assert!(cover["dataBase64"].as_str().unwrap().len() > 4);

    // Sugerencia: 100 pulsaciones, nunca 'Terminado', excludeId respetado.
    let mut prev: Option<i64> = None;
    for _ in 0..100 {
        let s = invoke(&w, "suggest_game", json!({ "req": { "excludeId": prev } })).unwrap();
        assert_eq!(s["eligibleCount"], 2);
        assert_eq!(s["totalCount"], 3);
        let g = &s["game"];
        assert_ne!(g["estado"], "Terminado");
        let gid = g["id"].as_i64().unwrap();
        if let Some(p) = prev {
            assert_ne!(gid, p, "excludeId must avoid immediate repeat");
        }
        prev = Some(gid);
    }

    // Comandos no registrados deben ser rechazados.
    assert!(invoke(&w, "execute_sql", json!({ "sql": "DELETE FROM juegos" })).is_err());

    drop(_app);
    assert_eq!(
        before,
        std::fs::read(&path).unwrap(),
        "DB file must be untouched"
    );
}

#[test]
fn ipc_config_error_is_reported() {
    let state = AppState::with_error(AppError::Config("sin base".into()));
    let app = app_builder(mock_builder(), state)
        .build(mock_context(noop_assets()))
        .unwrap();
    let w = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let e = invoke(&w, "suggest_game", json!({ "req": {} })).unwrap_err();
    assert_eq!(e["kind"], "Config");
    assert_eq!(e["message"], "sin base");
}

#[test]
fn check_data_version_detects_external_writes() {
    let dir = TempDir::new().unwrap();
    let path = fixture_db(&dir);
    let (_app, w) = setup(&path);

    assert_eq!(
        invoke(&w, "get_library_info", json!({})).unwrap()["total"],
        3
    );
    let a = invoke(&w, "check_data_version", json!({})).unwrap();
    let b = invoke(&w, "check_data_version", json!({})).unwrap();
    assert_eq!(a["changed"], false);
    assert_eq!(b["changed"], false);
    assert_eq!(a["version"], b["version"]);

    // Escritura externa (otra conexión, como la app Python).
    let writer = Connection::open(&path).unwrap();
    writer
        .execute(
            "INSERT INTO juegos(titulo, codigo, estado, idioma) VALUES ('Delta', 'RY-4000', 'Sin empezar', 'Inglés')",
            [],
        )
        .unwrap();
    drop(writer);

    let c = invoke(&w, "check_data_version", json!({})).unwrap();
    assert_eq!(c["changed"], true);
    assert_ne!(c["version"], b["version"]);
    assert_eq!(
        invoke(&w, "get_library_info", json!({})).unwrap()["total"],
        4
    );
    let d = invoke(&w, "check_data_version", json!({})).unwrap();
    assert_eq!(d["changed"], false);
}

// ---------------------------------------------------------------- Fase 4 (escritura)

fn setup_rw(path: &Path) -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let ready = Ready::open(path, false).unwrap();
    let app = app_builder(mock_builder(), AppState::with_ready(ready))
        .build(mock_context(noop_assets()))
        .unwrap();
    let w = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    (app, w)
}

fn input(titulo: &str, codigo: &str) -> Value {
    json!({
        "titulo": titulo, "descripcion": "d", "estado": "Sin empezar", "idioma": "Español",
        "ruta": "", "codigo": codigo, "pagina": "ryuugames", "tipo": "", "puntuacion": 2.5,
        "tags": ["Nuevo", "nuevo", "x"]
    })
}

#[test]
fn ipc_write_commands_arg_shapes_and_flow() {
    let dir = TempDir::new().unwrap();
    let path = fixture_db(&dir);
    let (_app, w) = setup_rw(&path);

    let s = invoke(&w, "get_session_info", json!({})).unwrap();
    assert_eq!(s["readOnly"], false);
    assert_eq!(s["dbFileName"], "ipc.db");
    assert!(s["lastBackup"].is_null());
    assert!(s["dbDir"].as_str().unwrap().len() > 2);

    let v = invoke(&w, "get_vocabulary", json!({})).unwrap();
    assert_eq!(v["estados"][0], "Sin empezar");
    assert_eq!(v["puntuaciones"][1], 0.5);
    assert!(v["paginas"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p == "ryuugames"));

    // create: { input, opts }
    let created = invoke(
        &w,
        "create_game",
        json!({ "input": input("Nuevo Juego", "NJ-1"), "opts": {} }),
    )
    .unwrap();
    assert_eq!(created["titulo"], "Nuevo Juego");
    assert_eq!(created["tipo"], "Otro");
    assert_eq!(created["tags"], json!(["Nuevo", "x"]));
    let id = created["id"].as_i64().unwrap();
    let rev = created["revision"].as_str().unwrap().to_string();
    assert_eq!(rev.len(), 16);
    assert!(invoke(&w, "get_session_info", json!({})).unwrap()["lastBackup"].is_string());

    // info cache invalidated by the write
    assert_eq!(
        invoke(&w, "get_library_info", json!({})).unwrap()["total"],
        4
    );
    assert_eq!(
        invoke(&w, "get_library_info", json!({})).unwrap()["readOnly"],
        false
    );
    // own writes are not "external changes"
    assert_eq!(
        invoke(&w, "check_data_version", json!({})).unwrap()["changed"],
        false
    );

    // duplicate code -> DuplicateCode (mentions the other title); allowDuplicateCode -> ok
    let dup = invoke(
        &w,
        "create_game",
        json!({ "input": input("Otro", "nj_1"), "opts": { "allowDuplicateCode": false } }),
    )
    .unwrap_err();
    assert_eq!(dup["kind"], "DuplicateCode");
    assert!(dup["message"].as_str().unwrap().contains("Nuevo Juego"));
    let ok = invoke(
        &w,
        "create_game",
        json!({ "input": input("Otro", "nj_1"), "opts": { "allowDuplicateCode": true } }),
    )
    .unwrap();
    let id2 = ok["id"].as_i64().unwrap();

    // update: { id, input, opts }; missing revision -> InvalidInput; stale -> Conflict
    let e = invoke(
        &w,
        "update_game",
        json!({ "id": id, "input": input("Editado", "NJ-9"), "opts": {} }),
    )
    .unwrap_err();
    assert_eq!(e["kind"], "InvalidInput");
    let e = invoke(
        &w,
        "update_game",
        json!({ "id": id, "input": input("Editado", "NJ-9"), "opts": { "expectedRevision": "0000000000000000" } }),
    )
    .unwrap_err();
    assert_eq!(e["kind"], "Conflict");
    let upd = invoke(
        &w,
        "update_game",
        json!({ "id": id, "input": input("Editado", "NJ-9"), "opts": { "expectedRevision": rev } }),
    )
    .unwrap();
    assert_eq!(upd["titulo"], "Editado");
    assert_ne!(upd["revision"], rev.as_str());
    let found = invoke(
        &w,
        "search_games",
        json!({ "req": { "query": "Editado", "sort": "title_asc", "offset": 0, "limit": 10 } }),
    )
    .unwrap();
    assert_eq!(found["total"], 1);

    // delete: { id, opts }
    let new_rev = upd["revision"].as_str().unwrap();
    let e = invoke(&w, "delete_game", json!({ "id": id, "opts": {} })).unwrap_err();
    assert_eq!(e["kind"], "InvalidInput");
    let del = invoke(
        &w,
        "delete_game",
        json!({ "id": id, "opts": { "expectedRevision": new_rev } }),
    )
    .unwrap();
    assert!(del.is_null());
    assert_eq!(
        invoke(&w, "get_game", json!({ "id": id })).unwrap_err()["kind"],
        "NotFound"
    );
    assert_eq!(
        invoke(&w, "get_game", json!({ "id": id2 })).unwrap()["codigo"],
        "nj_1"
    );

    // validation error
    let mut bad = input("", "");
    bad["puntuacion"] = json!(9);
    assert_eq!(
        invoke(&w, "create_game", json!({ "input": bad, "opts": {} })).unwrap_err()["kind"],
        "InvalidInput"
    );

    // migrations
    let st = invoke(&w, "get_migration_status", json!({})).unwrap();
    assert_eq!(st["current"], 0);
    assert_eq!(st["latest"], 1);
    assert_eq!(st["pending"][0]["version"], 1);
    let run = invoke(&w, "run_migrations", json!({})).unwrap();
    assert_eq!(run["applied"], json!([1]));
    assert!(run["backupPath"].as_str().unwrap().contains(".bak-"));
    let st = invoke(&w, "get_migration_status", json!({})).unwrap();
    assert_eq!(st["current"], 1);
    assert_eq!(st["pending"], json!([]));
    let run = invoke(&w, "run_migrations", json!({})).unwrap();
    assert_eq!(run["applied"], json!([]));

    // manual backup
    let b = invoke(&w, "create_backup", json!({})).unwrap();
    assert!(Path::new(b["path"].as_str().unwrap()).is_file());

    // backups live next to the DB, inside backups/
    assert!(dir.path().join("backups").is_dir());
}

#[test]
fn ipc_read_only_mode_rejects_writes_and_set_read_only_toggles() {
    let dir = TempDir::new().unwrap();
    let path = fixture_db(&dir);
    let (_app, w) = setup_rw(&path);
    let s = invoke(&w, "set_read_only", json!({ "readOnly": true })).unwrap();
    assert_eq!(s["readOnly"], true);
    let before = std::fs::read(&path).unwrap();
    for (cmd, args) in [
        (
            "create_game",
            json!({ "input": input("X", ""), "opts": {} }),
        ),
        (
            "update_game",
            json!({ "id": 1, "input": input("X", ""), "opts": { "expectedRevision": "a" } }),
        ),
        (
            "delete_game",
            json!({ "id": 1, "opts": { "expectedRevision": "a" } }),
        ),
        ("run_migrations", json!({})),
    ] {
        assert_eq!(
            invoke(&w, cmd, args).unwrap_err()["kind"],
            "ReadOnly",
            "{cmd}"
        );
    }
    assert_eq!(before, std::fs::read(&path).unwrap());
    assert_eq!(
        invoke(&w, "get_library_info", json!({})).unwrap()["readOnly"],
        true
    );
    // read still works; back to read-write
    assert_eq!(
        invoke(&w, "get_migration_status", json!({})).unwrap()["current"],
        0
    );
    let s = invoke(&w, "set_read_only", json!({ "readOnly": false })).unwrap();
    assert_eq!(s["readOnly"], false);
}

#[test]
fn ipc_set_db_path_validates_and_swaps() {
    let dir = TempDir::new().unwrap();
    let path = fixture_db(&dir);
    let (_app, w) = setup_rw(&path);

    // second catalog with a different content
    let other = dir.path().join("other.db");
    {
        let c = Connection::open(&other).unwrap();
        c.execute_batch(include_str!("fixtures/schema.sql"))
            .unwrap();
        c.execute("INSERT INTO juegos(titulo) VALUES ('Solo uno')", [])
            .unwrap();
    }
    assert_eq!(
        invoke(&w, "set_db_path", json!({ "path": "relativa/x.db" })).unwrap_err()["kind"],
        "InvalidInput"
    );
    let missing = dir.path().join("nope.db");
    assert_eq!(
        invoke(
            &w,
            "set_db_path",
            json!({ "path": missing.to_string_lossy() })
        )
        .unwrap_err()["kind"],
        "InvalidInput"
    );
    let junk = dir.path().join("junk.db");
    std::fs::write(&junk, "esto no es sqlite").unwrap();
    let e = invoke(&w, "set_db_path", json!({ "path": junk.to_string_lossy() })).unwrap_err();
    assert_eq!(e["kind"], "Config");
    assert_eq!(std::fs::read_to_string(&junk).unwrap(), "esto no es sqlite");
    // unchanged after failures
    assert_eq!(
        invoke(&w, "get_library_info", json!({})).unwrap()["total"],
        3
    );

    let info = invoke(
        &w,
        "set_db_path",
        json!({ "path": other.to_string_lossy() }),
    )
    .unwrap();
    assert_eq!(info["total"], 1);
    assert_eq!(info["dbFileName"], "other.db");
    assert_eq!(
        invoke(&w, "get_session_info", json!({})).unwrap()["dbFileName"],
        "other.db"
    );
}
