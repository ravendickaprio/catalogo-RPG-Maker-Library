//! Pruebas de extremo a extremo del IPC con `tauri::test::MockRuntime`: verifican
//! nombres de comandos, forma de los argumentos (`{ req }` / `{ id }`) y JSON en
//! camelCase exactamente como los invoca el frontend (docs/CONTRACTS.md).

use rpg_library_lib::commands::{AppState, Ready};
use rpg_library_lib::{app_builder, db, error::AppError};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
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
    path
}

fn setup(path: &Path) -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let conn = db::open_read_only(path).unwrap();
    let state = AppState {
        inner: Ok(Ready {
            conn: Arc::new(Mutex::new(conn)),
            fts_available: true,
            db_file_name: "ipc.db".into(),
        }),
    };
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
    let state = AppState {
        inner: Err(AppError::Config("sin base".into())),
    };
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
