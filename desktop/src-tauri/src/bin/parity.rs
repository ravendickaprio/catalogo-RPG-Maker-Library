//! `parity <db> <cases.json>`: runs each case through `repo::search` (read-only), paging
//! with limit 200 and collecting every id in order. Prints a JSON array of
//! `{case, ids, strategy}` for tools/legacy_parity.py.
use rpg_library_lib::models::{SearchRequest, SortKey};
use rpg_library_lib::{db, repo};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

#[derive(Deserialize, Clone, serde::Serialize)]
struct Case {
    query: String,
    #[serde(default)]
    estado: Option<String>,
    #[serde(default)]
    idioma: Option<String>,
    #[serde(default)]
    tag: Option<String>,
    sort: SortKey,
}

fn fail(msg: impl std::fmt::Display) -> ! {
    eprintln!("{msg}");
    std::process::exit(1)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        fail("uso: parity <db> <cases.json>");
    }
    let path = PathBuf::from(&args[1]);
    if let Err(e) = db::check_path_allowed(&path, false) {
        fail(e);
    }
    let cases: Vec<Case> = serde_json::from_str(
        &std::fs::read_to_string(&args[2]).unwrap_or_else(|e| fail(format!("cases: {e}"))),
    )
    .unwrap_or_else(|e| fail(format!("cases.json inválido: {e}")));
    let conn = db::open_read_only(&path).unwrap_or_else(|e| fail(e));
    let (fts, _) = repo::detect_fts(&conn).unwrap_or_else(|e| fail(e));

    let mut out = Vec::new();
    for c in &cases {
        let mut ids: Vec<i64> = Vec::new();
        let mut strategy;
        let mut offset = 0;
        loop {
            let req = SearchRequest {
                query: c.query.clone(),
                estado: c.estado.clone(),
                idioma: c.idioma.clone(),
                tag: c.tag.clone(),
                sort: c.sort,
                offset,
                limit: 200,
            };
            let page = repo::search(&conn, &req, fts).unwrap_or_else(|e| fail(e));
            strategy = page.strategy;
            ids.extend(page.items.iter().map(|g| g.id));
            offset += 200;
            if page.items.len() < 200 || offset >= page.total {
                break;
            }
        }
        out.push(json!({ "case": c, "ids": ids, "strategy": strategy }));
    }
    println!("{}", serde_json::to_string(&out).unwrap());
}
