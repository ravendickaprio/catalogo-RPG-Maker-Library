//! Escrituras del catálogo (Fase 4): validación, normalización NFC, CRUD transaccional con
//! control de concurrencia optimista (`revision`), vocabulario e integridad.
//! Semántica compatible con el legacy v04 (`guardar_tags`, `crear`, `actualizar`, `eliminar`).
use crate::error::AppError;
use crate::models::*;
use crate::repo;
use rusqlite::{params, Connection, OptionalExtension};
use unicode_normalization::UnicodeNormalization;

pub const ESTADOS: [&str; 3] = ["Sin empezar", "Empezado", "Terminado"];
pub const IDIOMAS: [&str; 3] = ["Español", "Inglés", "Japonés"];
pub const PAGINAS: [&str; 8] = [
    "Elonichan",
    "Estahi0",
    "lewdzone",
    "M0RETranslations",
    "Otome",
    "ryuugames",
    "Shorts",
    "Youtube",
];
pub const TIPOS: [&str; 5] = ["Visual Novel", "RPG", "Renpy", "3d", "Otro"];

fn invalid(msg: String) -> AppError {
    AppError::InvalidInput(msg)
}

fn nfc(s: &str) -> String {
    s.nfc().collect()
}

fn check_text(
    name: &str,
    raw: &str,
    trim: bool,
    min: usize,
    max: usize,
) -> Result<String, AppError> {
    if raw.contains('\0') {
        return Err(invalid(format!("{name} contiene caracteres no válidos.")));
    }
    let n = nfc(raw);
    let v = if trim { n.trim().to_string() } else { n };
    let len = v.chars().count();
    if len < min {
        return Err(invalid(format!("{name} no puede estar vacío.")));
    }
    if len > max {
        return Err(invalid(format!("{name} supera los {max} caracteres.")));
    }
    Ok(v)
}

/// Validates and normalizes (NFC, trims, `tipo ''` -> `Otro`, tags deduplicated NOCASE keeping the
/// first spelling). Unknown estado/idioma/tipo values are allowed (never coerced).
pub fn validate_input(i: &GameInput) -> Result<GameInput, AppError> {
    let titulo = check_text("El título", &i.titulo, true, 1, 300)?;
    let descripcion = check_text("La descripción", &i.descripcion, true, 0, 20_000)?;
    let estado = check_text("El estado", &i.estado, true, 1, 50)?;
    let idioma = check_text("El idioma", &i.idioma, true, 1, 50)?;
    let ruta = check_text("La ruta", &i.ruta, true, 0, 1000)?;
    let codigo = check_text("El código", &i.codigo, true, 0, 100)?;
    let pagina = check_text("La página", &i.pagina, true, 0, 100)?;
    let mut tipo = check_text("El tipo", &i.tipo, true, 0, 50)?;
    if tipo.is_empty() {
        tipo = "Otro".into();
    }
    let p = i.puntuacion;
    if !p.is_finite() || !(0.0..=5.0).contains(&p) || (p * 2.0).fract() != 0.0 {
        return Err(invalid(
            "La puntuación debe estar entre 0 y 5 en pasos de 0,5.".into(),
        ));
    }
    let mut tags: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = Default::default();
    for t in &i.tags {
        let t = check_text("Un tag", t, true, 0, 100)?;
        if t.is_empty() {
            continue;
        }
        // SQLite NOCASE folds ASCII only; use the same rule so UNIQUE(nombre) agrees with us.
        if seen.insert(t.to_ascii_lowercase()) {
            tags.push(t);
        }
    }
    if tags.len() > 50 {
        return Err(invalid("Un juego admite como máximo 50 tags.".into()));
    }
    Ok(GameInput {
        titulo,
        descripcion,
        estado,
        idioma,
        ruta,
        codigo,
        pagina,
        tipo,
        puntuacion: p,
        tags,
    })
}

/// SQL expression identical to the `codigo_norm` computed by the DB triggers.
fn norm_expr(x: &str) -> String {
    let mut e = format!("LOWER(COALESCE({x}, ''))");
    for c in [32, 9, 10, 13, 11, 12, 45, 95, 160] {
        e = format!("REPLACE({e}, char({c}), '')");
    }
    e
}

/// Title of another game whose normalized code equals the normalized `codigo`.
pub fn duplicate_code_title(
    conn: &Connection,
    codigo: &str,
    exclude_id: i64,
) -> Result<Option<String>, AppError> {
    let norm: String = conn.query_row(&format!("SELECT {}", norm_expr("?1")), [codigo], |r| {
        r.get(0)
    })?;
    if norm.is_empty() {
        return Ok(None);
    }
    let sql = if repo::table_exists(conn, "juegos_busqueda")? {
        "SELECT j.titulo FROM juegos_busqueda b JOIN juegos j ON j.id = b.juego_id \
         WHERE b.codigo_norm = ?1 AND b.juego_id <> ?2 LIMIT 1"
            .to_string()
    } else {
        format!(
            "SELECT titulo FROM juegos WHERE {} = ?1 AND id <> ?2 LIMIT 1",
            norm_expr("codigo")
        )
    };
    Ok(conn
        .query_row(&sql, params![norm, exclude_id], |r| r.get::<_, String>(0))
        .optional()?)
}

fn check_duplicate(
    conn: &Connection,
    codigo: &str,
    exclude_id: i64,
    opts: &WriteOptions,
) -> Result<(), AppError> {
    if opts.allow_duplicate_code {
        return Ok(());
    }
    if let Some(t) = duplicate_code_title(conn, codigo, exclude_id)? {
        return Err(AppError::DuplicateCode(format!(
            "El código «{codigo}» ya lo tiene «{t}»."
        )));
    }
    Ok(())
}

/// `BEGIN IMMEDIATE` ... `COMMIT`; rollback on any error.
pub fn in_tx<T>(conn: &Connection, f: impl FnOnce() -> Result<T, AppError>) -> Result<T, AppError> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    match f() {
        Ok(v) => match conn.execute_batch("COMMIT") {
            Ok(()) => Ok(v),
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e.into())
            }
        },
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// Legacy `guardar_tags`: replace the links of the game.
fn save_tags(conn: &Connection, id: i64, tags: &[String]) -> Result<(), AppError> {
    conn.execute("DELETE FROM juego_tags WHERE juego_id = ?", [id])?;
    for t in tags {
        conn.execute("INSERT OR IGNORE INTO tags(nombre) VALUES (?)", [t])?;
        let tag_id: i64 = conn.query_row(
            "SELECT id FROM tags WHERE nombre = ? COLLATE NOCASE",
            [t],
            |r| r.get(0),
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO juego_tags(juego_id, tag_id) VALUES (?, ?)",
            params![id, tag_id],
        )?;
    }
    Ok(())
}

fn require_revision(opts: &WriteOptions) -> Result<&str, AppError> {
    match opts.expected_revision.as_deref() {
        Some(r) if !r.is_empty() => Ok(r),
        _ => Err(invalid(
            "Falta expectedRevision (obligatorio al modificar o eliminar).".into(),
        )),
    }
}

fn check_revision(conn: &Connection, id: i64, expected: &str) -> Result<(), AppError> {
    let cur = repo::get_game(conn, id)?; // NotFound if missing
    if cur.revision != expected {
        return Err(AppError::Conflict(
            "El juego fue modificado por otra aplicación desde que se abrió. Recargue y reintente."
                .into(),
        ));
    }
    Ok(())
}

pub fn create_game(
    conn: &Connection,
    input: &GameInput,
    opts: &WriteOptions,
) -> Result<GameDetail, AppError> {
    let i = validate_input(input)?;
    in_tx(conn, || {
        check_duplicate(conn, &i.codigo, -1, opts)?;
        conn.execute(
            "INSERT INTO juegos (titulo, descripcion, estado, idioma, ruta, codigo, pagina, tipo, puntuacion) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![i.titulo, i.descripcion, i.estado, i.idioma, i.ruta, i.codigo, i.pagina, i.tipo, i.puntuacion],
        )?;
        let id = conn.last_insert_rowid();
        save_tags(conn, id, &i.tags)?;
        repo::get_game(conn, id)
    })
}

/// Does NOT touch `imagen`.
pub fn update_game(
    conn: &Connection,
    id: i64,
    input: &GameInput,
    opts: &WriteOptions,
) -> Result<GameDetail, AppError> {
    if id <= 0 {
        return Err(invalid("id debe ser > 0.".into()));
    }
    let i = validate_input(input)?;
    let expected = require_revision(opts)?;
    in_tx(conn, || {
        check_revision(conn, id, expected)?;
        check_duplicate(conn, &i.codigo, id, opts)?;
        conn.execute(
            "UPDATE juegos SET titulo=?, descripcion=?, estado=?, idioma=?, ruta=?, codigo=?, \
             pagina=?, tipo=?, puntuacion=? WHERE id=?",
            params![
                i.titulo,
                i.descripcion,
                i.estado,
                i.idioma,
                i.ruta,
                i.codigo,
                i.pagina,
                i.tipo,
                i.puntuacion,
                id
            ],
        )?;
        save_tags(conn, id, &i.tags)?;
        repo::get_game(conn, id)
    })
}

pub fn delete_game(conn: &Connection, id: i64, opts: &WriteOptions) -> Result<(), AppError> {
    if id <= 0 {
        return Err(invalid("id debe ser > 0.".into()));
    }
    let expected = require_revision(opts)?;
    in_tx(conn, || {
        check_revision(conn, id, expected)?;
        conn.execute("DELETE FROM juego_tags WHERE juego_id = ?", [id])?;
        conn.execute("DELETE FROM juegos WHERE id = ?", [id])?;
        Ok(())
    })
}

fn distinct(conn: &Connection, sql: &str) -> Result<Vec<String>, AppError> {
    let mut st = conn.prepare(sql)?;
    let rows = st
        .query_map([], |r| r.get::<_, Option<String>>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().flatten().collect())
}

/// Legacy list first (legacy order), then values found in the DB that are not in it, sorted.
fn merge_vocab(legacy: &[&str], found: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = legacy.iter().map(|s| s.to_string()).collect();
    let mut extra: Vec<String> = found
        .into_iter()
        .filter(|v| !v.trim().is_empty() && !out.contains(v))
        .collect();
    extra.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });
    extra.dedup();
    out.extend(extra);
    out
}

pub fn vocabulary(conn: &Connection) -> Result<Vocabulary, AppError> {
    // pagina/tipo live after the BLOB in `juegos`; juegos_busqueda has the same values without it.
    let (pag_sql, tipo_sql) = if repo::table_exists(conn, "juegos_busqueda")? {
        (
            "SELECT DISTINCT pagina FROM juegos_busqueda",
            "SELECT DISTINCT tipo FROM juegos_busqueda",
        )
    } else {
        (
            "SELECT DISTINCT pagina FROM juegos",
            "SELECT DISTINCT tipo FROM juegos",
        )
    };
    Ok(Vocabulary {
        estados: merge_vocab(
            &ESTADOS,
            distinct(conn, "SELECT DISTINCT estado FROM juegos")?,
        ),
        idiomas: merge_vocab(
            &IDIOMAS,
            distinct(conn, "SELECT DISTINCT idioma FROM juegos")?,
        ),
        paginas: merge_vocab(&PAGINAS, distinct(conn, pag_sql)?),
        tipos: merge_vocab(&TIPOS, distinct(conn, tipo_sql)?),
        puntuaciones: (0..=10).map(|x| x as f64 / 2.0).collect(),
    })
}

/// `PRAGMA integrity_check` plus the FTS5 `integrity-check` against the content table.
pub fn check_integrity(conn: &Connection) -> Result<(), AppError> {
    let ok: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if ok != "ok" {
        return Err(AppError::Database(format!("integrity_check: {ok}")));
    }
    if repo::table_exists(conn, "juegos_fts")? {
        conn.execute(
            "INSERT INTO juegos_fts(juegos_fts, rank) VALUES('integrity-check', 1)",
            [],
        )
        .map_err(|e| AppError::Database(format!("FTS integrity-check: {e}")))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> GameInput {
        GameInput {
            titulo: "  Hola ".into(),
            descripcion: " d\n".into(),
            estado: "Sin empezar".into(),
            idioma: "Español".into(),
            ruta: "".into(),
            codigo: " RY-1 ".into(),
            pagina: "".into(),
            tipo: "".into(),
            puntuacion: 2.5,
            tags: vec!["A".into(), " a ".into(), "".into(), "b".into()],
        }
    }

    #[test]
    fn normalizes() {
        let v = validate_input(&base()).unwrap();
        assert_eq!(v.titulo, "Hola");
        assert_eq!(v.descripcion, "d");
        assert_eq!(v.codigo, "RY-1");
        assert_eq!(v.tipo, "Otro");
        assert_eq!(v.tags, vec!["A", "b"]);
    }

    #[test]
    fn nfc_applied() {
        let mut i = base();
        i.titulo = "Cafe\u{301}".into();
        assert_eq!(validate_input(&i).unwrap().titulo, "Caf\u{e9}");
    }

    #[test]
    fn rejects() {
        type Mutator = Box<dyn Fn(&mut GameInput)>;
        let bad: Vec<Mutator> = vec![
            Box::new(|i| i.titulo = "  ".into()),
            Box::new(|i| i.titulo = "x".repeat(301)),
            Box::new(|i| i.descripcion = "x".repeat(20_001)),
            Box::new(|i| i.estado = "".into()),
            Box::new(|i| i.idioma = "x".repeat(51)),
            Box::new(|i| i.ruta = "x".repeat(1001)),
            Box::new(|i| i.codigo = "x".repeat(101)),
            Box::new(|i| i.pagina = "x".repeat(101)),
            Box::new(|i| i.tipo = "x".repeat(51)),
            Box::new(|i| i.puntuacion = 5.5),
            Box::new(|i| i.puntuacion = -0.5),
            Box::new(|i| i.puntuacion = 1.3),
            Box::new(|i| i.puntuacion = f64::NAN),
            Box::new(|i| i.tags = vec!["x".repeat(101)]),
            Box::new(|i| i.tags = (0..51).map(|n| format!("t{n}")).collect()),
            Box::new(|i| i.titulo = "a\0b".into()),
        ];
        for (n, f) in bad.iter().enumerate() {
            let mut i = base();
            f(&mut i);
            assert!(
                matches!(validate_input(&i), Err(AppError::InvalidInput(_))),
                "case {n}"
            );
        }
        let mut ok = base();
        ok.tags = (0..50).map(|n| format!("t{n}")).collect();
        ok.puntuacion = 5.0;
        assert!(validate_input(&ok).is_ok());
    }

    #[test]
    fn vocab_merge_order() {
        let v = merge_vocab(
            &["b", "a"],
            vec!["Z".into(), "a".into(), "c".into(), "".into(), "c".into()],
        );
        assert_eq!(v, vec!["b", "a", "c", "Z"]);
    }

    #[test]
    fn norm_expr_matches_rust() {
        let c = Connection::open_in_memory().unwrap();
        let n: String = c
            .query_row(
                &format!("SELECT {}", norm_expr("?1")),
                ["RY-10_00 \u{A0}x"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, crate::search::normalize_code("RY-10_00 \u{A0}x"));
    }
}
