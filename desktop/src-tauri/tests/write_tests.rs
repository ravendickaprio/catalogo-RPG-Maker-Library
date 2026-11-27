//! Pruebas de escritura (Fase 4) sobre DB temporales con el DDL real (fixtures/schema.sql).
use rpg_library_lib::commands::{run_migrations_sync, Ready};
use rpg_library_lib::error::AppError;
use rpg_library_lib::models::*;
use rpg_library_lib::{db, migrate, repo, write};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

mod common;

fn make_db(dir: &TempDir) -> PathBuf {
    common::make_db_with(
        dir,
        "t.db",
        "INSERT INTO juegos(titulo,codigo,estado,idioma,tipo,pagina,descripcion,imagen) VALUES
           ('Zeta Quest','RY-1000','Sin empezar','Japonés','RPG','ryuugames','épica', X'52494646040000005745425056503820'),
           ('Alpha Tale','RY-1001','Raro estado','Inglés','Tipo Raro','OtraPagina','',NULL),
           ('Beta Land','','Terminado','Inglés','RPG','ryuugames','',NULL);
         INSERT INTO tags(nombre) VALUES ('rpg'),('Fantasía');
         INSERT INTO juego_tags(juego_id,tag_id) VALUES (1,1),(1,2);",
    )
}

fn input(titulo: &str, codigo: &str) -> GameInput {
    GameInput {
        titulo: titulo.into(),
        descripcion: "desc".into(),
        estado: "Sin empezar".into(),
        idioma: "Español".into(),
        ruta: "C:/x".into(),
        codigo: codigo.into(),
        pagina: "ryuugames".into(),
        tipo: "RPG".into(),
        puntuacion: 3.5,
        tags: vec!["nuevo".into()],
    }
}

fn opts(rev: &str) -> WriteOptions {
    WriteOptions {
        allow_duplicate_code: false,
        expected_revision: Some(rev.into()),
    }
}

fn rw(path: &Path) -> Connection {
    db::open_read_write(path).unwrap()
}

fn count(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn assert_consistent(c: &Connection) {
    write::check_integrity(c).unwrap();
    assert_eq!(
        count(c, "SELECT count(*) FROM juegos"),
        count(c, "SELECT count(*) FROM juegos_busqueda")
    );
    assert_eq!(
        count(c, "SELECT count(*) FROM juegos j JOIN juegos_busqueda b ON b.juego_id=j.id WHERE b.titulo <> j.titulo"),
        0
    );
}

fn search(c: &Connection, q: &str) -> Vec<String> {
    let r = SearchRequest {
        query: q.into(),
        estado: None,
        idioma: None,
        tag: None,
        sort: SortKey::TitleAsc,
        offset: 0,
        limit: 50,
    };
    repo::search(c, &r, true)
        .unwrap()
        .items
        .into_iter()
        .map(|g| g.titulo)
        .collect()
}

fn backups(dir: &TempDir) -> Vec<PathBuf> {
    let d = dir.path().join("backups");
    if !d.is_dir() {
        return vec![];
    }
    let mut v: Vec<PathBuf> = std::fs::read_dir(d)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    v.sort();
    v
}

#[test]
fn crud_happy_path_keeps_search_in_sync() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let c = rw(&p);

    let g =
        write::create_game(&c, &input("Nuevo Titulo", "NT-1"), &WriteOptions::default()).unwrap();
    assert_eq!(g.summary.tipo, "RPG");
    assert_eq!(g.tags, vec!["nuevo"]);
    assert!(!g.summary.has_cover);
    assert_consistent(&c);
    assert_eq!(search(&c, "nuevo titu"), vec!["Nuevo Titulo"]);
    assert_eq!(
        search(&c, "nt1"),
        vec!["Nuevo Titulo"],
        "code search by codigo_norm"
    );
    assert_eq!(search(&c, "nuevo").len(), 1, "tag text is searchable");

    let mut inp = input("Titulo Editado", "NT-1");
    inp.tags = vec!["otro".into(), "Fantasía".into()];
    inp.puntuacion = 4.5;
    let u = write::update_game(&c, g.summary.id, &inp, &opts(&g.revision)).unwrap();
    assert_eq!(u.summary.titulo, "Titulo Editado");
    assert_eq!(u.summary.puntuacion, 4.5);
    assert_eq!(u.tags, vec!["Fantasía", "otro"]);
    assert_consistent(&c);
    assert!(search(&c, "nuevo titu").is_empty());
    assert_eq!(search(&c, "editado"), vec!["Titulo Editado"]);
    assert_eq!(
        search(&c, "otro"),
        vec!["Titulo Editado"],
        "new tag found via FTS"
    );
    assert!(search(&c, "nuevo").is_empty(), "old tag no longer indexed");
    // tag 'Fantasía' reused (same id), not duplicated
    assert_eq!(
        count(&c, "SELECT count(*) FROM tags WHERE nombre='Fantasía'"),
        1
    );

    write::delete_game(&c, g.summary.id, &opts(&u.revision)).unwrap();
    assert_consistent(&c);
    assert!(search(&c, "editado").is_empty());
    assert_eq!(
        count(
            &c,
            &format!(
                "SELECT count(*) FROM juego_tags WHERE juego_id={}",
                g.summary.id
            )
        ),
        0
    );
    assert!(matches!(
        repo::get_game(&c, g.summary.id),
        Err(AppError::NotFound(_))
    ));
}

#[test]
fn nfc_is_stored() {
    let d = TempDir::new().unwrap();
    let c = rw(&make_db(&d));
    let mut i = input("Cafe\u{301} Quest", "");
    i.tags = vec!["Fantasi\u{301}a2".into()];
    let g = write::create_game(&c, &i, &WriteOptions::default()).unwrap();
    assert_eq!(g.summary.titulo, "Caf\u{e9} Quest");
    assert_eq!(g.tags, vec!["Fantas\u{ed}a2"]);
    let raw: String = c
        .query_row(
            "SELECT titulo FROM juegos WHERE id=?",
            [g.summary.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(raw.chars().count(), "Café Quest".chars().count());
    // searching with the composed form finds it through FTS
    assert_eq!(search(&c, "café"), vec!["Café Quest"]);
}

#[test]
fn validation_errors_leave_db_untouched() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let c = rw(&p);
    let n0 = count(&c, "SELECT count(*) FROM juegos");
    for f in [
        |i: &mut GameInput| i.titulo = "   ".into(),
        |i: &mut GameInput| i.puntuacion = 5.5,
        |i: &mut GameInput| i.puntuacion = 0.3,
        |i: &mut GameInput| i.estado = "".into(),
        |i: &mut GameInput| i.tags = (0..51).map(|n| format!("t{n}")).collect(),
    ] {
        let mut i = input("ok", "");
        f(&mut i);
        assert!(matches!(
            write::create_game(&c, &i, &WriteOptions::default()),
            Err(AppError::InvalidInput(_))
        ));
    }
    assert_eq!(count(&c, "SELECT count(*) FROM juegos"), n0);
    assert_eq!(count(&c, "SELECT count(*) FROM tags"), 2);
}

#[test]
fn unknown_values_preserved_and_tipo_defaults_to_otro() {
    let d = TempDir::new().unwrap();
    let c = rw(&make_db(&d));
    let cur = repo::get_game(&c, 2).unwrap();
    assert_eq!(cur.summary.estado, "Raro estado");
    let mut i = input("Alpha Tale", "RY-1001");
    i.estado = "Raro estado".into();
    i.idioma = "Klingon".into();
    i.tipo = "Tipo Raro".into();
    i.pagina = "OtraPagina".into();
    let u = write::update_game(&c, 2, &i, &opts(&cur.revision)).unwrap();
    assert_eq!(u.summary.estado, "Raro estado");
    assert_eq!(u.summary.idioma, "Klingon");
    assert_eq!(u.summary.tipo, "Tipo Raro");
    let mut i2 = i.clone();
    i2.tipo = "  ".into();
    let u2 = write::update_game(&c, 2, &i2, &opts(&u.revision)).unwrap();
    assert_eq!(u2.summary.tipo, "Otro");
    // vocabulary keeps legacy order first, then DB extras sorted
    let v = write::vocabulary(&c).unwrap();
    assert_eq!(&v.estados[..3], ["Sin empezar", "Empezado", "Terminado"]);
    assert!(v.estados.contains(&"Raro estado".to_string()));
    assert_eq!(v.idiomas[..3], ["Español", "Inglés", "Japonés"]);
    assert!(v.idiomas.contains(&"Klingon".to_string()));
    assert!(v.paginas.contains(&"OtraPagina".to_string()));
    assert_eq!(v.puntuaciones.len(), 11);
    assert_eq!(v.puntuaciones[10], 5.0);
}

#[test]
fn update_does_not_touch_imagen() {
    let d = TempDir::new().unwrap();
    let c = rw(&make_db(&d));
    let before: Vec<u8> = c
        .query_row("SELECT imagen FROM juegos WHERE id=1", [], |r| r.get(0))
        .unwrap();
    let cur = repo::get_game(&c, 1).unwrap();
    let u = write::update_game(
        &c,
        1,
        &input("Zeta Quest 2", "RY-1000"),
        &opts(&cur.revision),
    )
    .unwrap();
    assert!(u.summary.has_cover);
    let after: Vec<u8> = c
        .query_row("SELECT imagen FROM juegos WHERE id=1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn duplicate_code_detection() {
    let d = TempDir::new().unwrap();
    let c = rw(&make_db(&d));
    let e =
        write::create_game(&c, &input("Otro", "ry_1000"), &WriteOptions::default()).unwrap_err();
    assert!(
        matches!(&e, AppError::DuplicateCode(m) if m.contains("Zeta Quest")),
        "{e:?}"
    );
    let e =
        write::create_game(&c, &input("Otro", "RY 10-00"), &WriteOptions::default()).unwrap_err();
    assert_eq!(e.kind(), "DuplicateCode");
    // self-update keeps its own code
    let cur = repo::get_game(&c, 1).unwrap();
    write::update_game(&c, 1, &input("Zeta Quest", "ry-1000"), &opts(&cur.revision)).unwrap();
    let allow = WriteOptions {
        allow_duplicate_code: true,
        expected_revision: None,
    };
    write::create_game(&c, &input("Otro", "ry_1000"), &allow).unwrap();
    // empty code never duplicates (Beta Land has '')
    write::create_game(&c, &input("Sin codigo 1", ""), &WriteOptions::default()).unwrap();
    write::create_game(&c, &input("Sin codigo 2", "  "), &WriteOptions::default()).unwrap();
    // update to another game's code is flagged
    let cur = repo::get_game(&c, 2).unwrap();
    let e = write::update_game(&c, 2, &input("Alpha Tale", "RY1000"), &opts(&cur.revision))
        .unwrap_err();
    assert_eq!(e.kind(), "DuplicateCode");
    assert_consistent(&c);
}

#[test]
fn revision_is_deterministic_and_field_sensitive() {
    let d = TempDir::new().unwrap();
    let c = rw(&make_db(&d));
    let a = repo::get_game(&c, 1).unwrap().revision;
    assert_eq!(a, repo::get_game(&c, 1).unwrap().revision);
    assert_eq!(a.len(), 16);
    assert_ne!(a, repo::get_game(&c, 2).unwrap().revision);
    // separator safety: moving a character between adjacent fields changes the hash
    c.execute(
        "UPDATE juegos SET titulo='ab', descripcion='c' WHERE id=3",
        [],
    )
    .unwrap();
    let r1 = repo::get_game(&c, 3).unwrap().revision;
    c.execute(
        "UPDATE juegos SET titulo='a', descripcion='bc' WHERE id=3",
        [],
    )
    .unwrap();
    assert_ne!(r1, repo::get_game(&c, 3).unwrap().revision);
    // cover length participates
    let r0 = repo::get_game(&c, 3).unwrap().revision;
    c.execute("UPDATE juegos SET imagen = X'0102' WHERE id=3", [])
        .unwrap();
    let r2 = repo::get_game(&c, 3).unwrap().revision;
    assert_ne!(r0, r2);
    c.execute("UPDATE juegos SET imagen = X'0103' WHERE id=3", [])
        .unwrap();
    assert_ne!(
        r2,
        repo::get_game(&c, 3).unwrap().revision,
        "same length, different tail -> different fingerprint (Phase 5)"
    );
    c.execute("UPDATE juegos SET imagen = X'0102' WHERE id=3", [])
        .unwrap();
    assert_eq!(r2, repo::get_game(&c, 3).unwrap().revision);
    // tags participate
    c.execute("INSERT INTO juego_tags(juego_id,tag_id) VALUES (3,1)", [])
        .unwrap();
    assert_ne!(r2, repo::get_game(&c, 3).unwrap().revision);
}

#[test]
fn revision_conflicts() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let c = rw(&p);
    let cur = repo::get_game(&c, 2).unwrap();
    // another app modifies the row
    let other = Connection::open(&p).unwrap();
    other
        .execute("UPDATE juegos SET puntuacion = 1.0 WHERE id=2", [])
        .unwrap();
    drop(other);
    let e = write::update_game(&c, 2, &input("Alpha Tale", "RY-1001"), &opts(&cur.revision))
        .unwrap_err();
    assert_eq!(e.kind(), "Conflict");
    let e = write::delete_game(&c, 2, &opts(&cur.revision)).unwrap_err();
    assert_eq!(e.kind(), "Conflict");
    assert!(repo::get_game(&c, 2).is_ok(), "row still there");
    // expectedRevision is mandatory
    assert_eq!(
        write::delete_game(&c, 2, &WriteOptions::default())
            .unwrap_err()
            .kind(),
        "InvalidInput"
    );
    // missing row
    assert_eq!(
        write::delete_game(&c, 999, &opts("x")).unwrap_err().kind(),
        "NotFound"
    );
    // with the fresh revision it works
    let fresh = repo::get_game(&c, 2).unwrap();
    write::update_game(
        &c,
        2,
        &input("Alpha Tale", "RY-1001"),
        &opts(&fresh.revision),
    )
    .unwrap();
}

#[test]
fn read_only_mode_rejects_writes_and_leaves_file_unchanged() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let before = std::fs::read(&p).unwrap();
    let ready = Ready::open(&p, true).unwrap();
    assert!(ready.read_only);
    let e = ready
        .with_write(|c| write::create_game(c, &input("X", ""), &WriteOptions::default()))
        .unwrap_err();
    assert_eq!(e.kind(), "ReadOnly");
    assert_eq!(run_migrations_sync(&ready).unwrap_err().kind(), "ReadOnly");
    assert_eq!(before, std::fs::read(&p).unwrap());
    assert!(backups(&d).is_empty(), "no backup in read-only mode");
    // a raw write on the read-only connection is also refused by SQLite (maps to ReadOnly)
    let c = ready.conn.lock().unwrap();
    let e: AppError = c.execute("DELETE FROM juegos", []).unwrap_err().into();
    assert!(matches!(e.kind(), "ReadOnly" | "Database"));
}

#[test]
fn open_never_creates_a_file_and_validates_schema() {
    let d = TempDir::new().unwrap();
    let missing = d.path().join("nueva.db");
    assert!(db::open_read_write(&missing).is_err());
    assert!(!missing.exists(), "must not create the database");
    assert!(Ready::open(&missing, false).is_err());
    assert!(!missing.exists());

    // wrong schema: lists what is missing and creates/alters nothing
    let bad = d.path().join("bad.db");
    {
        let c = Connection::open(&bad).unwrap();
        c.execute_batch("CREATE TABLE juegos (id INTEGER PRIMARY KEY, titulo TEXT);")
            .unwrap();
    }
    let before = std::fs::read(&bad).unwrap();
    let e = match Ready::open(&bad, false) {
        Err(e) => e,
        Ok(_) => panic!("must fail"),
    };
    assert_eq!(e.kind(), "Config");
    let m = e.to_string();
    for needle in [
        "juegos.puntuacion",
        "juegos.imagen",
        "tabla tags",
        "tabla juego_tags",
    ] {
        assert!(m.contains(needle), "{m}");
    }
    assert!(!m.contains("juegos.titulo"));
    assert_eq!(before, std::fs::read(&bad).unwrap());

    // not a database at all
    let junk = d.path().join("junk.db");
    std::fs::write(&junk, "hola").unwrap();
    assert_eq!(Ready::open(&junk, true).err().unwrap().kind(), "Config");
    assert_eq!(std::fs::read_to_string(&junk).unwrap(), "hola");

    // a good one opens in both modes (and the legacy fixture without juegos_busqueda too)
    let good = make_db(&d);
    assert!(Ready::open(&good, true).is_ok());
    assert!(Ready::open(&good, false).is_ok());
}

#[test]
fn auto_backup_once_per_session_before_first_write() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let ready = Ready::open(&p, false).unwrap();
    assert!(backups(&d).is_empty());
    ready
        .with_write(|c| write::create_game(c, &input("Primero", "P-1"), &WriteOptions::default()))
        .unwrap();
    let b = backups(&d);
    assert_eq!(b.len(), 1);
    let name = b[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("t.db.bak-"), "{name}");
    let stamp = &name["t.db.bak-".len()..];
    assert_eq!(stamp.len(), 15);
    assert_eq!(&stamp[8..9], "-");
    // the backup holds the PRE-write state and is a healthy database
    let bc = Connection::open(&b[0]).unwrap();
    let ok: String = bc
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ok, "ok");
    assert_eq!(count(&bc, "SELECT count(*) FROM juegos"), 3);
    drop(bc);
    ready
        .with_write(|c| write::create_game(c, &input("Segundo", "P-2"), &WriteOptions::default()))
        .unwrap();
    assert_eq!(
        backups(&d).len(),
        1,
        "only the first write of the session backs up"
    );
    assert_eq!(
        ready.session_info().last_backup.as_deref(),
        Some(b[0].to_string_lossy().as_ref())
    );
    // a new session (reopen) backs up again
    let ready2 = Ready::open(&p, false).unwrap();
    ready2
        .with_write(|c| write::create_game(c, &input("Tercero", "P-3"), &WriteOptions::default()))
        .unwrap();
    assert_eq!(backups(&d).len(), 2);
    // manual backup
    let m = ready2.manual_backup().unwrap();
    assert!(m.is_file());
    assert_eq!(backups(&d).len(), 3);
}

#[test]
fn backup_rotation_keeps_20_newest() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let bdir = d.path().join("backups");
    std::fs::create_dir_all(&bdir).unwrap();
    for i in 0..25 {
        std::fs::write(bdir.join(format!("t.db.bak-20200101-0000{i:02}")), "x").unwrap();
    }
    std::fs::write(bdir.join("notes.txt"), "keep").unwrap();
    std::fs::write(bdir.join("other.db.bak-20200101-000000"), "keep").unwrap();
    let ready = Ready::open(&p, false).unwrap();
    ready
        .with_write(|c| write::create_game(c, &input("R", ""), &WriteOptions::default()))
        .unwrap();
    let mine: Vec<String> = backups(&d)
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("t.db.bak-"))
        .collect();
    assert_eq!(mine.len(), 20);
    assert!(!mine.contains(&"t.db.bak-20200101-000000".to_string()));
    assert!(mine.contains(&"t.db.bak-20200101-000024".to_string()));
    assert!(bdir.join("notes.txt").exists());
    assert!(bdir.join("other.db.bak-20200101-000000").exists());
}

#[test]
fn backup_failure_aborts_the_write() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    std::fs::write(d.path().join("backups"), "soy un archivo").unwrap();
    let ready = Ready::open(&p, false).unwrap();
    let e = ready
        .with_write(|c| {
            write::create_game(
                c,
                &input("No debe existir", "N-1"),
                &WriteOptions::default(),
            )
        })
        .unwrap_err();
    assert_eq!(e.kind(), "Database");
    assert!(e.to_string().contains("Backup fallido"), "{e}");
    let c = Connection::open(&p).unwrap();
    assert_eq!(count(&c, "SELECT count(*) FROM juegos"), 3);
    assert!(!ready.backed_up.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn busy_is_mapped_and_recoverable() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let ready = Ready::open(&p, false).unwrap();
    ready
        .conn
        .lock()
        .unwrap()
        .busy_timeout(Duration::from_millis(50))
        .unwrap();
    let blocker = Connection::open(&p).unwrap();
    blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
    let e = ready
        .with_write(|c| write::create_game(c, &input("B", "B-1"), &WriteOptions::default()))
        .unwrap_err();
    assert_eq!(e.kind(), "Busy", "{e:?}");
    blocker.execute_batch("ROLLBACK").unwrap();
    // nothing was written, and the connection is usable again
    ready
        .with_write(|c| write::create_game(c, &input("B", "B-1"), &WriteOptions::default()))
        .unwrap();
    let c = Connection::open(&p).unwrap();
    assert_eq!(count(&c, "SELECT count(*) FROM juegos WHERE titulo='B'"), 1);
}

#[test]
fn own_writes_do_not_look_external() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let ready = Ready::open(&p, false).unwrap();
    let dv = |r: &Ready| -> i64 {
        r.conn
            .lock()
            .unwrap()
            .query_row("PRAGMA data_version", [], |x| x.get(0))
            .unwrap()
    };
    let v0 = dv(&ready);
    ready
        .with_write(|c| write::create_game(c, &input("Own", "O-1"), &WriteOptions::default()))
        .unwrap();
    assert_eq!(v0, dv(&ready), "same-connection commits keep data_version");
    // an external write does change it
    Connection::open(&p)
        .unwrap()
        .execute("UPDATE juegos SET puntuacion=1 WHERE id=1", [])
        .unwrap();
    assert_ne!(v0, dv(&ready));
}

#[test]
fn migrations_status_run_idempotent() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let ready = Ready::open(&p, false).unwrap();
    {
        let c = ready.conn.lock().unwrap();
        // simulate a legacy DB without app_metadata
        c.execute_batch("DROP TABLE app_metadata").unwrap();
        let st = migrate::status(&c).unwrap();
        assert_eq!((st.current, st.latest, st.pending.len()), (0, 1, 1));
        assert_eq!(st.pending[0].version, 1);
    }
    let r = run_migrations_sync(&ready).unwrap();
    assert_eq!(r.applied, vec![1]);
    assert!(r.backup_path.contains(".bak-"));
    assert!(Path::new(&r.backup_path).is_file());
    {
        let c = ready.conn.lock().unwrap();
        let st = migrate::status(&c).unwrap();
        assert_eq!((st.current, st.pending.len()), (1, 0));
        let v: String = c
            .query_row(
                "SELECT valor FROM app_metadata WHERE clave='desktop_schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(v, "1");
        assert_eq!(
            count(&c, "SELECT count(*) FROM sqlite_master WHERE type='index' AND name='ix_juegos_titulo_nocase'"),
            1
        );
        write::check_integrity(&c).unwrap();
        // title sort uses the index now
        let plan: String = c
            .query_row(
                "EXPLAIN QUERY PLAN SELECT id FROM juegos ORDER BY titulo COLLATE NOCASE LIMIT 5",
                [],
                |r| r.get(3),
            )
            .unwrap();
        assert!(plan.contains("ix_juegos_titulo_nocase"), "{plan}");
    }
    let n = backups(&d).len();
    let again = run_migrations_sync(&ready).unwrap();
    assert!(again.applied.is_empty() && again.backup_path.is_empty());
    assert_eq!(backups(&d).len(), n, "no backup when nothing is pending");
    // the legacy-like busqueda_schema row survives alongside the new key on a DB that had app_metadata
    let d2 = TempDir::new().unwrap();
    let p2 = make_db(&d2);
    let r2 = Ready::open(&p2, false).unwrap();
    run_migrations_sync(&r2).unwrap();
    let c = r2.conn.lock().unwrap();
    assert_eq!(count(&c, "SELECT count(*) FROM app_metadata"), 2);
}

#[test]
fn code_sort_matches_with_and_without_title_index() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let ready = Ready::open(&p, false).unwrap();
    let order = |c: &Connection| -> Vec<String> {
        let r = SearchRequest {
            query: "".into(),
            estado: None,
            idioma: None,
            tag: None,
            sort: SortKey::Code,
            offset: 0,
            limit: 50,
        };
        repo::search(c, &r, true)
            .unwrap()
            .items
            .into_iter()
            .map(|g| g.titulo)
            .collect()
    };
    let before = order(&ready.conn.lock().unwrap());
    run_migrations_sync(&ready).unwrap();
    let c = ready.conn.lock().unwrap();
    assert_eq!(before, order(&c));
    assert_eq!(before, vec!["Zeta Quest", "Alpha Tale", "Beta Land"]);
    let r = SearchRequest {
        query: "".into(),
        estado: None,
        idioma: None,
        tag: None,
        sort: SortKey::Code,
        offset: 0,
        limit: 10,
    };
    let q = repo::build_search_sql(&c, &r, true).unwrap();
    assert!(q.page_sql.contains("juegos j NOT INDEXED"));
}

/// Interop con el legacy: CRUD sobre una COPIA de la DB de desarrollo (se omite si no existe).
/// Con `RPGLIB_INTEROP_OUT=<archivo>` deja la copia modificada ahí para leerla con Python.
#[test]
fn interop_on_dev_copy() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dev-data/juegos.dev.db");
    if !src.is_file() {
        eprintln!("dev-data/juegos.dev.db no existe: se omite");
        return;
    }
    let d = TempDir::new().unwrap();
    let p = d.path().join("copy.db");
    std::fs::copy(&src, &p).unwrap();
    let ready = Ready::open(&p, false).unwrap();
    let total0 = count(&ready.conn.lock().unwrap(), "SELECT count(*) FROM juegos");
    let g = ready
        .with_write(|c| {
            write::create_game(c, &input("Interop Café", "IO-1"), &WriteOptions::default())
        })
        .unwrap();
    let u = ready
        .with_write(|c| {
            let mut i = input("Interop Editado", "IO-1");
            i.tags = vec!["interop".into(), "rpg".into()];
            write::update_game(c, g.summary.id, &i, &opts(&g.revision))
        })
        .unwrap();
    let g2 = ready
        .with_write(|c| {
            write::create_game(
                c,
                &input("Interop Borrar", "IO-2"),
                &WriteOptions::default(),
            )
        })
        .unwrap();
    ready
        .with_write(|c| write::delete_game(c, g2.summary.id, &opts(&g2.revision)))
        .unwrap();
    {
        let c = ready.conn.lock().unwrap();
        assert_consistent(&c);
        assert_eq!(count(&c, "SELECT count(*) FROM juegos"), total0 + 1);
        println!("interop id={} revision={}", u.summary.id, u.revision);
    }
    assert_eq!(backups(&d).len(), 1);
    drop(ready);
    if let Ok(out) = std::env::var("RPGLIB_INTEROP_OUT") {
        std::fs::copy(&p, out).unwrap();
    }
}

// ---------------------------------------------------------------- Fase 5 (portadas)

fn webp_of(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([(x * 3 % 256) as u8, (y * 5 % 256) as u8, 40])
    });
    let mut c = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut c, image::ImageFormat::Png)
        .unwrap();
    rpg_library_lib::images::process_cover(&c.into_inner()).unwrap()
}

#[test]
fn set_and_remove_cover_with_backup_and_consistency() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let ready = Ready::open(&p, false).unwrap();
    assert!(backups(&d).is_empty());
    let cur = repo::get_game(&rw(&p), 2).unwrap();
    assert!(!cur.summary.has_cover);
    let webp = webp_of(900, 600);
    let g = ready
        .with_write(|c| write::set_cover(c, 2, &webp, &opts(&cur.revision)))
        .unwrap();
    assert!(g.summary.has_cover);
    assert_ne!(g.revision, cur.revision);
    assert_eq!(
        backups(&d).len(),
        1,
        "session backup made before the first write"
    );

    let c = rw(&p);
    let blob: Vec<u8> = c
        .query_row("SELECT imagen FROM juegos WHERE id=2", [], |r| r.get(0))
        .unwrap();
    assert_eq!(blob, webp);
    assert_eq!(&blob[0..4], b"RIFF");
    let dec = image::load_from_memory_with_format(&blob, image::ImageFormat::WebP).unwrap();
    assert_eq!((dec.width(), dec.height()), (300, 200));
    // text fields and tags untouched; search and FTS in sync
    assert_eq!(g.summary.titulo, "Alpha Tale");
    assert_eq!(search(&c, "alpha"), vec!["Alpha Tale"]);
    assert_consistent(&c);

    // remove
    let g2 = ready
        .with_write(|c| write::remove_cover(c, 2, &opts(&g.revision)))
        .unwrap();
    assert!(!g2.summary.has_cover);
    assert_ne!(g2.revision, g.revision);
    assert_eq!(
        count(
            &c,
            "SELECT count(*) FROM juegos WHERE id=2 AND imagen IS NULL"
        ),
        1
    );
    assert_eq!(backups(&d).len(), 1);
    assert_consistent(&c);
}

#[test]
fn cover_revision_detects_same_length_replacement() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let c = rw(&p);
    let a = X_BLOB.to_vec();
    let mut b = a.clone();
    let last = b.len() - 1;
    b[last] ^= 0xFF; // same length, different tail
    let r0 = repo::get_game(&c, 1).unwrap().revision;
    c.execute("UPDATE juegos SET imagen=? WHERE id=1", [&a])
        .unwrap();
    let ra = repo::get_game(&c, 1).unwrap().revision;
    c.execute("UPDATE juegos SET imagen=? WHERE id=1", [&b])
        .unwrap();
    let rb = repo::get_game(&c, 1).unwrap().revision;
    assert_ne!(ra, rb);
    assert_ne!(r0, rb);
}
const X_BLOB: [u8; 40] = [7u8; 40];

#[test]
fn cover_conflict_and_readonly() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let ready = Ready::open(&p, false).unwrap();
    let webp = webp_of(50, 50);
    let before: Vec<u8> = rw(&p)
        .query_row("SELECT imagen FROM juegos WHERE id=1", [], |r| r.get(0))
        .unwrap();
    let e = ready
        .with_write(|c| write::set_cover(c, 1, &webp, &opts("stale")))
        .unwrap_err();
    assert!(matches!(e, AppError::Conflict(_)));
    let e = ready
        .with_write(|c| write::remove_cover(c, 1, &WriteOptions::default()))
        .unwrap_err();
    assert!(matches!(e, AppError::InvalidInput(_)));
    let e = ready
        .with_write(|c| write::set_cover(c, 99, &webp, &opts("x")))
        .unwrap_err();
    assert!(matches!(e, AppError::NotFound(_)));
    let after: Vec<u8> = rw(&p)
        .query_row("SELECT imagen FROM juegos WHERE id=1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after);

    let ro = Ready::open(&p, true).unwrap();
    let e = ro
        .with_write(|c| write::set_cover(c, 1, &webp, &opts("x")))
        .unwrap_err();
    assert!(matches!(e, AppError::ReadOnly(_)));
}
