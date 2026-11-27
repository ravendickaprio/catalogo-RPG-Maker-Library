//! Pruebas del centro de mantenimiento (Fase 7) sobre DB y carpetas temporales.
use rpg_library_lib::commands::Ready;
use rpg_library_lib::maintenance::*;
use rpg_library_lib::{repo, write};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use tempfile::TempDir;

mod common;

const SEED: &str = "
INSERT INTO juegos(titulo,codigo,descripcion,estado,ruta,imagen,puntuacion) VALUES
 ('RY-100 Foo','RY-100','','Sin empezar','',NULL,1.5),
 ('ry_200','ry 100','x','Empezado','',NULL,2),
 ('Rylan Quest','',' ','Sin empezar','',NULL,0),
 ('RY','RY-300','d','Sin empezar','',NULL,3),
 ('Good','RY-300','d','Sin empezar','/tmp/[F] RY-300 Good.zip',x'ff00ff',4.5),
 ('Plain','RY-400','d','Terminado','C:/a/RY-400 Plain.zip',NULL,0),
 ('Done','RY-500','d','terminado','C:/a/[F] RY-500 Done.zip',NULL,5);
INSERT INTO tags(nombre) VALUES ('Tag'),('Huerfana');
INSERT INTO juego_tags(juego_id, tag_id) VALUES (5,1),(1,1);
";

fn fixture() -> (TempDir, PathBuf, Ready) {
    let dir = TempDir::new().unwrap();
    let p = common::make_db_with(&dir, "t.db", SEED);
    let ready = Ready::open(&p, false).unwrap();
    (dir, p, ready)
}

fn ids_of(page: &IssuePage) -> Vec<i64> {
    page.items.iter().map(|i| i.id).collect()
}

fn list_check(ready: &Ready, check: CheckId, offset: i64, limit: i64) -> IssuePage {
    let c = ready.lock_conn().unwrap();
    list(
        &c,
        &ListRequest {
            check,
            offset,
            limit,
        },
    )
    .unwrap()
}

fn q(ready: &Ready, sql: &str) -> i64 {
    ready
        .lock_conn()
        .unwrap()
        .query_row(sql, [], |r| r.get(0))
        .unwrap()
}

fn qs(ready: &Ready, sql: &str) -> String {
    ready
        .lock_conn()
        .unwrap()
        .query_row(sql, [], |r| r.get(0))
        .unwrap()
}

/// Campos que ningún lote puede tocar.
fn untouched(ready: &Ready) -> Vec<String> {
    let c = ready.lock_conn().unwrap();
    let mut st = c
        .prepare("SELECT id || '|' || coalesce(hex(imagen),'') || '|' || coalesce(descripcion,'') || '|' || puntuacion || '|' || coalesce(ruta,'') FROM juegos ORDER BY id")
        .unwrap();
    st.query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

fn preview(ready: &Ready, ids: &[i64], action: BatchAction) -> BatchPreview {
    let c = ready.lock_conn().unwrap();
    preview_batch(
        &c,
        &BatchRequest {
            ids: ids.to_vec(),
            action,
        },
    )
    .unwrap()
}

fn revs(p: &BatchPreview) -> HashMap<i64, String> {
    p.changes
        .iter()
        .map(|c| (c.id, c.revision.clone()))
        .collect()
}

fn set_estado(v: &str) -> BatchAction {
    BatchAction::SetField {
        field: BatchField::Estado,
        value: v.into(),
    }
}

fn apply(
    ready: &Ready,
    ids: &[i64],
    action: BatchAction,
    undo: &Path,
) -> Result<BatchResult, rpg_library_lib::error::AppError> {
    let req = BatchRequest {
        ids: ids.to_vec(),
        action,
    };
    let p = {
        let c = ready.lock_conn().unwrap();
        preview_batch(&c, &req)?
    };
    apply_batch(ready, &req, &revs(&p), Some(undo))
}

// ------------------------------------------------------------------ detectores

#[test]
fn summary_counts_each_detector() {
    let (_d, _p, ready) = fixture();
    let c = ready.lock_conn().unwrap();
    let s: HashMap<_, _> = summary(&c)
        .unwrap()
        .into_iter()
        .map(|x| (format!("{:?}", x.check), x.count))
        .collect();
    assert_eq!(s["SinDescripcion"], 2);
    assert_eq!(s["SinPortada"], 6);
    assert_eq!(s["SinTags"], 5);
    assert_eq!(s["CodigoDuplicado"], 4);
    assert_eq!(s["CodigoVacio"], 1);
    assert_eq!(s["TituloRy"], 3);
    assert_eq!(s["EstadoInconsistente"], 1);
    assert!(!s.contains_key("RutaInexistente"));
}

#[test]
fn list_each_detector_with_details() {
    let (_d, _p, ready) = fixture();
    assert_eq!(
        ids_of(&list_check(&ready, CheckId::SinDescripcion, 0, 50)),
        [1, 3]
    );
    assert_eq!(
        ids_of(&list_check(&ready, CheckId::SinPortada, 0, 50)),
        [1, 2, 3, 4, 6, 7]
    );
    assert_eq!(
        ids_of(&list_check(&ready, CheckId::SinTags, 0, 50)),
        [2, 3, 4, 6, 7]
    );
    assert_eq!(
        ids_of(&list_check(&ready, CheckId::CodigoVacio, 0, 50)),
        [3]
    );
    // «RY-100 Foo», «ry_200» y «RY» (el título «Rylan Quest» no cuenta).
    assert_eq!(
        ids_of(&list_check(&ready, CheckId::TituloRy, 0, 50)),
        [1, 2, 4]
    );

    // Códigos duplicados con distintos separadores y mayúsculas: «RY-100» y «ry 100».
    let dup = list_check(&ready, CheckId::CodigoDuplicado, 0, 50);
    assert_eq!(ids_of(&dup), [1, 2, 4, 5]);
    assert_eq!(dup.total, 4);
    let d1 = dup.items[0].detail.as_deref().unwrap();
    assert!(d1.contains("#2") && d1.contains("ry_200"), "{d1}");
    assert!(dup.items[2].detail.as_deref().unwrap().contains("#5"));

    // Estado inconsistente: solo el juego 5 (marca [F]); el 6 no tiene marca y el 7 coincide.
    let inc = list_check(&ready, CheckId::EstadoInconsistente, 0, 50);
    assert_eq!(ids_of(&inc), [5]);
    assert!(inc.items[0]
        .detail
        .as_deref()
        .unwrap()
        .contains("Terminado"));
}

#[test]
fn list_paginates_and_validates() {
    let (_d, _p, ready) = fixture();
    let p = list_check(&ready, CheckId::SinPortada, 4, 4);
    assert_eq!(p.total, 6);
    assert_eq!(ids_of(&p), [6, 7]);
    assert_eq!((p.offset, p.limit), (4, 4));
    let p = list_check(&ready, CheckId::CodigoDuplicado, 1, 2);
    assert_eq!((p.total, ids_of(&p)), (4, vec![2, 4]));
    assert!(p.items[0].detail.as_deref().unwrap().contains("#1"));
    let c = ready.lock_conn().unwrap();
    for (check, off, lim) in [
        (CheckId::SinPortada, 0, 0),
        (CheckId::SinPortada, 0, 501),
        (CheckId::SinPortada, -1, 10),
        (CheckId::RutaInexistente, 0, 10),
    ] {
        let e = list(
            &c,
            &ListRequest {
                check,
                offset: off,
                limit: lim,
            },
        )
        .unwrap_err();
        assert_eq!(e.kind(), "InvalidInput", "{check:?} {off} {lim}");
    }
}

#[test]
fn ry_prefix_rule() {
    for t in ["RY", "ry-1", "Ry_x", "RY algo", "  RY-1"] {
        assert!(has_ry_prefix(t), "{t}");
    }
    for t in ["", "R", "Rylan", "RYU", "Good RY-1", "XRY-1"] {
        assert!(!has_ry_prefix(t), "{t}");
    }
}

// ------------------------------------------------------------------ rutas

#[test]
fn foreign_path_rule() {
    assert_eq!(foreign_path("/home/x/a.zip", true), Some("ruta de Linux"));
    assert_eq!(foreign_path("C:\\x\\a.zip", true), None);
    assert_eq!(foreign_path("C:\\x\\a.zip", false), Some("ruta de Windows"));
    assert_eq!(foreign_path("d:/x/a.zip", false), Some("ruta de Windows"));
    assert_eq!(foreign_path("/home/x/a.zip", false), None);
    assert_eq!(foreign_path("C:", false), None);
}

#[test]
fn missing_paths_real_file_vs_missing_and_cancel() {
    let dir = TempDir::new().unwrap();
    let real = dir.path().join("RY-1 Real.zip");
    std::fs::write(&real, b"").unwrap();
    let missing = dir.path().join("RY-2 Falta.zip");
    let foreign = if cfg!(windows) {
        "/home/x/a.zip"
    } else {
        "C:\\x\\a.zip"
    };
    let seed = format!(
        "INSERT INTO juegos(titulo,ruta) VALUES ('Real','{}'),('Falta','{}'),('SinRuta',''),('Otro','{}'),('NullRuta',NULL);",
        real.to_string_lossy().replace('\'', "''"),
        missing.to_string_lossy().replace('\'', "''"),
        foreign
    );
    let p = common::make_db_with(&dir, "t.db", &seed);
    let ready = Ready::open(&p, false).unwrap();
    let mut progress = Vec::new();
    let scan = {
        let c = ready.lock_conn().unwrap();
        scan_missing_paths(&c, &AtomicBool::new(false), &mut |p| {
            progress.push(p.clone())
        })
        .unwrap()
    };
    assert!(!scan.cancelled);
    let got: Vec<(i64, String)> = scan
        .items
        .iter()
        .map(|i| (i.id, i.detail.clone().unwrap()))
        .collect();
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!(got[0], (2, "no existe".to_string()));
    assert_eq!(
        got[1].1,
        if cfg!(windows) {
            "ruta de Linux"
        } else {
            "ruta de Windows"
        }
    );
    let last = progress.last().unwrap();
    assert_eq!((last.checked, last.total), (3, 3));

    let cancelled = {
        let c = ready.lock_conn().unwrap();
        scan_missing_paths(&c, &AtomicBool::new(true), &mut |_| {}).unwrap()
    };
    assert!(cancelled.cancelled);
    assert!(cancelled.items.is_empty());
}

#[test]
fn maint_guard_is_exclusive() {
    let g = MaintGuard::acquire().unwrap();
    assert_eq!(MaintGuard::acquire().err().unwrap().kind(), "InvalidInput");
    request_cancel();
    assert!(g.cancel_flag().load(std::sync::atomic::Ordering::SeqCst));
    drop(g);
    let g2 = MaintGuard::acquire().unwrap();
    assert!(!g2.cancel_flag().load(std::sync::atomic::Ordering::SeqCst));
}

// ------------------------------------------------------------------ tags huérfanas

#[test]
fn orphan_tags_list_delete_and_undo_recreates_names() {
    let (d, _p, ready) = fixture();
    let undo = d.path().join("undo");
    let names = |r: &Ready| -> Vec<String> {
        let c = r.lock_conn().unwrap();
        list_orphan_tags(&c)
            .unwrap()
            .into_iter()
            .map(|t| t.nombre)
            .collect()
    };
    assert_eq!(names(&ready), ["Huerfana"]);
    let r = delete_orphan_tags(&ready, Some(&undo)).unwrap();
    assert_eq!(r.changed, 1);
    assert!(r.batch_id.is_some() && r.warning.is_none());
    assert!(names(&ready).is_empty());
    assert_eq!(q(&ready, "SELECT count(*) FROM tags"), 1);
    // Segunda ejecución: nada que borrar ni registro nuevo.
    let r2 = delete_orphan_tags(&ready, Some(&undo)).unwrap();
    assert_eq!((r2.changed, r2.batch_id), (0, None));

    let l = list_batches(&ready, &undo);
    assert_eq!(l.len(), 1);
    assert_eq!((l[0].changed, l[0].undone), (1, false));
    let u = undo_last_batch(&ready, &undo).unwrap();
    assert_eq!(u.restored, 1);
    assert_eq!(names(&ready), ["Huerfana"]);
    assert_eq!(q(&ready, "SELECT count(*) FROM juego_tags"), 2);
}

// ------------------------------------------------------------------ índice de búsqueda

fn fts_hits(ready: &Ready, term: &str) -> i64 {
    q(
        ready,
        &format!("SELECT count(*) FROM juegos_fts WHERE juegos_fts MATCH '\"{term}\"'"),
    )
}

#[test]
fn index_status_detects_corruption_and_rebuild_fixes_it() {
    let (_d, _p, ready) = fixture();
    let st = {
        let c = ready.lock_conn().unwrap();
        search_index_status(&c).unwrap()
    };
    assert_eq!(
        st,
        IndexStatus {
            juegos: 7,
            busqueda: 7,
            mismatched: 0,
            fts_ok: true
        }
    );
    assert_eq!(fts_hits(&ready, "foo"), 1);
    {
        let c = ready.lock_conn().unwrap();
        c.execute_batch(
            "UPDATE juegos_busqueda SET titulo='zzcorrupto' WHERE juego_id=1;
             DELETE FROM juegos_busqueda WHERE juego_id=2;
             INSERT INTO juegos_busqueda(juego_id, titulo) VALUES (999,'huerfano');",
        )
        .unwrap();
    }
    let st = {
        let c = ready.lock_conn().unwrap();
        search_index_status(&c).unwrap()
    };
    assert_eq!((st.juegos, st.busqueda, st.mismatched), (7, 7, 3));
    assert_eq!(fts_hits(&ready, "foo"), 0);

    let st = rebuild_search_index(&ready).unwrap();
    assert_eq!(
        st,
        IndexStatus {
            juegos: 7,
            busqueda: 7,
            mismatched: 0,
            fts_ok: true
        }
    );
    assert_eq!(fts_hits(&ready, "foo"), 1);
    assert_eq!(fts_hits(&ready, "zzcorrupto"), 0);
    assert_eq!(
        q(
            &ready,
            "SELECT count(*) FROM juegos_busqueda WHERE juego_id=999"
        ),
        0
    );
    // El tag del juego 1 está en el índice reconstruido.
    assert_eq!(
        qs(&ready, "SELECT tags FROM juegos_busqueda WHERE juego_id=1"),
        "Tag"
    );
    // No toca `juegos`.
    assert_eq!(q(&ready, "SELECT count(*) FROM juegos"), 7);
}

#[test]
fn index_status_on_read_only_connection() {
    let (_d, p, _ready) = fixture();
    let ro = Ready::open(&p, true).unwrap();
    let c = ro.lock_conn().unwrap();
    let st = search_index_status(&c).unwrap();
    assert_eq!((st.mismatched, st.fts_ok), (0, true));
    drop(c);
    assert_eq!(rebuild_search_index(&ro).unwrap_err().kind(), "ReadOnly");
}

#[test]
fn rebuild_without_fts_skips_the_fts_part() {
    let (_d, _p, ready) = fixture();
    {
        let c = ready.lock_conn().unwrap();
        c.execute_batch(
            "DROP TRIGGER juegos_fts_ai; DROP TRIGGER juegos_fts_ad; DROP TRIGGER juegos_fts_au;
             DROP TABLE juegos_fts;
             UPDATE juegos_busqueda SET descripcion='mala' WHERE juego_id=3;",
        )
        .unwrap();
    }
    let st = {
        let c = ready.lock_conn().unwrap();
        search_index_status(&c).unwrap()
    };
    assert_eq!((st.mismatched, st.fts_ok), (1, false));
    let st = rebuild_search_index(&ready).unwrap();
    assert_eq!((st.mismatched, st.fts_ok), (0, false));
    assert_eq!(
        qs(
            &ready,
            "SELECT descripcion FROM juegos_busqueda WHERE juego_id=3"
        ),
        " "
    );
}

// ------------------------------------------------------------------ lotes

#[test]
fn set_field_preview_matches_apply_and_untouched_fields_stay() {
    let (d, _p, ready) = fixture();
    let undo = d.path().join("undo");
    let before = untouched(&ready);
    let pv = preview(&ready, &[1, 2, 6, 99, 1], set_estado("Terminado"));
    // 1 y 2 cambian; 6 ya es «Terminado»; 99 no existe; el id repetido cuenta una vez.
    assert_eq!(pv.changes.iter().map(|c| c.id).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(pv.changes[1].before, "Empezado");
    assert_eq!(pv.changes[1].after, "Terminado");
    assert_eq!(pv.changes[1].titulo, "ry_200");
    assert_eq!(pv.skipped.iter().map(|s| s.id).collect::<Vec<_>>(), [6, 99]);
    // La vista previa no escribe.
    assert_eq!(
        qs(&ready, "SELECT estado FROM juegos WHERE id=1"),
        "Sin empezar"
    );

    let res = apply(&ready, &[1, 2, 6, 99, 1], set_estado("Terminado"), &undo).unwrap();
    assert_eq!(res.changed, 2);
    assert_eq!(res.skipped.len(), 2);
    assert!(res.backup_path.is_some() && res.batch_id.is_some() && res.warning.is_none());
    assert_eq!(
        qs(&ready, "SELECT estado FROM juegos WHERE id=1"),
        "Terminado"
    );
    assert_eq!(
        qs(&ready, "SELECT estado FROM juegos WHERE id=2"),
        "Terminado"
    );
    assert_eq!(untouched(&ready), before);
    // Un segundo lote idéntico no cambia nada y no crea registro.
    let again = apply(&ready, &[1, 2], set_estado("Terminado"), &undo).unwrap();
    assert_eq!((again.changed, again.batch_id), (0, None));
}

#[test]
fn set_field_other_fields_and_validation() {
    let (d, _p, ready) = fixture();
    let undo = d.path().join("undo");
    for (field, col, v) in [
        (BatchField::Idioma, "idioma", "Inglés"),
        (BatchField::Tipo, "tipo", "RPG"),
        (BatchField::Pagina, "pagina", "ryuugames"),
    ] {
        let r = apply(
            &ready,
            &[3],
            BatchAction::SetField {
                field,
                value: format!("  {v} "),
            },
            &undo,
        )
        .unwrap();
        assert_eq!(r.changed, 1, "{col}");
        assert_eq!(
            qs(&ready, &format!("SELECT {col} FROM juegos WHERE id=3")),
            v
        );
    }
    let c = ready.lock_conn().unwrap();
    let bad = |ids: Vec<i64>, a: BatchAction| {
        preview_batch(&c, &BatchRequest { ids, action: a })
            .unwrap_err()
            .kind()
    };
    assert_eq!(bad(vec![1], set_estado("  ")), "InvalidInput");
    assert_eq!(bad(vec![1], set_estado(&"x".repeat(51))), "InvalidInput");
    assert_eq!(
        bad(
            vec![1],
            BatchAction::SetField {
                field: BatchField::Pagina,
                value: "x".repeat(101)
            }
        ),
        "InvalidInput"
    );
    assert_eq!(bad(vec![], set_estado("a")), "InvalidInput");
    assert_eq!(bad(vec![0], set_estado("a")), "InvalidInput");
    assert_eq!(bad((1..=5001).collect(), set_estado("a")), "InvalidInput");
    assert_eq!(
        bad(vec![1], BatchAction::AddTag { tag: " ".into() }),
        "InvalidInput"
    );
}

#[test]
fn add_and_remove_tag_use_nocase_rules() {
    let (d, _p, ready) = fixture();
    let undo = d.path().join("undo");
    let tags = |id: i64| -> String {
        qs(
            &ready,
            &format!(
                "SELECT coalesce(group_concat(t.nombre,','),'') FROM juego_tags jt JOIN tags t ON t.id=jt.tag_id WHERE jt.juego_id={id}"
            ),
        )
    };
    let before = untouched(&ready);
    // Juego 1 ya tiene «Tag» (ignora mayúsculas): se omite; juego 2 la recibe sin duplicar la fila.
    let pv = preview(&ready, &[1, 2], BatchAction::AddTag { tag: "tAG".into() });
    assert_eq!(pv.changes.len(), 1);
    assert_eq!(
        (pv.changes[0].before.as_str(), pv.changes[0].after.as_str()),
        ("", "tAG")
    );
    assert_eq!(pv.skipped[0].id, 1);
    let r = apply(
        &ready,
        &[1, 2],
        BatchAction::AddTag { tag: "tAG".into() },
        &undo,
    )
    .unwrap();
    assert_eq!(r.changed, 1);
    assert_eq!(tags(2), "Tag");
    assert_eq!(q(&ready, "SELECT count(*) FROM tags"), 2);
    // Nuevo tag.
    let r = apply(
        &ready,
        &[2, 3],
        BatchAction::AddTag {
            tag: "Nuevo".into(),
        },
        &undo,
    )
    .unwrap();
    assert_eq!(r.changed, 2);
    assert_eq!(tags(3), "Nuevo");
    // `juegos_busqueda.tags` se actualiza por los triggers.
    assert!(qs(&ready, "SELECT tags FROM juegos_busqueda WHERE juego_id=3").contains("Nuevo"));
    // Quitar.
    let pv = preview(
        &ready,
        &[1, 2, 3],
        BatchAction::RemoveTag { tag: "TAG".into() },
    );
    assert_eq!(pv.changes.iter().map(|c| c.id).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(pv.skipped[0].id, 3);
    let r = apply(
        &ready,
        &[1, 2, 3],
        BatchAction::RemoveTag { tag: "TAG".into() },
        &undo,
    )
    .unwrap();
    assert_eq!(r.changed, 2);
    assert_eq!(tags(1), "");
    assert_eq!(tags(2), "Nuevo");
    assert_eq!(tags(5), "Tag");
    assert_eq!(untouched(&ready), before);
}

#[test]
fn fix_ry_titles_only_when_it_improves() {
    let dir = TempDir::new().unwrap();
    let seed = "INSERT INTO juegos(titulo,ruta,descripcion,puntuacion) VALUES
      ('RY-100 Foo','C:/a/[F] RY-100 Foo Bar_EN.zip','desc',3),
      ('RY-101 Sin ruta','','',0),
      ('Titulo bueno','C:/a/RY-102 Otro.zip','',0),
      ('RY-103','C:/a/RY-103.zip','',0),
      ('ry_104 x','C:\\\\a\\\\RY-104 Quest.rar','',0),
      ('RY-105 Mismo','C:/a/Mismo RY-105.zip','',0);";
    let p = common::make_db_with(&dir, "t.db", seed);
    let ready = Ready::open(&p, false).unwrap();
    let undo = dir.path().join("undo");
    let before = untouched(&ready);
    let ids: Vec<i64> = (1..=6).collect();
    let pv = preview(&ready, &ids, BatchAction::FixRyTitles);
    let ch: Vec<(i64, &str)> = pv
        .changes
        .iter()
        .map(|c| (c.id, c.after.as_str()))
        .collect();
    assert_eq!(ch, [(1, "Foo Bar"), (5, "Quest"), (6, "Mismo")]);
    assert_eq!(
        pv.skipped.iter().map(|s| s.id).collect::<Vec<_>>(),
        [2, 3, 4]
    );
    let r = apply(&ready, &ids, BatchAction::FixRyTitles, &undo).unwrap();
    assert_eq!(r.changed, 3);
    assert_eq!(
        qs(&ready, "SELECT titulo FROM juegos WHERE id=1"),
        "Foo Bar"
    );
    assert_eq!(
        qs(&ready, "SELECT titulo FROM juegos WHERE id=2"),
        "RY-101 Sin ruta"
    );
    assert_eq!(untouched(&ready), before);
    // El índice de búsqueda sigue la nueva titulación (trigger).
    assert_eq!(
        qs(
            &ready,
            "SELECT titulo FROM juegos_busqueda WHERE juego_id=1"
        ),
        "Foo Bar"
    );
    // Tras corregir, el detector ya no los lista.
    assert_eq!(list_check(&ready, CheckId::TituloRy, 0, 50).total, 2);
}

#[test]
fn apply_skips_rows_changed_since_preview_and_missing_revisions() {
    let (d, _p, ready) = fixture();
    let undo = d.path().join("undo");
    let req = BatchRequest {
        ids: vec![1, 2, 3, 4],
        action: set_estado("Terminado"),
    };
    let pv = {
        let c = ready.lock_conn().unwrap();
        preview_batch(&c, &req).unwrap()
    };
    let mut revisions = revs(&pv);
    revisions.remove(&3); // sin revisión
                          // Edición posterior de la fila 2 (cambia su revisión).
    ready
        .lock_conn()
        .unwrap()
        .execute("UPDATE juegos SET descripcion='editada' WHERE id=2", [])
        .unwrap();
    let r = apply_batch(&ready, &req, &revisions, Some(&undo)).unwrap();
    assert_eq!(r.changed, 2); // 1 y 4
    let skipped: HashMap<i64, String> =
        r.skipped.iter().map(|s| (s.id, s.reason.clone())).collect();
    assert!(skipped[&2].contains("modific"));
    assert!(skipped[&3].contains("falta"));
    assert_eq!(
        qs(&ready, "SELECT estado FROM juegos WHERE id=2"),
        "Empezado"
    );
    assert_eq!(
        qs(&ready, "SELECT estado FROM juegos WHERE id=4"),
        "Terminado"
    );
}

#[test]
fn read_only_refuses_writes_but_allows_preview() {
    let (d, p, _rw) = fixture();
    let undo = d.path().join("undo");
    let ro = Ready::open(&p, true).unwrap();
    let pv = preview(&ro, &[1], set_estado("Terminado"));
    assert_eq!(pv.changes.len(), 1);
    let req = BatchRequest {
        ids: vec![1],
        action: set_estado("Terminado"),
    };
    assert_eq!(
        apply_batch(&ro, &req, &revs(&pv), Some(&undo))
            .unwrap_err()
            .kind(),
        "ReadOnly"
    );
    assert_eq!(
        delete_orphan_tags(&ro, Some(&undo)).unwrap_err().kind(),
        "ReadOnly"
    );
    assert_eq!(undo_last_batch(&ro, &undo).unwrap_err().kind(), "ReadOnly");
    assert_eq!(rebuild_search_index(&ro).unwrap_err().kind(), "ReadOnly");
    assert!(!undo.exists());
    let c = ro.lock_conn().unwrap();
    assert!(summary(&c).is_ok());
}

#[test]
fn undo_restores_unchanged_keeps_edited_and_cannot_repeat() {
    let (d, _p, ready) = fixture();
    let undo = d.path().join("undo");
    let before = untouched(&ready);
    assert_eq!(
        undo_last_batch(&ready, &undo).unwrap_err().kind(),
        "NotFound"
    );
    let r = apply(&ready, &[1, 2, 3], set_estado("Terminado"), &undo).unwrap();
    assert_eq!(r.changed, 3);
    // Edita 2 después del lote y borra 3.
    {
        let c = ready.lock_conn().unwrap();
        c.execute("UPDATE juegos SET titulo='Editado' WHERE id=2", [])
            .unwrap();
        write::delete_game_row(&c, 3).unwrap();
    }
    let u = undo_last_batch(&ready, &undo).unwrap();
    assert_eq!(u.batch_id, r.batch_id.clone().unwrap());
    assert_eq!(u.restored, 1);
    assert!(u.warning.is_none());
    let kept: HashMap<i64, String> = u.kept.iter().map(|k| (k.id, k.reason.clone())).collect();
    assert_eq!(kept.len(), 2);
    assert!(kept[&2].contains("editó"));
    assert_eq!(kept[&3], "ya no existe");
    assert_eq!(u.kept.iter().find(|k| k.id == 2).unwrap().titulo, "Editado");
    assert_eq!(
        qs(&ready, "SELECT estado FROM juegos WHERE id=1"),
        "Sin empezar"
    );
    assert_eq!(
        qs(&ready, "SELECT estado FROM juegos WHERE id=2"),
        "Terminado"
    );
    let l = list_batches(&ready, &undo);
    assert_eq!((l.len(), l[0].undone, l[0].changed), (1, true, 3));
    assert_eq!(
        undo_last_batch(&ready, &undo).unwrap_err().kind(),
        "InvalidInput"
    );
    // Campos intactos (salvo la edición y el borrado hechos a mano).
    let after = untouched(&ready);
    assert_eq!(after.len(), before.len() - 1);
}

#[test]
fn undo_tags_restores_the_full_previous_list() {
    let (d, _p, ready) = fixture();
    let undo = d.path().join("undo");
    let tags = || -> String {
        qs(
            &ready,
            "SELECT coalesce(group_concat(t.nombre,','),'') FROM juego_tags jt JOIN tags t ON t.id=jt.tag_id WHERE jt.juego_id=1",
        )
    };
    assert_eq!(tags(), "Tag");
    apply(
        &ready,
        &[1],
        BatchAction::AddTag { tag: "Otro".into() },
        &undo,
    )
    .unwrap();
    assert!(tags().contains("Otro"));
    let u = undo_last_batch(&ready, &undo).unwrap();
    assert_eq!(u.restored, 1);
    assert_eq!(tags(), "Tag");
    // Quitar y deshacer.
    apply(
        &ready,
        &[1],
        BatchAction::RemoveTag { tag: "tag".into() },
        &undo,
    )
    .unwrap();
    assert_eq!(tags(), "");
    undo_last_batch(&ready, &undo).unwrap();
    assert_eq!(tags(), "Tag");
}

#[test]
fn undo_takes_the_latest_batch_and_list_is_newest_first() {
    let (d, _p, ready) = fixture();
    let undo = d.path().join("undo");
    apply(&ready, &[1], set_estado("Terminado"), &undo).unwrap();
    apply(
        &ready,
        &[1],
        BatchAction::SetField {
            field: BatchField::Idioma,
            value: "Inglés".into(),
        },
        &undo,
    )
    .unwrap();
    let l = list_batches(&ready, &undo);
    assert_eq!(l.len(), 2);
    assert!(l[0].id > l[1].id && l[0].description.contains("idioma"));
    undo_last_batch(&ready, &undo).unwrap();
    assert_eq!(
        qs(&ready, "SELECT idioma FROM juegos WHERE id=1"),
        "Español"
    );
    assert_eq!(
        qs(&ready, "SELECT estado FROM juegos WHERE id=1"),
        "Terminado"
    );
}

#[test]
fn batches_are_scoped_by_database() {
    let (d, _p, a) = fixture();
    let undo = d.path().join("undo");
    let pb = common::make_db_with(&d, "otra.db", SEED);
    let b = Ready::open(&pb, false).unwrap();
    apply(&a, &[1], set_estado("Terminado"), &undo).unwrap();
    assert_eq!(list_batches(&a, &undo).len(), 1);
    assert!(list_batches(&b, &undo).is_empty());
    assert_eq!(undo_last_batch(&b, &undo).unwrap_err().kind(), "NotFound");
    assert_eq!(
        qs(&b, "SELECT estado FROM juegos WHERE id=1"),
        "Sin empezar"
    );
    apply(&b, &[2], set_estado("Terminado"), &undo).unwrap();
    // Cada base deshace el suyo.
    assert_eq!(undo_last_batch(&a, &undo).unwrap().restored, 1);
    assert_eq!(
        qs(&a, "SELECT estado FROM juegos WHERE id=1"),
        "Sin empezar"
    );
    assert_eq!(qs(&b, "SELECT estado FROM juegos WHERE id=2"), "Terminado");
}

#[test]
fn concurrent_undos_are_serialized() {
    let (d, _p, ready) = fixture();
    let undo = d.path().join("undo");
    apply(&ready, &[1, 2], set_estado("Terminado"), &undo).unwrap();
    let results: Vec<_> = std::thread::scope(|s| {
        (0..4)
            .map(|_| s.spawn(|| undo_last_batch(&ready, &undo)))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect()
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(results
        .iter()
        .filter_map(|r| r.as_ref().err())
        .all(|e| e.kind() == "InvalidInput"));
}

#[test]
fn missing_undo_dir_is_a_warning_not_an_error() {
    let (_d, _p, ready) = fixture();
    let req = BatchRequest {
        ids: vec![1],
        action: set_estado("Terminado"),
    };
    let pv = preview(&ready, &[1], set_estado("Terminado"));
    let r = apply_batch(&ready, &req, &revs(&pv), None).unwrap();
    assert_eq!(r.changed, 1);
    assert!(r.batch_id.is_none());
    assert!(r.warning.unwrap().contains("deshacer"));
    assert_eq!(
        qs(&ready, "SELECT estado FROM juegos WHERE id=1"),
        "Terminado"
    );
}

#[test]
fn undo_record_failure_is_a_warning() {
    let (d, _p, ready) = fixture();
    // `undo` es un archivo, no una carpeta: no se puede crear el registro.
    let undo = d.path().join("undo");
    std::fs::write(&undo, b"x").unwrap();
    let r = apply(&ready, &[1], set_estado("Terminado"), &undo).unwrap();
    assert_eq!(r.changed, 1);
    assert!(r.batch_id.is_none() && r.warning.is_some());
}

#[test]
fn unused_helpers_compile() {
    // `repo::get_game` mantiene la revisión usada por los lotes: cambia al editar un campo.
    let (_d, _p, ready) = fixture();
    let c = ready.lock_conn().unwrap();
    let a = repo::get_game(&c, 1).unwrap().revision;
    c.execute("UPDATE juegos SET estado='X' WHERE id=1", [])
        .unwrap();
    assert_ne!(a, repo::get_game(&c, 1).unwrap().revision);
}

// ------------------------------------------------------------------ calidad RY, rutas de red y límites

#[test]
fn fix_ry_titles_rejects_short_single_word_proposals() {
    let dir = TempDir::new().unwrap();
    let seed = "INSERT INTO juegos(titulo,ruta) VALUES
      ('RY 4142570','C:/a/R18.zip'),
      ('RY LHG x1','C:/a/Fix1.zip'),
      ('RY-300 Algo','C:/a/Real Game Title.zip');";
    let p = common::make_db_with(&dir, "t.db", seed);
    let ready = Ready::open(&p, false).unwrap();
    let pv = preview(&ready, &[1, 2, 3], BatchAction::FixRyTitles);
    let ch: Vec<(i64, &str)> = pv
        .changes
        .iter()
        .map(|c| (c.id, c.after.as_str()))
        .collect();
    assert_eq!(ch, [(3, "Real Game Title")]);
    let sk: HashMap<i64, String> = pv
        .skipped
        .iter()
        .map(|s| (s.id, s.reason.clone()))
        .collect();
    assert_eq!(sk[&1], "propuesta poco fiable");
    assert_eq!(sk[&2], "propuesta poco fiable");
}

#[test]
fn missing_paths_do_not_touch_unc_and_relative_paths() {
    let dir = TempDir::new().unwrap();
    let seed = r"INSERT INTO juegos(titulo,ruta) VALUES
      ('Unc','\\server\share\a.zip'),
      ('Unc2','//server/share/a.zip'),
      ('Rel','juegos/a.zip');";
    let p = common::make_db_with(&dir, "t.db", seed);
    let ready = Ready::open(&p, false).unwrap();
    let scan = {
        let c = ready.lock_conn().unwrap();
        scan_missing_paths(&c, &AtomicBool::new(false), &mut |_| {}).unwrap()
    };
    let got: Vec<(i64, String)> = scan
        .items
        .iter()
        .map(|i| (i.id, i.detail.clone().unwrap()))
        .collect();
    assert_eq!(
        got,
        [
            (1, "ruta de red (no comprobada)".to_string()),
            (2, "ruta de red (no comprobada)".to_string()),
            (3, "ruta relativa (no comprobada)".to_string()),
        ]
    );
}

#[test]
fn batch_input_limits_are_validated() {
    let (d, _p, ready) = fixture();
    let undo = d.path().join("undo");
    let bad: Vec<Vec<i64>> = vec![(1..=5001).collect(), vec![], vec![0], vec![1, -3]];
    for ids in bad {
        let req = BatchRequest {
            ids: ids.clone(),
            action: set_estado("Terminado"),
        };
        let c = ready.lock_conn().unwrap();
        assert_eq!(
            preview_batch(&c, &req).unwrap_err().kind(),
            "InvalidInput",
            "{}",
            ids.len()
        );
        drop(c);
        assert_eq!(
            apply_batch(&ready, &req, &HashMap::new(), Some(&undo))
                .unwrap_err()
                .kind(),
            "InvalidInput"
        );
    }
    // 5000 ids es el máximo aceptado.
    let ok: Vec<i64> = (1..=5000).collect();
    let c = ready.lock_conn().unwrap();
    assert!(preview_batch(
        &c,
        &BatchRequest {
            ids: ok,
            action: set_estado("Terminado")
        }
    )
    .is_ok());
}

#[test]
fn list_limit_and_offset_out_of_range_are_rejected() {
    let (_d, _p, ready) = fixture();
    let c = ready.lock_conn().unwrap();
    for (off, lim) in [(0, -1), (0, 501), (-5, 10)] {
        let e = list(
            &c,
            &ListRequest {
                check: CheckId::TituloRy,
                offset: off,
                limit: lim,
            },
        )
        .unwrap_err();
        assert_eq!(e.kind(), "InvalidInput", "{off} {lim}");
    }
}

#[test]
fn batch_is_atomic_when_a_later_write_fails() {
    let (d, _p, ready) = fixture();
    let undo = d.path().join("undo");
    let before = qs(
        &ready,
        "SELECT group_concat(estado,'|') FROM (SELECT estado FROM juegos ORDER BY id)",
    );
    ready
        .lock_conn()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER boom BEFORE UPDATE OF estado ON juegos WHEN NEW.id=3 \
             BEGIN SELECT RAISE(ABORT,'boom'); END;",
        )
        .unwrap();
    let res = apply(&ready, &[1, 2, 3, 4], set_estado("Terminado"), &undo);
    assert!(res.is_err());
    let after = qs(
        &ready,
        "SELECT group_concat(estado,'|') FROM (SELECT estado FROM juegos ORDER BY id)",
    );
    assert_eq!(before, after, "no debe quedar nada escrito");
    assert!(list_batches(&ready, &undo).is_empty());
}

#[test]
fn undo_of_fix_ry_titles_restores_old_titles() {
    let dir = TempDir::new().unwrap();
    let seed = "INSERT INTO juegos(titulo,ruta) VALUES
      ('RY-100 Foo','C:/a/[F] RY-100 Foo Bar_EN.zip'),
      ('ry_104 x','C:/a/RY-104 Quest Game.zip');";
    let p = common::make_db_with(&dir, "t.db", seed);
    let ready = Ready::open(&p, false).unwrap();
    let undo = dir.path().join("undo");
    let r = apply(&ready, &[1, 2], BatchAction::FixRyTitles, &undo).unwrap();
    assert_eq!(r.changed, 2);
    assert_eq!(
        qs(&ready, "SELECT titulo FROM juegos WHERE id=1"),
        "Foo Bar"
    );
    let u = undo_last_batch(&ready, &undo).unwrap();
    assert_eq!(u.restored, 2);
    assert_eq!(
        qs(&ready, "SELECT titulo FROM juegos WHERE id=1"),
        "RY-100 Foo"
    );
    assert_eq!(
        qs(&ready, "SELECT titulo FROM juegos WHERE id=2"),
        "ry_104 x"
    );
}
