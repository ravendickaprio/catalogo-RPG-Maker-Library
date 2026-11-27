//! Read-only benchmark: `bench [--plans] [--index-experiment] [db_path ...]`.
//!
//! * default: one markdown table per DB (p50/p95/max over 50 runs after 5 warmups).
//! * `--plans`: prints EXPLAIN QUERY PLAN for the main query shapes (per DB).
//! * `--index-experiment [--indexes=frag,frag]`: for each DB, copies it to a temp file, measures without
//!   indexes, creates candidate indexes + ANALYZE on the COPY, measures again, and
//!   prints a side-by-side table. The input DB is only read.
use rpg_library_lib::models::*;
use rpg_library_lib::{db, repo};
use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};
use std::path::{Path, PathBuf};
use std::time::Instant;

const ITER: usize = 50;
const WARMUP: usize = 5;

#[derive(Clone, Copy)]
struct Stats {
    p50: f64,
    p95: f64,
    max: f64,
}

fn measure<F: FnMut()>(mut f: F) -> Stats {
    for _ in 0..WARMUP {
        f();
    }
    let mut v: Vec<f64> = (0..ITER)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Stats {
        p50: v[v.len() / 2],
        p95: v[((v.len() as f64 * 0.95).ceil() as usize - 1).min(v.len() - 1)],
        max: v[v.len() - 1],
    }
}

fn req(q: &str, sort: SortKey) -> SearchRequest {
    SearchRequest {
        query: q.into(),
        estado: None,
        idioma: None,
        tag: None,
        sort,
        offset: 0,
        limit: 100,
    }
}

fn fail(msg: impl std::fmt::Display) -> ! {
    eprintln!("{msg}");
    std::process::exit(1)
}

/// Search scenarios (name, request). Shared by the table, plans and index experiment.
fn search_scenarios(conn: &Connection, total: i64) -> Vec<(String, SearchRequest)> {
    use SortKey::*;
    let mut v: Vec<(String, SearchRequest)> = Vec::new();
    let mut add = |n: &str, r: SearchRequest| v.push((n.to_string(), r));
    add("search empty, sort title, limit 100", req("", TitleAsc));
    add("search empty, sort code", req("", Code));
    add("search empty, sort status", req("", Status));
    let mut deep = req("", TitleAsc);
    deep.offset = (total - 100).clamp(0, 5000);
    add(&format!("deep page (offset {})", deep.offset), deep);
    add("search \"the\" (fts)", req("the", TitleAsc));
    add("search \"ry-1\" (fts code)", req("ry-1", TitleAsc));
    add("search \"ryu\" (fts, 3 chars)", req("ryu", TitleAsc));
    add("search \"the dark\" (fts, long)", req("the dark", TitleAsc));
    add("search \"a\" (like, 1 char)", req("a", TitleAsc));
    add("search \"ry\" (like, 2 chars)", req("ry", TitleAsc));
    add("search \"zzzzqqq\" (no match)", req("zzzzqqq", TitleAsc));
    let mut r = req("", TitleAsc);
    r.estado = Some("Sin empezar".into());
    r.idioma = Some("Japonés".into());
    add("estado='Sin empezar' + idioma='Japonés'", r);
    let mut r = req("the", TitleAsc);
    r.estado = Some("Sin empezar".into());
    r.idioma = Some("Japonés".into());
    add("\"the\" + estado + idioma", r);
    // most used tag, if any
    let tag: Option<String> = conn
        .query_row(
            "SELECT t.nombre FROM tags t JOIN juego_tags jt ON jt.tag_id = t.id \
             GROUP BY t.id ORDER BY count(*) DESC, t.nombre LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();
    if let Some(t) = tag {
        let mut r = req("", TitleAsc);
        r.tag = Some(t);
        add("tag filter (most used tag)", r);
    }
    v
}

fn run_all(path: &Path) -> (Vec<(String, Stats)>, i64) {
    let conn = db::open_read_only(path).unwrap_or_else(|e| fail(e));
    let (fts, _) = repo::detect_fts(&conn).unwrap_or_else(|e| fail(e));
    let total = repo::library_info(&conn, "bench", true).unwrap().total;
    let mut out: Vec<(String, Stats)> = Vec::new();
    out.push((
        "library_info".into(),
        measure(|| {
            repo::library_info(&conn, "bench", true).unwrap();
        }),
    ));
    for (name, r) in search_scenarios(&conn, total) {
        out.push((
            name,
            measure(|| {
                repo::search(&conn, &r, fts).unwrap();
            }),
        ));
    }
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
    if !ids.is_empty() {
        out.push((
            "get_game (random ids)".into(),
            measure(|| {
                repo::get_game(&conn, ids[next() % ids.len()]).unwrap();
            }),
        ));
    }
    if !cover_ids.is_empty() {
        out.push((
            "get_cover (ids with cover)".into(),
            measure(|| {
                repo::get_cover(&conn, cover_ids[next() % cover_ids.len()]).unwrap();
            }),
        ));
    }
    out.push((
        "suggest".into(),
        measure(|| {
            repo::suggest(&conn, &SuggestRequest::default()).unwrap();
        }),
    ));
    (out, total)
}

fn header(path: &Path, label: &str) {
    let conn = db::open_read_only(path).unwrap_or_else(|e| fail(e));
    let (fts, sync) = repo::detect_fts(&conn).unwrap();
    let info = repo::library_info(&conn, "bench", true).unwrap();
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    println!("### {label}\n");
    println!("- DB: {}", path.file_name().unwrap().to_string_lossy());
    println!("- SQLite: {}", info.sqlite_version);
    println!(
        "- Rows: {} (fts_available={fts}, in_sync={sync}), tags: {}",
        info.total,
        info.tags.len()
    );
    println!("- File size: {size} bytes");
    println!(
        "- OS: {} {}; profile: {}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        if cfg!(debug_assertions) {
            "debug (NOT representative)"
        } else {
            "release"
        }
    );
    println!("- Iterations: {ITER} (after {WARMUP} warmups), read-only, measures SQL path only (no IPC/render)\n");
}

fn print_table(rows: &[(String, Stats)]) {
    println!("| scenario | p50 ms | p95 ms | max ms |\n|---|---|---|---|");
    for (n, s) in rows {
        println!("| {n} | {:.3} | {:.3} | {:.3} |", s.p50, s.p95, s.max);
    }
    println!();
}

fn explain(conn: &Connection, title: &str, sql: &str, params: &[Value]) {
    println!("{title}");
    let mut st = conn
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap_or_else(|e| fail(e));
    let mut rows = st
        .query(params_from_iter(params.iter()))
        .unwrap_or_else(|e| fail(e));
    let mut depth: std::collections::HashMap<i64, usize> = Default::default();
    while let Some(r) = rows.next().unwrap() {
        let (id, parent, detail): (i64, i64, String) =
            (r.get(0).unwrap(), r.get(1).unwrap(), r.get(3).unwrap());
        let d = depth.get(&parent).map(|d| d + 1).unwrap_or(0);
        depth.insert(id, d);
        println!("    {}{}", "  ".repeat(d), detail);
    }
    println!();
}

fn plans(path: &Path) {
    let conn = db::open_read_only(path).unwrap_or_else(|e| fail(e));
    let (fts, _) = repo::detect_fts(&conn).unwrap();
    let total = repo::library_info(&conn, "bench", true).unwrap().total;
    println!("```text");
    let sc = search_scenarios(&conn, total);
    let pick = |prefix: &str| {
        sc.iter()
            .find(|(n, _)| n.starts_with(prefix))
            .map(|(_, r)| r)
    };
    for (label, prefix, count_too) in [
        ("empty query + title sort", "search empty, sort title", true),
        ("FTS query \"the\"", "search \"the\" (fts)", true),
        ("LIKE query \"ry\"", "search \"ry\"", true),
        ("estado + idioma filter", "estado=", true),
        ("tag filter", "tag filter", true),
        ("sort by code", "search empty, sort code", false),
    ] {
        let Some(r) = pick(prefix) else { continue };
        let q = repo::build_search_sql(&conn, r, fts).unwrap();
        let mut p = q.params.clone();
        p.push(Value::Integer(r.limit));
        p.push(Value::Integer(r.offset));
        explain(&conn, &format!("-- {label}: page query"), &q.page_sql, &p);
        if count_too {
            explain(
                &conn,
                &format!("-- {label}: count query"),
                &q.count_sql,
                &q.params,
            );
        }
    }
    explain(
        &conn,
        "-- suggest: eligible count",
        "SELECT count(*) FROM juegos WHERE lower(trim(coalesce(estado,''), ' ' || char(9) || char(10) || char(13))) <> 'terminado'",
        &[],
    );
    explain(
        &conn,
        "-- suggest: random pick",
        "SELECT id FROM juegos WHERE lower(trim(coalesce(estado,''), ' ' || char(9) || char(10) || char(13))) <> 'terminado' ORDER BY random() LIMIT 1",
        &[],
    );
    explain(
        &conn,
        "-- library_info: tags counts",
        "SELECT t.nombre, count(*) FROM tags t JOIN juego_tags jt ON jt.tag_id = t.id JOIN juegos j ON j.id = jt.juego_id GROUP BY t.id ORDER BY t.nombre COLLATE NOCASE",
        &[],
    );
    println!("```\n");
}

const CANDIDATE_INDEXES: &[&str] = &[
    "CREATE INDEX ix_juegos_estado ON juegos(estado)",
    "CREATE INDEX ix_juegos_idioma ON juegos(idioma)",
    "CREATE INDEX ix_juegos_titulo_nocase ON juegos(titulo COLLATE NOCASE)",
    "CREATE INDEX ix_juegos_codigo_nocase ON juegos(codigo COLLATE NOCASE)",
];

fn page_count(c: &Connection) -> i64 {
    c.query_row("PRAGMA page_count", [], |r| r.get(0)).unwrap()
}

fn index_experiment(path: &Path, only: &[String]) {
    let tmp = std::env::temp_dir().join(format!(
        "rpglib_idxexp_{}_{}.db",
        std::process::id(),
        path.file_stem().unwrap().to_string_lossy()
    ));
    let _ = std::fs::remove_file(&tmp);
    std::fs::copy(path, &tmp).unwrap_or_else(|e| fail(format!("copia: {e}")));
    let (before, _) = run_all(&tmp);
    let (pages_before, page_size, pages_after) = {
        let w = Connection::open(&tmp).unwrap();
        let pb = page_count(&w);
        let ps: i64 = w.query_row("PRAGMA page_size", [], |r| r.get(0)).unwrap();
        for ddl in CANDIDATE_INDEXES
            .iter()
            .filter(|d| only.is_empty() || only.iter().any(|o| d.contains(o.as_str())))
        {
            w.execute_batch(ddl).unwrap();
        }
        w.execute_batch("ANALYZE").unwrap();
        (pb, ps, page_count(&w))
    };
    let (after, _) = run_all(&tmp);
    let _ = std::fs::remove_file(&tmp);
    println!("| scenario | p50 no-index | p50 indexed | speedup |\n|---|---|---|---|");
    for ((n, b), (_, a)) in before.iter().zip(after.iter()) {
        println!(
            "| {n} | {:.3} | {:.3} | {:.2}x |",
            b.p50,
            a.p50,
            b.p50 / a.p50
        );
    }
    println!(
        "\nIndex size cost: {} -> {} pages (+{} pages x {} B = {:.1} KiB, includes sqlite_stat1)\n",
        pages_before,
        pages_after,
        pages_after - pages_before,
        page_size,
        ((pages_after - pages_before) * page_size) as f64 / 1024.0
    );
}

fn main() {
    let mut want_plans = false;
    let mut want_idx = false;
    let mut only: Vec<String> = Vec::new();
    let mut paths: Vec<PathBuf> = Vec::new();
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--plans" => want_plans = true,
            "--index-experiment" => want_idx = true,
            // restrict candidates by name fragment: --indexes=estado,idioma
            s if s.starts_with("--indexes=") => {
                only = s["--indexes=".len()..]
                    .split(',')
                    .map(String::from)
                    .collect()
            }
            s if s.starts_with("--") => fail(format!("flag desconocida: {s}")),
            s => paths.push(PathBuf::from(s)),
        }
    }
    if paths.is_empty() {
        paths.push(db::resolve_db_path().unwrap_or_else(|e| fail(e)));
    }
    let allow = std::env::var("RPGLIB_ALLOW_PRIMARY_DB").is_ok_and(|v| v == "1");
    for p in &paths {
        if let Err(e) = db::check_path_allowed(p, allow) {
            fail(e);
        }
    }
    for p in &paths {
        let label = p.file_name().unwrap().to_string_lossy().into_owned();
        header(p, &label);
        if want_plans {
            plans(p);
        } else if want_idx {
            index_experiment(p, &only);
        } else {
            let (rows, _) = run_all(p);
            print_table(&rows);
        }
    }
}
