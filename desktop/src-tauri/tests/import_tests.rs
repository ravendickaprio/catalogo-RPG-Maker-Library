//! Pruebas del importador (Fase 6) sobre DB y árboles de carpetas temporales.
use rpg_library_lib::commands::Ready;
use rpg_library_lib::importer::{
    build_preview, db_key, read_history, run_import_sync, scan_tree, undo_last_import_sync,
    ImportItemInput, ImportPreview, ImportRequest, ItemStatus, RawScan,
};
use rpg_library_lib::models::*;
use rpg_library_lib::{repo, write};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use tempfile::TempDir;

mod common;

fn make_db(dir: &TempDir, name: &str) -> PathBuf {
    common::make_db_with(
        dir,
        name,
        "INSERT INTO juegos(titulo,codigo,estado,idioma,tipo,pagina,descripcion,ruta) VALUES
           ('Zeta Quest','RY-1000','Sin empezar','Japonés','RPG','ryuugames','', 'C:/Otro/Zeta.zip'),
           ('Alpha Tale','RY-1001','Terminado','Inglés','RPG','ryuugames','', NULL),
           ('Beta Land','','Terminado','Inglés','Visual Novel','Otome','', NULL);",
    )
}

fn touch(p: &Path) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, b"").unwrap();
}

fn scan(root: &Path) -> RawScan {
    scan_tree(root, &AtomicBool::new(false), &mut |_| {}).unwrap()
}

fn names(raw: &RawScan) -> Vec<String> {
    let mut v: Vec<String> = raw
        .candidates
        .iter()
        .map(|c| format!("{}:{}", c.kind, c.name))
        .collect();
    v.sort();
    v
}

fn preview(ready: &Ready, root: &Path) -> ImportPreview {
    let conn = ready.lock_conn().unwrap();
    build_preview(&conn, &root.to_string_lossy(), scan(root)).unwrap()
}

fn new_items(p: &ImportPreview) -> Vec<ImportItemInput> {
    p.items
        .iter()
        .filter(|i| i.status == ItemStatus::Nuevo)
        .map(|i| ImportItemInput {
            key: i.key.clone(),
            path: i.path.clone(),
            titulo: i.parsed.titulo.clone(),
            codigo: i.parsed.codigo.clone(),
            estado: i.parsed.estado.clone(),
            idioma: i.parsed.idioma.clone(),
        })
        .collect()
}

fn req(root: &Path, items: Vec<ImportItemInput>) -> ImportRequest {
    ImportRequest {
        root: root.to_string_lossy().into_owned(),
        items,
        pagina: "ryuugames".into(),
        tipo: "RPG".into(),
    }
}

fn item(key: &str, path: &str, titulo: &str, codigo: &str) -> ImportItemInput {
    ImportItemInput {
        key: key.into(),
        path: path.into(),
        titulo: titulo.into(),
        codigo: codigo.into(),
        estado: "Sin empezar".into(),
        idioma: "Japonés".into(),
    }
}

fn count(ready: &Ready, sql: &str) -> i64 {
    ready
        .lock_conn()
        .unwrap()
        .query_row(sql, [], |r| r.get(0))
        .unwrap()
}

/// DB + carpeta con tres juegos nuevos; devuelve (dir, ready, root, history).
fn imported_fixture() -> (TempDir, Ready, PathBuf, PathBuf) {
    let dir = TempDir::new().unwrap();
    let p = make_db(&dir, "t.db");
    let ready = Ready::open(&p, false).unwrap();
    let root = dir.path().join("juegos");
    touch(&root.join("[F] RY-2001 Uno_EN.zip"));
    touch(&root.join("RY-2002 Dos.rar"));
    touch(&root.join("sub").join("RY-2003 Tres.7z"));
    let hist = dir.path().join("data").join("import-history.jsonl");
    (dir, ready, root, hist)
}

#[test]
fn scan_finds_candidates_and_ignores_the_rest() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    touch(&root.join("[F] RY-1_en.zip"));
    touch(&root.join("a").join("RY-2 Game.rar"));
    touch(&root.join("b").join("sin codigo.7z"));
    touch(&root.join("b").join("MAYUS.ZIP"));
    touch(&root.join("c").join("RY-3 Carpeta").join("interno.zip"));
    touch(&root.join(".git").join("RY-9.zip"));
    touch(&root.join("node_modules").join("x.zip"));
    touch(&root.join("__pycache__").join("y.zip"));
    touch(&root.join(".venv").join("z.zip"));
    touch(&root.join("notas.txt"));
    touch(&root.join("RY-4 imagen.png"));
    fs::create_dir_all(root.join("vacia")).unwrap();
    let raw = scan(root);
    assert_eq!(
        names(&raw),
        vec![
            "archivo:MAYUS",
            "archivo:RY-2 Game",
            "archivo:[F] RY-1_en",
            "archivo:interno",
            "archivo:sin codigo",
            "carpeta:RY-3 Carpeta",
        ]
    );
    assert!(!raw.cancelled && !raw.truncated);
    assert_eq!(raw.unreadable, 0);
}

#[test]
fn scan_rejects_missing_root_and_honours_cancel() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("no-existe");
    let e = scan_tree(&missing, &AtomicBool::new(false), &mut |_| {}).unwrap_err();
    assert_eq!(e.kind(), "InvalidInput");

    touch(&dir.path().join("RY-1.zip"));
    let raw = scan_tree(dir.path(), &AtomicBool::new(true), &mut |_| {}).unwrap();
    assert!(raw.cancelled);
    assert!(raw.candidates.is_empty());
}

#[test]
fn preview_classifies_statuses() {
    let dir = TempDir::new().unwrap();
    let p = make_db(&dir, "t.db");
    let root = dir.path().join("juegos");
    touch(&root.join("RY-7000 Nuevo.zip"));
    touch(&root.join("ry_1001 duplicado por codigo.zip")); // normaliza a RY1001 de la DB
    touch(&root.join("RY-7002 Ruta.zip"));
    touch(&root.join("x").join("RY-7003 Lote A.zip"));
    touch(&root.join("y").join("RY-7003 Lote B.zip"));
    touch(&root.join("sin codigo.zip"));
    // Una fila de la DB con la ruta de «RY-7002 Ruta.zip» en otra grafía (mayúsculas, `/`).
    let alt = root
        .join("RY-7002 Ruta.zip")
        .to_string_lossy()
        .replace('\\', "/")
        .to_uppercase();
    {
        let c = Connection::open(&p).unwrap();
        c.execute(
            "INSERT INTO juegos(titulo,codigo,estado,idioma,ruta) VALUES ('Ruta vieja','','Sin empezar','Japonés',?)",
            [alt],
        )
        .unwrap();
    }
    let ready = Ready::open(&p, false).unwrap();
    let pv = preview(&ready, &root);
    let st = |needle: &str| {
        pv.items
            .iter()
            .filter(|i| i.name.contains(needle))
            .map(|i| i.status)
            .collect::<Vec<_>>()
    };
    assert_eq!(st("Nuevo"), vec![ItemStatus::Nuevo]);
    assert_eq!(
        st("duplicado por codigo"),
        vec![ItemStatus::DuplicadoCodigo]
    );
    assert_eq!(st("Ruta"), vec![ItemStatus::DuplicadoRuta]);
    let mut lote = st("Lote");
    lote.sort_by_key(|s| *s as u8);
    assert_eq!(lote, vec![ItemStatus::Nuevo, ItemStatus::DuplicadoLote]);
    assert_eq!(st("sin codigo"), vec![ItemStatus::SinCodigo]);
    let c = &pv.counts;
    assert_eq!(
        (
            c.total,
            c.nuevo,
            c.duplicado_codigo,
            c.duplicado_ruta,
            c.duplicado_lote,
            c.sin_codigo
        ),
        (6, 2, 1, 1, 1, 1)
    );
    // Valores por defecto: lo más frecuente de la DB.
    assert_eq!(pv.defaults.pagina, "ryuugames");
    assert_eq!(pv.defaults.tipo, "RPG");
}

#[test]
fn import_inserts_with_defaults_and_fts_finds_them() {
    let (_d, ready, root, hist) = imported_fixture();
    let pv = preview(&ready, &root);
    let items = new_items(&pv);
    assert_eq!(items.len(), 3);
    let r = run_import_sync(&ready, &req(&root, items), Some(&hist)).unwrap();
    assert_eq!(r.inserted.len(), 3);
    assert!(r.skipped.is_empty());
    assert!(r.import_id.is_some() && r.warning.is_none());
    assert_eq!(count(&ready, "SELECT count(*) FROM juegos"), 6);
    let conn = ready.lock_conn().unwrap();
    let (desc, img_null, punt, pagina, tipo, estado, idioma, ruta): (
        String,
        bool,
        f64,
        String,
        String,
        String,
        String,
        String,
    ) = conn
        .query_row(
            "SELECT descripcion, imagen IS NULL, puntuacion, pagina, tipo, estado, idioma, ruta FROM juegos WHERE codigo='RY-2001'",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(desc, "");
    assert!(img_null);
    assert_eq!(punt, 0.0);
    assert_eq!((pagina.as_str(), tipo.as_str()), ("ryuugames", "RPG"));
    assert_eq!((estado.as_str(), idioma.as_str()), ("Terminado", "Inglés"));
    assert!(ruta.ends_with("[F] RY-2001 Uno_EN.zip"));
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM juego_tags WHERE juego_id IN (SELECT id FROM juegos WHERE codigo LIKE 'RY-200%')",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    let page = repo::search(
        &conn,
        &SearchRequest {
            query: "Tres".into(),
            estado: None,
            idioma: None,
            tag: None,
            sort: SortKey::TitleAsc,
            offset: 0,
            limit: 10,
        },
        ready.fts_available,
    )
    .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].codigo, "RY-2003");
    // Historial escrito (una línea).
    let text = fs::read_to_string(&hist).unwrap();
    assert_eq!(text.lines().count(), 1);
    drop(conn);
    let h = read_history(&hist, db_key(&ready).as_deref(), 20);
    assert_eq!(h.len(), 1);
    assert_eq!(h[0].inserted_count, 3);
    assert!(!h[0].undone);
}

#[test]
fn import_skips_duplicates_created_after_the_scan() {
    let (_d, ready, root, hist) = imported_fixture();
    let pv = preview(&ready, &root);
    let items = new_items(&pv);
    // Después del análisis: otro proceso añade un código y una ruta.
    {
        let conn = ready.lock_conn().unwrap();
        conn.execute(
            "INSERT INTO juegos(titulo,codigo,estado,idioma) VALUES ('Tardío','RY-2002','Sin empezar','Japonés')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO juegos(titulo,codigo,estado,idioma,ruta) VALUES ('Ruta tardía','','Sin empezar','Japonés',?)",
            [items.iter().find(|i| i.codigo == "RY-2003").unwrap().path.to_uppercase()],
        )
        .unwrap();
    }
    let mut items = items;
    items.push(item("dup-lote", "C:/x/otra.zip", "Otra copia", "ry 2001"));
    items.push(item("sin-codigo", "C:/x/sc.zip", "Sin código", "  "));
    let r = run_import_sync(&ready, &req(&root, items), Some(&hist)).unwrap();
    assert_eq!(r.inserted.len(), 1);
    assert_eq!(r.inserted[0].codigo, "RY-2001");
    assert_eq!(r.skipped.len(), 4);
    let reason = |c: &str| {
        r.skipped
            .iter()
            .find(|s| {
                pv.items
                    .iter()
                    .any(|i| i.key == s.key && i.parsed.codigo == c)
            })
            .map(|s| s.reason.clone())
            .unwrap()
    };
    assert!(reason("RY-2002").contains("ya lo tiene"));
    assert!(reason("RY-2003").contains("ruta"));
    let by_key = |k: &str| {
        r.skipped
            .iter()
            .find(|s| s.key == k)
            .unwrap()
            .reason
            .clone()
    };
    assert!(by_key("dup-lote").contains("ya lo tiene"));
    assert!(by_key("sin-codigo").contains("omitido"));
}

#[test]
fn import_refuses_read_only_and_empty() {
    let (_d, _ready, root, hist) = imported_fixture();
    let p = _d.path().join("t.db");
    let ro = Ready::open(&p, true).unwrap();
    let e = run_import_sync(
        &ro,
        &req(&root, vec![item("a", "C:/a.zip", "A", "RY-5")]),
        Some(&hist),
    )
    .unwrap_err();
    assert_eq!(e.kind(), "ReadOnly");
    assert_eq!(count(&ro, "SELECT count(*) FROM juegos"), 3);
    assert!(!hist.exists());
    let rw = Ready::open(&p, false).unwrap();
    let e = run_import_sync(&rw, &req(&root, vec![]), Some(&hist)).unwrap_err();
    assert_eq!(e.kind(), "InvalidInput");
}

#[test]
fn undo_deletes_unchanged_keeps_edited_and_cannot_repeat() {
    let (_d, ready, root, hist) = imported_fixture();
    let pv = preview(&ready, &root);
    let r = run_import_sync(&ready, &req(&root, new_items(&pv)), Some(&hist)).unwrap();
    assert_eq!(r.inserted.len(), 3);
    // Se edita un juego importado.
    let edited = r.inserted.iter().find(|g| g.codigo == "RY-2002").unwrap();
    {
        let conn = ready.lock_conn().unwrap();
        let g = repo::get_game(&conn, edited.id).unwrap();
        let input = GameInput {
            titulo: "Editado".into(),
            descripcion: "algo".into(),
            estado: g.summary.estado.clone(),
            idioma: g.summary.idioma.clone(),
            ruta: g.ruta.clone(),
            codigo: g.summary.codigo.clone(),
            pagina: g.summary.pagina.clone(),
            tipo: g.summary.tipo.clone(),
            puntuacion: 4.0,
            tags: vec![],
        };
        write::update_game(
            &conn,
            edited.id,
            &input,
            &WriteOptions {
                allow_duplicate_code: false,
                expected_revision: Some(g.revision),
            },
        )
        .unwrap();
    }
    let u = undo_last_import_sync(&ready, &hist).unwrap();
    assert_eq!(u.deleted, 2);
    assert_eq!(u.kept.len(), 1);
    assert_eq!(u.kept[0].id, edited.id);
    assert!(u.kept[0].reason.contains("editó"));
    assert!(u.warning.is_none());
    assert_eq!(count(&ready, "SELECT count(*) FROM juegos"), 4);
    assert_eq!(
        count(&ready, "SELECT count(*) FROM juegos WHERE codigo='RY-2002'"),
        1
    );
    let h = read_history(&hist, db_key(&ready).as_deref(), 20);
    assert!(h[0].undone && h[0].undone_at.is_some());
    let e = undo_last_import_sync(&ready, &hist).unwrap_err();
    assert_eq!(e.kind(), "InvalidInput");
    assert_eq!(fs::read_to_string(&hist).unwrap().lines().count(), 2);
}

#[test]
fn undo_without_history_is_not_found_and_read_only_refused() {
    let (_d, ready, _root, hist) = imported_fixture();
    assert_eq!(
        undo_last_import_sync(&ready, &hist).unwrap_err().kind(),
        "NotFound"
    );
    let ro = Ready::open(&_d.path().join("t.db"), true).unwrap();
    assert_eq!(
        undo_last_import_sync(&ro, &hist).unwrap_err().kind(),
        "ReadOnly"
    );
}

#[test]
fn corrupted_history_lines_are_ignored() {
    let (_d, ready, root, hist) = imported_fixture();
    fs::create_dir_all(hist.parent().unwrap()).unwrap();
    fs::write(
        &hist,
        "esto no es json\n{\"kind\":\"import\"}\n\n{\"kind\":\"otro\"}\n",
    )
    .unwrap();
    let pv = preview(&ready, &root);
    run_import_sync(&ready, &req(&root, new_items(&pv)), Some(&hist)).unwrap();
    let h = read_history(&hist, db_key(&ready).as_deref(), 20);
    assert_eq!(h.len(), 1);
    assert_eq!(undo_last_import_sync(&ready, &hist).unwrap().deleted, 3);
}

#[test]
fn undo_is_bound_to_the_database() {
    let (d, ready_a, root, hist) = imported_fixture();
    let pv = preview(&ready_a, &root);
    let r = run_import_sync(&ready_a, &req(&root, new_items(&pv)), Some(&hist)).unwrap();
    assert_eq!(r.inserted.len(), 3);
    // DB B con los mismos ids (4..6) que acaba de crear A.
    let pb = make_db(&d, "b.db");
    {
        let c = Connection::open(&pb).unwrap();
        for n in 0..3 {
            c.execute(
                "INSERT INTO juegos(titulo,codigo,estado,idioma) VALUES (?,?, 'Sin empezar','Japonés')",
                [format!("B{n}"), format!("RY-90{n}")],
            )
            .unwrap();
        }
    }
    let ready_b = Ready::open(&pb, false).unwrap();
    assert!(read_history(&hist, db_key(&ready_b).as_deref(), 20).is_empty());
    let e = undo_last_import_sync(&ready_b, &hist).unwrap_err();
    assert_eq!(e.kind(), "NotFound");
    assert_eq!(count(&ready_b, "SELECT count(*) FROM juegos"), 6);
    assert_eq!(fs::read_to_string(&hist).unwrap().lines().count(), 1);
    // Los registros sin `db` (formato antiguo) tampoco se deshacen.
    let old = hist.with_file_name("old.jsonl");
    fs::write(
        &old,
        "{\"kind\":\"import\",\"id\":\"imp-1\",\"timestamp\":\"t\",\"root\":\"r\",\"inserted\":[{\"id\":4,\"revision\":\"x\"}],\"skippedCount\":0,\"backupPath\":null}\n",
    )
    .unwrap();
    assert_eq!(
        undo_last_import_sync(&ready_a, &old).unwrap_err().kind(),
        "NotFound"
    );
    // En A sí se deshace.
    assert_eq!(undo_last_import_sync(&ready_a, &hist).unwrap().deleted, 3);
}

#[test]
fn concurrent_undos_are_serialized() {
    let (_d, ready, root, hist) = imported_fixture();
    let pv = preview(&ready, &root);
    run_import_sync(&ready, &req(&root, new_items(&pv)), Some(&hist)).unwrap();
    let results: Vec<_> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..2)
            .map(|_| s.spawn(|| undo_last_import_sync(&ready, &hist)))
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let ok = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(ok, 1);
    let err = results.iter().find_map(|r| r.as_ref().err()).unwrap();
    assert_eq!(err.kind(), "InvalidInput");
    assert_eq!(fs::read_to_string(&hist).unwrap().lines().count(), 2);
    assert_eq!(count(&ready, "SELECT count(*) FROM juegos"), 3);
}

#[test]
fn undo_history_failure_is_a_warning_not_an_error() {
    let (_d, ready, root, hist) = imported_fixture();
    let pv = preview(&ready, &root);
    run_import_sync(&ready, &req(&root, new_items(&pv)), Some(&hist)).unwrap();
    let mut perms = fs::metadata(&hist).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&hist, perms.clone()).unwrap();
    let r = undo_last_import_sync(&ready, &hist);
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    fs::set_permissions(&hist, perms).unwrap();
    let u = r.unwrap();
    assert_eq!(u.deleted, 3);
    assert!(u.warning.unwrap().contains("historial"));
    assert_eq!(count(&ready, "SELECT count(*) FROM juegos"), 3);
}
