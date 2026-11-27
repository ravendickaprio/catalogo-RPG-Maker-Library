//! Read-only queries. All SQL is built from fixed fragments; user input only via parameters.
use crate::error::AppError;
use crate::models::*;
use crate::search;
use base64::Engine;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use std::time::Instant;

const SUMMARY_COLS: &str =
    "j.id, coalesce(j.titulo,''), coalesce(j.codigo,''), coalesce(j.estado,''), \
coalesce(j.idioma,''), coalesce(j.tipo,''), coalesce(j.pagina,''), coalesce(j.puntuacion,0), \
(coalesce(length(j.imagen),0) > 0)";

/// Elegible para sugerencia: estado distinto de 'Terminado' (ignora mayúsculas y espacios/tab/CR/LF). Ver ADR-005.
const ELIGIBLE: &str =
    "lower(trim(coalesce(estado,''), ' ' || char(9) || char(10) || char(13))) <> 'terminado'";

fn table_exists(conn: &Connection, name: &str) -> Result<bool, AppError> {
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type IN ('table','view') AND name = ?",
        [name],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Returns (fts_available, fts_in_sync).
pub fn detect_fts(conn: &Connection) -> Result<(bool, bool), AppError> {
    let available = table_exists(conn, "juegos_fts")? && table_exists(conn, "juegos_busqueda")?;
    if !available {
        return Ok((false, false));
    }
    let j: i64 = conn.query_row("SELECT count(*) FROM juegos", [], |r| r.get(0))?;
    let b: i64 = conn.query_row("SELECT count(*) FROM juegos_busqueda", [], |r| r.get(0))?;
    Ok((true, j == b))
}

fn counts(conn: &Connection, sql: &str) -> Result<Vec<CountItem>, AppError> {
    let mut st = conn.prepare(sql)?;
    let rows = st.query_map([], |r| {
        Ok(CountItem {
            value: r.get(0)?,
            count: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn library_info(conn: &Connection, db_file_name: &str) -> Result<LibraryInfo, AppError> {
    let (fts_available, fts_in_sync) = detect_fts(conn)?;
    let total: i64 = conn.query_row("SELECT count(*) FROM juegos", [], |r| r.get(0))?;
    let estados = counts(
        conn,
        "SELECT coalesce(estado,''), count(*) AS c FROM juegos GROUP BY 1 ORDER BY c DESC, 1",
    )?;
    let idiomas = counts(
        conn,
        "SELECT coalesce(idioma,''), count(*) AS c FROM juegos GROUP BY 1 ORDER BY c DESC, 1",
    )?;
    let sqlite_version: String = conn.query_row("SELECT sqlite_version()", [], |r| r.get(0))?;
    Ok(LibraryInfo {
        db_file_name: db_file_name.to_string(),
        read_only: true,
        total,
        fts_available,
        fts_in_sync,
        estados,
        idiomas,
        sqlite_version,
    })
}

fn summary_from_row(r: &Row) -> rusqlite::Result<GameSummary> {
    Ok(GameSummary {
        id: r.get(0)?,
        titulo: r.get(1)?,
        codigo: r.get(2)?,
        estado: r.get(3)?,
        idioma: r.get(4)?,
        tipo: r.get(5)?,
        pagina: r.get(6)?,
        puntuacion: r.get(7)?,
        has_cover: r.get(8)?,
    })
}

fn order_by(sort: SortKey) -> &'static str {
    match sort {
        SortKey::TitleAsc => "j.titulo COLLATE NOCASE, j.id",
        SortKey::TitleDesc => "j.titulo COLLATE NOCASE DESC, j.id DESC",
        SortKey::Code => {
            "(trim(coalesce(j.codigo,'')) = ''), j.codigo COLLATE NOCASE, j.titulo COLLATE NOCASE, j.id"
        }
        SortKey::Status => {
            "CASE j.estado WHEN 'Sin empezar' THEN 0 WHEN 'Empezado' THEN 1 WHEN 'Terminado' THEN 2 ELSE 3 END, j.titulo COLLATE NOCASE, j.id"
        }
        SortKey::Language => "j.idioma COLLATE NOCASE, j.titulo COLLATE NOCASE, j.id",
        SortKey::Recent => "j.id DESC",
    }
}

pub fn search(
    conn: &Connection,
    req: &SearchRequest,
    fts_available: bool,
) -> Result<SearchPage, AppError> {
    let query = req.query.trim();
    if query.chars().count() > 200 {
        return Err(AppError::InvalidInput(
            "La consulta supera los 200 caracteres.".into(),
        ));
    }
    if !(1..=200).contains(&req.limit) {
        return Err(AppError::InvalidInput(
            "limit debe estar entre 1 y 200.".into(),
        ));
    }
    if req.offset < 0 {
        return Err(AppError::InvalidInput("offset debe ser >= 0.".into()));
    }

    let start = Instant::now();
    let mut conds: Vec<&'static str> = Vec::new();
    let mut params_v: Vec<Value> = Vec::new();
    let strategy: &'static str;

    if query.is_empty() {
        strategy = "none";
    } else if let (Some(m), true) = (search::build_match(query), fts_available) {
        strategy = "fts";
        conds.push("j.id IN (SELECT rowid FROM juegos_fts WHERE juegos_fts MATCH ?)");
        params_v.push(Value::Text(m));
    } else {
        strategy = "like";
        let pat = search::like_pattern(query);
        if table_exists(conn, "juegos_busqueda")? {
            conds.push(
                "j.id IN (SELECT b.juego_id FROM juegos_busqueda b WHERE \
                 b.titulo LIKE ? ESCAPE '\\' OR b.codigo LIKE ? ESCAPE '\\' OR b.codigo_norm LIKE ? ESCAPE '\\' \
                 OR b.descripcion LIKE ? ESCAPE '\\' OR b.pagina LIKE ? ESCAPE '\\' OR b.tipo LIKE ? ESCAPE '\\' \
                 OR b.tags LIKE ? ESCAPE '\\')",
            );
            let norm = search::normalize_code(query);
            let norm_pat = if norm.is_empty() {
                pat.clone()
            } else {
                search::like_pattern(&norm)
            };
            for p in [&pat, &pat, &norm_pat, &pat, &pat, &pat, &pat] {
                params_v.push(Value::Text(p.clone()));
            }
        } else {
            conds.push(
                "(j.titulo LIKE ? ESCAPE '\\' OR j.codigo LIKE ? ESCAPE '\\' OR j.descripcion LIKE ? ESCAPE '\\' \
                 OR j.pagina LIKE ? ESCAPE '\\' OR j.tipo LIKE ? ESCAPE '\\')",
            );
            for _ in 0..5 {
                params_v.push(Value::Text(pat.clone()));
            }
        }
    }
    if let Some(e) = &req.estado {
        conds.push("j.estado = ?");
        params_v.push(Value::Text(e.clone()));
    }
    if let Some(i) = &req.idioma {
        conds.push("j.idioma = ?");
        params_v.push(Value::Text(i.clone()));
    }
    let where_sql = if conds.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conds.join(" AND "))
    };

    let count_sql = format!("SELECT count(*) FROM juegos j{where_sql}");
    let total: i64 = conn.query_row(&count_sql, params_from_iter(params_v.iter()), |r| r.get(0))?;

    let page_sql = format!(
        "SELECT {SUMMARY_COLS} FROM juegos j{where_sql} ORDER BY {} LIMIT ? OFFSET ?",
        order_by(req.sort)
    );
    params_v.push(Value::Integer(req.limit));
    params_v.push(Value::Integer(req.offset));
    let mut st = conn.prepare(&page_sql)?;
    let items = st
        .query_map(params_from_iter(params_v.iter()), summary_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SearchPage {
        items,
        total,
        offset: req.offset,
        limit: req.limit,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
        strategy,
    })
}

pub fn get_game(conn: &Connection, id: i64) -> Result<GameDetail, AppError> {
    if id <= 0 {
        return Err(AppError::InvalidInput("id debe ser > 0.".into()));
    }
    let sql = format!(
        "SELECT {SUMMARY_COLS}, coalesce(j.descripcion,''), coalesce(j.ruta,'') FROM juegos j WHERE j.id = ?"
    );
    let row = conn
        .query_row(&sql, [id], |r| {
            Ok((
                summary_from_row(r)?,
                r.get::<_, String>(9)?,
                r.get::<_, String>(10)?,
            ))
        })
        .optional()?;
    let (summary, descripcion, ruta) =
        row.ok_or_else(|| AppError::NotFound(format!("No existe el juego con id {id}.")))?;
    let mut st = conn.prepare(
        "SELECT t.nombre FROM juego_tags jt JOIN tags t ON t.id = jt.tag_id \
         WHERE jt.juego_id = ? ORDER BY t.nombre COLLATE NOCASE",
    )?;
    let tags = st
        .query_map([id], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(GameDetail {
        summary,
        descripcion,
        ruta,
        tags,
    })
}

fn mime_of(b: &[u8]) -> &'static str {
    if b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        "image/webp"
    } else if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if b.starts_with(b"GIF8") {
        "image/gif"
    } else {
        "application/octet-stream"
    }
}

pub fn get_cover(conn: &Connection, id: i64) -> Result<Option<Cover>, AppError> {
    if id <= 0 {
        return Err(AppError::InvalidInput("id debe ser > 0.".into()));
    }
    let blob: Option<Option<Vec<u8>>> = conn
        .query_row("SELECT imagen FROM juegos WHERE id = ?", [id], |r| r.get(0))
        .optional()?;
    match blob {
        None => Err(AppError::NotFound(format!(
            "No existe el juego con id {id}."
        ))),
        Some(None) => Ok(None),
        Some(Some(b)) if b.is_empty() => Ok(None),
        Some(Some(b)) => Ok(Some(Cover {
            mime: mime_of(&b).to_string(),
            data_base64: base64::engine::general_purpose::STANDARD.encode(&b),
        })),
    }
}

pub fn suggest(conn: &Connection, req: &SuggestRequest) -> Result<Suggestion, AppError> {
    let total_count: i64 = conn.query_row("SELECT count(*) FROM juegos", [], |r| r.get(0))?;
    let eligible_count: i64 = conn.query_row(
        &format!("SELECT count(*) FROM juegos WHERE {ELIGIBLE}"),
        [],
        |r| r.get(0),
    )?;
    if eligible_count == 0 {
        return Ok(Suggestion {
            game: None,
            eligible_count,
            total_count,
        });
    }
    let id: i64 = match req.exclude_id {
        Some(ex) if eligible_count > 1 => conn.query_row(
            &format!(
                "SELECT id FROM juegos WHERE {ELIGIBLE} AND id <> ? ORDER BY random() LIMIT 1"
            ),
            params![ex],
            |r| r.get(0),
        )?,
        _ => conn.query_row(
            &format!("SELECT id FROM juegos WHERE {ELIGIBLE} ORDER BY random() LIMIT 1"),
            [],
            |r| r.get(0),
        )?,
    };
    Ok(Suggestion {
        game: Some(get_game(conn, id)?),
        eligible_count,
        total_count,
    })
}
