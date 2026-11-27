//! Sonda para `tools/interop_test.py` (QA de coexistencia con las apps Python, ADR-018).
//! Imprime siempre UNA línea JSON en stdout. Rechaza `juegos.db` (solo copias).
//!
//!   interop_probe read     <db> <id> [consulta]   lectura (get_game, búsqueda, estado del índice)
//!   interop_probe create   <db> <json GameInput>  repo `write::create_game`
//!   interop_probe update   <db> <id> <json>       `write::update_game` (revisión actual)
//!   interop_probe favorite <db> <id> <true|false> `write::set_favorite`
//!   interop_probe delete   <db> <id>              `write::delete_game`
//!   interop_probe hold     <db> <segundos>        retiene BEGIN IMMEDIATE y hace ROLLBACK
//!   interop_probe busytry  <db> <id>              intenta set_favorite y mide hasta el error
use rpg_library_lib::models::{GameInput, SearchRequest, SortKey, WriteOptions};
use rpg_library_lib::{db, maintenance, repo, write};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Instant;

fn out(v: Value) -> ! {
    println!("{v}");
    std::process::exit(0)
}

fn err(e: rpg_library_lib::error::AppError) -> ! {
    out(json!({"ok": false, "error": {"kind": e.kind(), "message": e.to_string()}}))
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        out(
            json!({"ok": false, "error": {"kind": "Usage", "message": "uso: interop_probe <cmd> <db> ..."}}),
        );
    }
    let cmd = a[1].as_str();
    let path = Path::new(&a[2]);
    if let Err(e) = db::check_path_allowed(path, false) {
        err(e);
    }
    let read_only = cmd == "read";
    let conn = db::open_validated(path, read_only).unwrap_or_else(|e| err(e));
    let id = |i: usize| -> i64 { a.get(i).and_then(|s| s.parse().ok()).unwrap_or(0) };
    let rev = |conn: &rusqlite::Connection, id: i64| -> WriteOptions {
        WriteOptions {
            allow_duplicate_code: true,
            expected_revision: Some(
                repo::get_game(conn, id)
                    .map(|g| g.revision)
                    .unwrap_or_default(),
            ),
        }
    };
    let input = |i: usize| -> GameInput {
        serde_json::from_str(a.get(i).map(String::as_str).unwrap_or("{}")).unwrap_or_else(|e| {
            out(json!({"ok": false, "error": {"kind": "Usage", "message": e.to_string()}}))
        })
    };
    match cmd {
        "read" => {
            let game = repo::get_game(&conn, id(3))
                .map(|g| serde_json::to_value(g).unwrap())
                .unwrap_or(Value::Null);
            let (fts, in_sync) = repo::detect_fts(&conn).unwrap_or_else(|e| err(e));
            let mut hit = Value::Null;
            if let Some(q) = a.get(4) {
                let req = SearchRequest {
                    query: q.clone(),
                    estado: None,
                    idioma: None,
                    tag: None,
                    sort: SortKey::TitleAsc,
                    offset: 0,
                    limit: 50,
                };
                let p = repo::search(&conn, &req, fts).unwrap_or_else(|e| err(e));
                hit = json!({"ids": p.items.iter().map(|g| g.id).collect::<Vec<_>>(), "total": p.total, "strategy": p.strategy});
            }
            let status = maintenance::search_index_status(&conn).unwrap_or_else(|e| err(e));
            let integ: String = conn
                .query_row("PRAGMA integrity_check", [], |r| r.get(0))
                .unwrap_or_default();
            out(
                json!({"ok": true, "game": game, "search": hit, "fts": {"available": fts, "inSync": in_sync},
                "indexStatus": serde_json::to_value(status).unwrap(), "integrity": integ}),
            )
        }
        "create" => match write::create_game(
            &conn,
            &input(3),
            &WriteOptions {
                allow_duplicate_code: true,
                expected_revision: None,
            },
        ) {
            Ok(g) => out(json!({"ok": true, "game": serde_json::to_value(g).unwrap()})),
            Err(e) => err(e),
        },
        "update" => match write::update_game(&conn, id(3), &input(4), &rev(&conn, id(3))) {
            Ok(g) => out(json!({"ok": true, "game": serde_json::to_value(g).unwrap()})),
            Err(e) => err(e),
        },
        "favorite" => {
            let fav = a.get(4).is_some_and(|s| s == "true");
            match write::set_favorite(&conn, id(3), fav, &rev(&conn, id(3))) {
                Ok(g) => out(json!({"ok": true, "game": serde_json::to_value(g).unwrap()})),
                Err(e) => err(e),
            }
        }
        "delete" => match write::delete_game(&conn, id(3), &rev(&conn, id(3))) {
            Ok(()) => out(json!({"ok": true})),
            Err(e) => err(e),
        },
        "hold" => {
            // Retiene BEGIN IMMEDIATE `secs` segundos y hace ROLLBACK (no cambia datos).
            let secs: u64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(5);
            conn.execute_batch("BEGIN IMMEDIATE")
                .unwrap_or_else(|e| err(e.into()));
            println!("{}", json!({"held": true}));
            std::thread::sleep(std::time::Duration::from_secs(secs));
            let _ = conn.execute_batch("ROLLBACK");
            out(json!({"ok": true, "released": true}))
        }
        "busytry" => {
            let t = Instant::now();
            let opts = WriteOptions {
                allow_duplicate_code: true,
                expected_revision: Some(String::new()),
            };
            // La revisión se calcula con una lectura (no necesita bloqueo de escritura);
            // el intento de escritura es el que debe devolver Busy.
            let opts = WriteOptions {
                expected_revision: rev(&conn, id(3)).expected_revision,
                ..opts
            };
            let r = write::set_favorite(&conn, id(3), true, &opts);
            let ms = t.elapsed().as_millis() as u64;
            match r {
                Ok(_) => out(json!({"ok": true, "elapsedMs": ms})),
                Err(e) => out(
                    json!({"ok": false, "elapsedMs": ms, "error": {"kind": e.kind(), "message": e.to_string()}}),
                ),
            }
        }
        _ => {
            out(json!({"ok": false, "error": {"kind": "Usage", "message": "comando desconocido"}}))
        }
    }
}
