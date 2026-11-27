//! Read-only benchmark: `bench [db_path]`.
use rpg_library_lib::models::*;
use rpg_library_lib::{db, repo};
use std::path::PathBuf;
use std::time::Instant;

fn measure<F: FnMut()>(name: &str, mut f: F) {
    for _ in 0..5 {
        f();
    }
    let mut v: Vec<f64> = (0..50)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = v[v.len() / 2];
    let p95 = v[((v.len() as f64 * 0.95).ceil() as usize - 1).min(v.len() - 1)];
    println!("| {name} | {p50:.3} | {p95:.3} | {:.3} |", v[v.len() - 1]);
}

fn req(q: &str, estado: Option<&str>, idioma: Option<&str>, limit: i64) -> SearchRequest {
    SearchRequest {
        query: q.into(),
        estado: estado.map(String::from),
        idioma: idioma.map(String::from),
        sort: SortKey::TitleAsc,
        offset: 0,
        limit,
    }
}

fn main() {
    let path: PathBuf = match std::env::args().nth(1) {
        Some(p) => PathBuf::from(p),
        None => db::resolve_db_path().unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(1)
        }),
    };
    if let Err(e) = db::check_path_allowed(
        &path,
        std::env::var("RPGLIB_ALLOW_PRIMARY_DB").is_ok_and(|v| v == "1"),
    ) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    let conn = db::open_read_only(&path).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let (fts, sync) = repo::detect_fts(&conn).unwrap();
    let info = repo::library_info(&conn, "bench").unwrap();
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    println!("- DB: {}", path.file_name().unwrap().to_string_lossy());
    println!("- SQLite: {}", info.sqlite_version);
    println!(
        "- Rows: {} (fts_available={fts}, in_sync={sync})",
        info.total
    );
    println!("- File size: {size} bytes");
    println!("- OS: {} {}", std::env::consts::OS, std::env::consts::ARCH);
    println!("- Iterations: 50 (after 5 warmups), read-only\n");

    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM juegos")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    let cover_ids: Vec<i64> = conn
        .prepare("SELECT id FROM juegos WHERE coalesce(length(imagen),0) > 0")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    let mut seed: u64 = 0x9E3779B97F4A7C15;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed as usize
    };

    println!("| scenario | p50 ms | p95 ms | max ms |\n|---|---|---|---|");
    measure("library_info", || {
        repo::library_info(&conn, "bench").unwrap();
    });
    measure("search empty, sort title, limit 100", || {
        repo::search(&conn, &req("", None, None, 100), fts).unwrap();
    });
    measure("search \"the\" (fts)", || {
        repo::search(&conn, &req("the", None, None, 100), fts).unwrap();
    });
    measure("search \"ry-1\" (fts code)", || {
        repo::search(&conn, &req("ry-1", None, None, 100), fts).unwrap();
    });
    measure("search \"a\" (like, <3 chars)", || {
        repo::search(&conn, &req("a", None, None, 100), fts).unwrap();
    });
    measure("search \"zzzzqqq\" (no match)", || {
        repo::search(&conn, &req("zzzzqqq", None, None, 100), fts).unwrap();
    });
    measure("search estado='Sin empezar' + idioma='Japonés'", || {
        repo::search(
            &conn,
            &req("", Some("Sin empezar"), Some("Japonés"), 100),
            fts,
        )
        .unwrap();
    });
    if !ids.is_empty() {
        measure("get_game (random ids)", || {
            let id = ids[next() % ids.len()];
            repo::get_game(&conn, id).unwrap();
        });
    }
    if !cover_ids.is_empty() {
        measure("get_cover (ids with cover)", || {
            let id = cover_ids[next() % cover_ids.len()];
            repo::get_cover(&conn, id).unwrap();
        });
    }
    measure("suggest", || {
        repo::suggest(&conn, &SuggestRequest::default()).unwrap();
    });
}
