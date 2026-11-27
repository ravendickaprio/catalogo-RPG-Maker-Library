use rpg_library_lib::db;
use rpg_library_lib::error::AppError;
use rpg_library_lib::models::*;
use rpg_library_lib::repo;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const SCHEMA: &str = include_str!("fixtures/schema.sql");

struct Row {
    titulo: &'static str,
    codigo: &'static str,
    estado: &'static str,
    idioma: &'static str,
    tipo: &'static str,
    desc: &'static str,
    img: Option<Vec<u8>>,
    tags: &'static [&'static str],
}

fn row(
    titulo: &'static str,
    codigo: &'static str,
    estado: &'static str,
    idioma: &'static str,
) -> Row {
    Row {
        titulo,
        codigo,
        estado,
        idioma,
        tipo: "Aventura",
        desc: "",
        img: None,
        tags: &[],
    }
}

fn webp() -> Vec<u8> {
    let mut v = b"RIFF".to_vec();
    v.extend_from_slice(&[4, 0, 0, 0]);
    v.extend_from_slice(b"WEBPVP8 ");
    v
}

fn standard_rows() -> Vec<Row> {
    vec![
        Row {
            desc: "Una aventura épica",
            img: Some(webp()),
            tags: &["rpg", "Fantasía", "accion"],
            ..row("Zeta Quest", "RY-1000", "Sin empezar", "Japonés")
        },
        Row {
            img: Some(b"\x89PNG\r\n\x1a\n".to_vec()),
            ..row("alpha Tale", "RY-1001", "Empezado", "Inglés")
        },
        Row {
            img: Some(vec![0xFF, 0xD8, 0xFF, 0xE0]),
            ..row("Beta Land", "", "Terminado", "Japonés")
        },
        Row {
            img: Some(b"GIF89a".to_vec()),
            ..row("Gamma Ray", "AB-5", "Sin empezar", "Inglés")
        },
        Row {
            img: Some(vec![1, 2, 3]),
            ..row("Delta The Hero", "ZZ-9", "Sin empezar", "Japonés")
        },
        Row {
            desc: "100% real_deal",
            ..row("Epsilon", "ab_77", "Raro", "Inglés")
        },
    ]
}

fn make_db(dir: &TempDir, name: &str, rows: &[Row], with_schema: bool) -> PathBuf {
    let path = dir.path().join(name);
    let conn = Connection::open(&path).unwrap();
    if with_schema {
        conn.execute_batch(SCHEMA).unwrap();
    } else {
        conn.execute_batch(
            "CREATE TABLE juegos (id INTEGER PRIMARY KEY AUTOINCREMENT, titulo TEXT NOT NULL, descripcion TEXT DEFAULT '', estado TEXT DEFAULT 'Sin empezar', idioma TEXT DEFAULT 'Español', imagen BLOB, ruta TEXT DEFAULT '', codigo TEXT DEFAULT '', pagina TEXT DEFAULT '', tipo TEXT DEFAULT 'Otro', puntuacion REAL DEFAULT 0);
             CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, nombre TEXT NOT NULL COLLATE NOCASE UNIQUE);
             CREATE TABLE juego_tags (juego_id INTEGER NOT NULL, tag_id INTEGER NOT NULL, PRIMARY KEY (juego_id, tag_id));",
        )
        .unwrap();
    }
    for r in rows {
        conn.execute(
            "INSERT INTO juegos (titulo, descripcion, estado, idioma, imagen, ruta, codigo, pagina, tipo, puntuacion) VALUES (?,?,?,?,?,?,?,?,?,?)",
            params![r.titulo, r.desc, r.estado, r.idioma, r.img, "C:/x", r.codigo, "", r.tipo, 3.5],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        for t in r.tags {
            conn.execute("INSERT OR IGNORE INTO tags(nombre) VALUES (?)", [t])
                .unwrap();
            conn.execute(
                "INSERT INTO juego_tags(juego_id, tag_id) SELECT ?, id FROM tags WHERE nombre = ?",
                params![id, t],
            )
            .unwrap();
        }
    }
    drop(conn);
    path
}

fn open_std() -> (TempDir, Connection) {
    let dir = TempDir::new().unwrap();
    let p = make_db(&dir, "t.db", &standard_rows(), true);
    let c = db::open_read_only(&p).unwrap();
    (dir, c)
}

fn req(q: &str) -> SearchRequest {
    SearchRequest {
        query: q.into(),
        estado: None,
        idioma: None,
        tag: None,
        sort: SortKey::TitleAsc,
        offset: 0,
        limit: 50,
    }
}

fn titles(p: &SearchPage) -> Vec<&str> {
    p.items.iter().map(|g| g.titulo.as_str()).collect()
}

#[test]
fn search_empty_returns_all() {
    let (_d, c) = open_std();
    let p = repo::search(&c, &req(""), true).unwrap();
    assert_eq!(p.strategy, "none");
    assert_eq!(p.total, 6);
    assert_eq!(p.items.len(), 6);
    assert!(p.items.iter().any(|g| g.has_cover));
    assert!(!p
        .items
        .iter()
        .find(|g| g.titulo == "Beta Land")
        .map(|g| !g.has_cover)
        .unwrap());
}

#[test]
fn search_fts_substring_case_unicode() {
    let (_d, c) = open_std();
    let p = repo::search(&c, &req("QUES"), true).unwrap();
    assert_eq!(p.strategy, "fts");
    assert_eq!(titles(&p), vec!["Zeta Quest"]);
    // unicode in description and idioma-free fields
    let p = repo::search(&c, &req("épica"), true).unwrap();
    assert_eq!(titles(&p), vec!["Zeta Quest"]);
    let p = repo::search(&c, &req("ÉPICA"), true).unwrap();
    assert_eq!(titles(&p), vec!["Zeta Quest"]);
    // tags are searchable
    let p = repo::search(&c, &req("fantas"), true).unwrap();
    assert_eq!(titles(&p), vec!["Zeta Quest"]);
}

#[test]
fn search_unicode_japones_in_title() {
    let dir = TempDir::new().unwrap();
    let rows = vec![
        row("Aventura en Japonés", "X-1", "Sin empezar", "Japonés"),
        row("Other", "", "Sin empezar", "Inglés"),
    ];
    let p = make_db(&dir, "u.db", &rows, true);
    let c = db::open_read_only(&p).unwrap();
    let r = repo::search(&c, &req("japonés"), true).unwrap();
    assert_eq!(titles(&r), vec!["Aventura en Japonés"]);
    let r = repo::search(&c, &req("JAPONÉS"), true).unwrap();
    assert_eq!(r.total, 1);
}

#[test]
fn search_code_variants() {
    let (_d, c) = open_std();
    for q in ["ry1000", "RY_1000", "ry-1000", "RY 1000", "RY-1000"] {
        let p = repo::search(&c, &req(q), true).unwrap();
        assert_eq!(p.strategy, "fts", "{q}");
        assert_eq!(titles(&p), vec!["Zeta Quest"], "{q}");
    }
    let p = repo::search(&c, &req("ab77"), true).unwrap();
    assert_eq!(titles(&p), vec!["Epsilon"]);
}

#[test]
fn search_short_query_uses_like() {
    let (_d, c) = open_std();
    let p = repo::search(&c, &req("ab"), true).unwrap();
    assert_eq!(p.strategy, "like");
    assert_eq!(titles(&p), vec!["Epsilon", "Gamma Ray"]);
    // wildcard chars are escaped
    let p = repo::search(&c, &req("%"), true).unwrap();
    assert_eq!(titles(&p), vec!["Epsilon"]);
    let p = repo::search(&c, &req("_"), true).unwrap();
    assert_eq!(titles(&p), vec!["Epsilon"]);
}

#[test]
fn search_filters() {
    let (_d, c) = open_std();
    let mut r = req("");
    r.estado = Some("Sin empezar".into());
    let p = repo::search(&c, &r, true).unwrap();
    assert_eq!(p.total, 3);
    r.idioma = Some("Japonés".into());
    let p = repo::search(&c, &r, true).unwrap();
    assert_eq!(p.total, 2);
    r.query = "quest".into();
    let p = repo::search(&c, &r, true).unwrap();
    assert_eq!(titles(&p), vec!["Zeta Quest"]);
    assert_eq!(p.strategy, "fts");
}

#[test]
fn search_pagination() {
    let (_d, c) = open_std();
    let mut r = req("");
    r.limit = 2;
    r.offset = 2;
    let p = repo::search(&c, &r, true).unwrap();
    assert_eq!(p.total, 6);
    assert_eq!(p.items.len(), 2);
    assert_eq!((p.offset, p.limit), (2, 2));
    let all = repo::search(&c, &req(""), true).unwrap();
    assert_eq!(
        titles(&p),
        all.items[2..4]
            .iter()
            .map(|g| g.titulo.as_str())
            .collect::<Vec<_>>()
    );
    r.offset = 100;
    let p = repo::search(&c, &r, true).unwrap();
    assert!(p.items.is_empty());
    assert_eq!(p.total, 6);
}

#[test]
fn search_sort_keys() {
    let (_d, c) = open_std();
    let s = |k: SortKey| {
        let mut r = req("");
        r.sort = k;
        let p = repo::search(&c, &r, true).unwrap();
        p.items.iter().map(|g| g.titulo.clone()).collect::<Vec<_>>()
    };
    assert_eq!(
        s(SortKey::TitleAsc),
        [
            "alpha Tale",
            "Beta Land",
            "Delta The Hero",
            "Epsilon",
            "Gamma Ray",
            "Zeta Quest"
        ]
    );
    assert_eq!(
        s(SortKey::TitleDesc),
        [
            "Zeta Quest",
            "Gamma Ray",
            "Epsilon",
            "Delta The Hero",
            "Beta Land",
            "alpha Tale"
        ]
    );
    // empty code last; then codigo nocase: ab_77, AB-5? '-'(45) < '_'(95) so AB-5 first
    assert_eq!(
        s(SortKey::Code),
        [
            "Gamma Ray",
            "Epsilon",
            "Zeta Quest",
            "alpha Tale",
            "Delta The Hero",
            "Beta Land"
        ]
        .map(String::from)
        .to_vec()
    );
    // Sin empezar, Empezado, Terminado, otros
    assert_eq!(
        s(SortKey::Status),
        [
            "Delta The Hero",
            "Gamma Ray",
            "Zeta Quest",
            "alpha Tale",
            "Beta Land",
            "Epsilon"
        ]
        .map(String::from)
        .to_vec()
    );
    assert_eq!(
        s(SortKey::Language),
        [
            "alpha Tale",
            "Epsilon",
            "Gamma Ray",
            "Beta Land",
            "Delta The Hero",
            "Zeta Quest"
        ]
        .map(String::from)
        .to_vec()
    );
    assert_eq!(
        s(SortKey::Recent),
        [
            "Epsilon",
            "Delta The Hero",
            "Gamma Ray",
            "Beta Land",
            "alpha Tale",
            "Zeta Quest"
        ]
        .map(String::from)
        .to_vec()
    );
}

#[test]
fn search_validation() {
    let (_d, c) = open_std();
    let bad = |f: &dyn Fn(&mut SearchRequest)| {
        let mut r = req("");
        f(&mut r);
        assert!(matches!(
            repo::search(&c, &r, true),
            Err(AppError::InvalidInput(_))
        ));
    };
    bad(&|r| r.limit = 0);
    bad(&|r| r.limit = 201);
    bad(&|r| r.offset = -1);
    bad(&|r| r.query = "a".repeat(201));
    let mut r = req("");
    r.query = "a".repeat(200);
    r.limit = 200;
    assert!(repo::search(&c, &r, true).is_ok());
}

#[test]
fn search_injection_and_quotes() {
    let (_d, c) = open_std();
    for q in [
        "\"'; DROP TABLE juegos; --",
        "\"",
        "a\"b\"c",
        "'",
        "\"\"\"",
        "foo OR bar",
        "titulo : x",
        "*",
        "NEAR(a b)",
    ] {
        let r = repo::search(&c, &req(q), true);
        assert!(r.is_ok(), "{q}: {r:?}");
        let r = repo::search(&c, &req(q), false);
        assert!(r.is_ok(), "{q} like: {r:?}");
    }
    let n: i64 = c
        .query_row("SELECT count(*) FROM juegos", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 6);
}

#[test]
fn fallback_without_fts_tables() {
    let dir = TempDir::new().unwrap();
    let p = make_db(&dir, "nofts.db", &standard_rows(), false);
    let c = db::open_read_only(&p).unwrap();
    assert_eq!(repo::detect_fts(&c).unwrap(), (false, false));
    let r = repo::search(&c, &req("quest"), false).unwrap();
    assert_eq!(r.strategy, "like");
    assert_eq!(titles(&r), vec!["Zeta Quest"]);
    let r = repo::search(&c, &req(""), false).unwrap();
    assert_eq!(r.total, 6);
    let info = repo::library_info(&c, "nofts.db").unwrap();
    assert!(!info.fts_available && !info.fts_in_sync);
    assert_eq!(info.total, 6);
    let s = repo::suggest(&c, &SuggestRequest::default()).unwrap();
    assert!(s.game.is_some());
}

#[test]
fn library_info_values() {
    let (_d, c) = open_std();
    assert_eq!(repo::detect_fts(&c).unwrap(), (true, true));
    let i = repo::library_info(&c, "t.db").unwrap();
    assert_eq!(i.db_file_name, "t.db");
    assert!(i.read_only);
    assert_eq!(i.total, 6);
    assert_eq!(i.estados[0].value, "Sin empezar");
    assert_eq!(i.estados[0].count, 3);
    assert!(i.sqlite_version.starts_with('3'));
    assert_eq!(i.idiomas.iter().map(|x| x.count).sum::<i64>(), 6);
}

#[test]
fn get_game_tags_sorted_and_errors() {
    let (_d, c) = open_std();
    let g = repo::get_game(&c, 1).unwrap();
    assert_eq!(g.summary.titulo, "Zeta Quest");
    assert_eq!(g.tags, vec!["accion", "Fantasía", "rpg"]);
    assert!(g.summary.has_cover);
    assert_eq!(g.descripcion, "Una aventura épica");
    assert_eq!(g.ruta, "C:/x");
    assert!(repo::get_game(&c, 2).unwrap().tags.is_empty());
    assert!(matches!(
        repo::get_game(&c, 9999),
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        repo::get_game(&c, 0),
        Err(AppError::InvalidInput(_))
    ));
    assert!(matches!(
        repo::get_game(&c, -3),
        Err(AppError::InvalidInput(_))
    ));
}

#[test]
fn get_cover_mime_and_none() {
    let (_d, c) = open_std();
    let m = |id| repo::get_cover(&c, id).unwrap().map(|x| x.mime);
    assert_eq!(m(1).as_deref(), Some("image/webp"));
    assert_eq!(m(2).as_deref(), Some("image/png"));
    assert_eq!(m(3).as_deref(), Some("image/jpeg"));
    assert_eq!(m(4).as_deref(), Some("image/gif"));
    assert_eq!(m(5).as_deref(), Some("application/octet-stream"));
    assert_eq!(m(6), None);
    let cv = repo::get_cover(&c, 1).unwrap().unwrap();
    assert!(!cv.data_base64.is_empty());
    assert!(matches!(
        repo::get_cover(&c, 999),
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        repo::get_cover(&c, 0),
        Err(AppError::InvalidInput(_))
    ));
}

#[test]
fn suggest_never_terminado() {
    let dir = TempDir::new().unwrap();
    let mut rows = standard_rows();
    rows.push(row("Hidden1", "", " terminado ", "Inglés"));
    rows.push(row("Hidden2", "", "TERMINADO", "Inglés"));
    rows.push(row("Hidden3", "", "  Terminado  ", "Inglés"));
    rows.push(row("Hidden4", "", "\tTerminado\r\n", "Inglés"));
    let p = make_db(&dir, "s.db", &rows, true);
    let c = db::open_read_only(&p).unwrap();
    let mut seen_empty_state_included = false;
    for _ in 0..200 {
        let s = repo::suggest(&c, &SuggestRequest::default()).unwrap();
        let g = s.game.unwrap();
        let e = g.summary.estado.trim().to_lowercase();
        assert_ne!(e, "terminado");
        assert_ne!(g.summary.titulo, "Beta Land");
        assert_eq!((s.eligible_count, s.total_count), (5, 10));
        if g.summary.estado == "Raro" {
            seen_empty_state_included = true;
        }
    }
    assert!(seen_empty_state_included, "unknown states must be eligible");
}

#[test]
fn suggest_none_cases() {
    let dir = TempDir::new().unwrap();
    let rows = vec![
        row("A", "", "Terminado", "x"),
        row("B", "", " terminado", "x"),
    ];
    let p = make_db(&dir, "all.db", &rows, true);
    let c = db::open_read_only(&p).unwrap();
    let s = repo::suggest(&c, &SuggestRequest::default()).unwrap();
    assert!(s.game.is_none());
    assert_eq!((s.eligible_count, s.total_count), (0, 2));
    let p = make_db(&dir, "empty.db", &[], true);
    let c = db::open_read_only(&p).unwrap();
    let s = repo::suggest(&c, &SuggestRequest::default()).unwrap();
    assert!(s.game.is_none());
    assert_eq!((s.eligible_count, s.total_count), (0, 0));
}

#[test]
fn suggest_exclude_id() {
    let dir = TempDir::new().unwrap();
    let rows = vec![
        row("A", "", "Sin empezar", "x"),
        row("B", "", "Empezado", "x"),
        row("C", "", "Terminado", "x"),
    ];
    let p = make_db(&dir, "ex.db", &rows, true);
    let c = db::open_read_only(&p).unwrap();
    for _ in 0..50 {
        let s = repo::suggest(
            &c,
            &SuggestRequest {
                exclude_id: Some(1),
            },
        )
        .unwrap();
        assert_eq!(s.game.unwrap().summary.id, 2);
    }
    // single candidate: exclusion ignored
    let rows = vec![
        row("A", "", "Sin empezar", "x"),
        row("C", "", "Terminado", "x"),
    ];
    let p = make_db(&dir, "one.db", &rows, true);
    let c = db::open_read_only(&p).unwrap();
    let s = repo::suggest(
        &c,
        &SuggestRequest {
            exclude_id: Some(1),
        },
    )
    .unwrap();
    assert_eq!(s.game.unwrap().summary.id, 1);
    assert_eq!(s.eligible_count, 1);
}

fn snapshot(p: &Path) -> (Vec<u8>, std::time::SystemTime) {
    (
        std::fs::read(p).unwrap(),
        std::fs::metadata(p).unwrap().modified().unwrap(),
    )
}

#[test]
fn read_only_guarantee() {
    let dir = TempDir::new().unwrap();
    let p = make_db(&dir, "ro.db", &standard_rows(), true);
    let before = snapshot(&p);
    let c = db::open_read_only(&p).unwrap();
    for i in 0..100 {
        repo::suggest(
            &c,
            &SuggestRequest {
                exclude_id: Some(i % 6 + 1),
            },
        )
        .unwrap();
        repo::search(&c, &req("quest"), true).unwrap();
        repo::search(&c, &req("a"), true).unwrap();
        repo::get_game(&c, 1).unwrap();
        repo::get_cover(&c, 1).unwrap();
        repo::library_info(&c, "ro.db").unwrap();
    }
    assert!(c.execute("UPDATE juegos SET estado='x'", []).is_err());
    assert!(c.execute("DELETE FROM juegos", []).is_err());
    assert!(c.execute("CREATE TABLE z(a)", []).is_err());
    let after = snapshot(&p);
    assert!(before.0 == after.0, "file bytes changed");
    assert_eq!(before.1, after.1, "mtime changed");
    let n: i64 = c
        .query_row("SELECT count(*) FROM juegos WHERE estado='x'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(n, 0);
}

#[test]
fn db_path_guard() {
    assert!(db::check_path_allowed(Path::new("a/juegos.db"), false).is_err());
    assert!(db::check_path_allowed(Path::new("a/JUEGOS.DB"), false).is_err());
    assert!(matches!(
        db::check_path_allowed(Path::new("juegos.db"), false),
        Err(AppError::Config(_))
    ));
    assert!(db::check_path_allowed(Path::new("a/juegos.db"), true).is_ok());
    assert!(db::check_path_allowed(Path::new("a/juegos.dev.db"), false).is_ok());
    assert!(db::check_path_allowed(Path::new("a/copy.db"), false).is_ok());
}

#[test]
fn search_tag_filter() {
    let (_d, c) = open_std();
    let mut r = req("");
    r.tag = Some("RPG".into()); // stored as 'rpg': NOCASE
    let p = repo::search(&c, &r, true).unwrap();
    assert_eq!(titles(&p), vec!["Zeta Quest"]);
    assert_eq!(p.total, 1);
    r.tag = Some("fantasía".into());
    assert_eq!(repo::search(&c, &r, true).unwrap().total, 1);
    // combined with query and estado
    r.tag = Some("accion".into());
    r.query = "quest".into();
    r.estado = Some("Sin empezar".into());
    let p = repo::search(&c, &r, true).unwrap();
    assert_eq!(titles(&p), vec!["Zeta Quest"]);
    assert_eq!(p.strategy, "fts");
    r.estado = Some("Terminado".into());
    assert_eq!(repo::search(&c, &r, true).unwrap().total, 0);
    // unknown tag
    r.estado = None;
    r.tag = Some("no-existe".into());
    let p = repo::search(&c, &r, true).unwrap();
    assert_eq!((p.total, p.items.len()), (0, 0));
    // validation
    r.tag = Some("x".repeat(101));
    assert!(matches!(
        repo::search(&c, &r, true),
        Err(AppError::InvalidInput(_))
    ));
    r.tag = Some("x".repeat(100));
    assert!(repo::search(&c, &r, true).is_ok());
}

#[test]
fn library_info_tags_counts_and_order() {
    let dir = TempDir::new().unwrap();
    let rows = vec![
        Row {
            tags: &["zeta", "Alfa"],
            ..row("A", "", "Sin empezar", "Japonés")
        },
        Row {
            tags: &["alfa"],
            ..row("B", "", "Sin empezar", "Japonés")
        },
    ];
    let p = make_db(&dir, "tg.db", &rows, true);
    {
        // tag without games must not be listed
        let w = Connection::open(&p).unwrap();
        w.execute("INSERT INTO tags(nombre) VALUES ('huerfano')", [])
            .unwrap();
    }
    let c = db::open_read_only(&p).unwrap();
    let i = repo::library_info(&c, "tg.db").unwrap();
    let got: Vec<(&str, i64)> = i.tags.iter().map(|t| (t.value.as_str(), t.count)).collect();
    assert_eq!(got, vec![("Alfa", 2), ("zeta", 1)]);
}

#[test]
fn title_desc_ties_use_id_ascending() {
    let dir = TempDir::new().unwrap();
    let rows = vec![
        row("Same", "A-1", "Sin empezar", "Japonés"),
        row("zzz", "A-2", "Sin empezar", "Japonés"),
        row("same", "A-3", "Sin empezar", "Japonés"),
    ];
    let p = make_db(&dir, "tie.db", &rows, true);
    let c = db::open_read_only(&p).unwrap();
    let mut r = req("");
    r.sort = SortKey::TitleDesc;
    let ids: Vec<i64> = repo::search(&c, &r, true)
        .unwrap()
        .items
        .iter()
        .map(|g| g.id)
        .collect();
    assert_eq!(ids, vec![2, 1, 3]);
}

#[test]
fn short_normalized_code_matches_by_like() {
    let (_d, c) = open_std();
    // "r-y" normalizes to "ry" (2 chars) -> codes containing "ry" (RY-1000, RY-1001)
    let p = repo::search(&c, &req("r-y"), true).unwrap();
    assert_eq!(p.strategy, "fts");
    assert_eq!(titles(&p), vec!["alpha Tale", "Zeta Quest"]);
    // "ab-" -> norm "ab": AB-5 and ab_77 codes
    let p = repo::search(&c, &req("ab-"), true).unwrap();
    assert_eq!(titles(&p), vec!["Epsilon", "Gamma Ray"]);
    // no cross-field matching (pagina + tipo are not concatenated)
    let p = repo::search(&c, &req("Aventura Aventura"), true).unwrap();
    assert_eq!(p.total, 0);
}

#[test]
fn like_path_folds_unicode() {
    let dir = TempDir::new().unwrap();
    let rows = vec![
        Row {
            desc: "Una aventura épica",
            ..row("Épica Saga", "X-1", "Sin empezar", "Japonés")
        },
        row("Plain", "X-2", "Sin empezar", "Inglés"),
    ];
    let p = make_db(&dir, "fold.db", &rows, true);
    let c = db::open_read_only(&p).unwrap();
    for q in ["é", "É"] {
        let r = repo::search(&c, &req(q), true).unwrap();
        assert_eq!(r.strategy, "like", "{q}");
        assert_eq!(titles(&r), vec!["Épica Saga"], "{q}");
    }
    let r = repo::search(&c, &req("zz"), true).unwrap();
    assert_eq!(r.total, 0);
    // also without juegos_busqueda
    let p2 = make_db(&dir, "fold2.db", &rows, false);
    let c2 = db::open_read_only(&p2).unwrap();
    let r = repo::search(&c2, &req("É"), false).unwrap();
    assert_eq!(titles(&r), vec!["Épica Saga"]);
}

#[test]
fn code_sort_two_phase_semantics() {
    let dir = TempDir::new().unwrap();
    let rows = vec![
        row("b", "", "Sin empezar", "Japonés"),
        row("c", "ry-2", "Sin empezar", "Japonés"),
        row("a", "  ", "Sin empezar", "Japonés"),
        row("d", "RY-1", "Sin empezar", "Japonés"),
        row("e", "ry-1", "Sin empezar", "Japonés"),
    ];
    let p = make_db(&dir, "code.db", &rows, true);
    let c = db::open_read_only(&p).unwrap();
    let mut r = req("");
    r.sort = SortKey::Code;
    let page = repo::search(&c, &r, true).unwrap();
    // codes first (NOCASE, then title); blank codes last, ordered by raw code ("" < "  ")
    assert_eq!(titles(&page), vec!["d", "e", "c", "b", "a"]);
    // limit/offset keep order across the two phases
    r.limit = 2;
    r.offset = 2;
    let page = repo::search(&c, &r, true).unwrap();
    assert_eq!(titles(&page), vec!["c", "b"]);
    assert_eq!(page.total, 5);
}

#[test]
fn suggest_is_roughly_uniform() {
    let dir = TempDir::new().unwrap();
    let mut rows: Vec<Row> = ["a", "b", "c", "d", "e"]
        .iter()
        .map(|t| row(t, "", "Sin empezar", "Japonés"))
        .collect();
    rows.push(row("done", "", "Terminado", "Japonés"));
    let p = make_db(&dir, "sg.db", &rows, true);
    let c = db::open_read_only(&p).unwrap();
    let mut seen = std::collections::HashMap::new();
    for _ in 0..2000 {
        let s = repo::suggest(&c, &SuggestRequest::default()).unwrap();
        *seen.entry(s.game.unwrap().summary.titulo).or_insert(0) += 1;
    }
    assert_eq!(seen.len(), 5, "{seen:?}");
    assert!(!seen.contains_key("done"));
    assert!(seen.values().all(|&n| n > 250), "{seen:?}");
}
