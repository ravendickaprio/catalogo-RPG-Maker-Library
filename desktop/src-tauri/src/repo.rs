//! Read-only queries. All SQL is built from fixed fragments; user input only via parameters.
use crate::error::AppError;
use crate::models::*;
use crate::search;
use base64::Engine;
use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection, OptionalExtension, Row};
use std::time::Instant;

const SUMMARY_COLS: &str =
    "j.id, coalesce(j.titulo,''), coalesce(j.codigo,''), coalesce(j.estado,''), \
coalesce(j.idioma,''), coalesce(j.tipo,''), coalesce(j.pagina,''), coalesce(j.puntuacion,0), \
(coalesce(length(j.imagen),0) > 0)";

/// Elegible para sugerencia: estado distinto de 'Terminado' (ignora mayúsculas y espacios/tab/CR/LF). Ver ADR-005.
const ELIGIBLE: &str =
    "lower(trim(coalesce(estado,''), ' ' || char(9) || char(10) || char(13))) <> 'terminado'";

pub fn table_exists(conn: &Connection, name: &str) -> Result<bool, AppError> {
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

/// One scan of `juegos` (columns before `imagen` only) gives total, estados and idiomas.
pub fn library_info(
    conn: &Connection,
    db_file_name: &str,
    read_only: bool,
) -> Result<LibraryInfo, AppError> {
    let available = table_exists(conn, "juegos_fts")? && table_exists(conn, "juegos_busqueda")?;
    let mut estados_m: std::collections::HashMap<String, i64> = Default::default();
    let mut idiomas_m: std::collections::HashMap<String, i64> = Default::default();
    let mut total = 0i64;
    {
        let mut st = conn.prepare(
            "SELECT coalesce(estado,''), coalesce(idioma,''), count(*) FROM juegos GROUP BY 1, 2",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let (e, i, c): (String, String, i64) = (r.get(0)?, r.get(1)?, r.get(2)?);
            total += c;
            *estados_m.entry(e).or_insert(0) += c;
            *idiomas_m.entry(i).or_insert(0) += c;
        }
    }
    let fts_in_sync = if available {
        let b: i64 = conn.query_row("SELECT count(*) FROM juegos_busqueda", [], |r| r.get(0))?;
        b == total
    } else {
        false
    };
    let sorted = |m: std::collections::HashMap<String, i64>| {
        let mut v: Vec<CountItem> = m
            .into_iter()
            .map(|(value, count)| CountItem { value, count })
            .collect();
        v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.value.cmp(&b.value)));
        v
    };
    let tags = counts(
        conn,
        "SELECT t.nombre, count(*) FROM tags t JOIN juego_tags jt ON jt.tag_id = t.id \
         JOIN juegos j ON j.id = jt.juego_id GROUP BY t.id ORDER BY t.nombre COLLATE NOCASE",
    )?;
    let sqlite_version: String = conn.query_row("SELECT sqlite_version()", [], |r| r.get(0))?;
    Ok(LibraryInfo {
        db_file_name: db_file_name.to_string(),
        read_only,
        total,
        fts_available: available,
        fts_in_sync,
        estados: sorted(estados_m),
        idiomas: sorted(idiomas_m),
        tags,
        sqlite_version,
    })
}

fn sorted_counts(m: std::collections::HashMap<String, i64>) -> Vec<CountItem> {
    let mut v: Vec<CountItem> = m
        .into_iter()
        .map(|(value, count)| CountItem { value, count })
        .collect();
    v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.value.cmp(&b.value)));
    v
}

/// Statistics for the dashboard (read-only). One pass over `juegos` reads only the
/// columns before `imagen` plus `length(imagen)`; the BLOB itself is never loaded.
/// Empty/NULL grouping values are reported as `""`.
///
/// Score buckets: `puntuacion` (NULL or non-numeric -> 0) is clamped to `0.0..=5.0` and
/// rounded to the nearest 0.5 (`round(x * 2) / 2`, halves away from zero, so 4.25 -> "4.5").
/// All 11 buckets "0.0".."5.0" are always returned, in ascending order.
pub fn dashboard(conn: &Connection) -> Result<Dashboard, AppError> {
    use std::collections::HashMap;
    let start = Instant::now();
    let mut total = 0i64;
    let mut estados: HashMap<String, i64> = HashMap::new();
    let mut idiomas: HashMap<String, i64> = HashMap::new();
    let mut tipos: HashMap<String, i64> = HashMap::new();
    let mut paginas: HashMap<String, i64> = HashMap::new();
    let mut ei: HashMap<(String, String), i64> = HashMap::new();
    let mut buckets = [0i64; 11];
    let mut cob = Cobertura {
        con_portada: 0,
        con_descripcion: 0,
        con_tags: 0,
        con_ruta: 0,
    };
    {
        let mut st = conn.prepare(
            "SELECT coalesce(estado,''), coalesce(idioma,''), coalesce(tipo,''), coalesce(pagina,''),              CAST(coalesce(puntuacion,0) AS REAL),              (imagen IS NOT NULL AND length(imagen) > 0),              (trim(coalesce(descripcion,''), ' ' || char(9) || char(10) || char(13)) <> ''),              (trim(coalesce(ruta,''), ' ' || char(9) || char(10) || char(13)) <> ''),              EXISTS(SELECT 1 FROM juego_tags jt WHERE jt.juego_id = juegos.id)              FROM juegos",
        )?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            total += 1;
            let estado: String = r.get(0)?;
            let idioma: String = r.get(1)?;
            let tipo: String = r.get(2)?;
            let pagina: String = r.get(3)?;
            let p: f64 = r.get(4)?;
            let p = if p.is_nan() { 0.0 } else { p.clamp(0.0, 5.0) };
            buckets[((p * 2.0).round() as usize).min(10)] += 1;
            cob.con_portada += r.get::<_, i64>(5)?;
            cob.con_descripcion += r.get::<_, i64>(6)?;
            cob.con_ruta += r.get::<_, i64>(7)?;
            cob.con_tags += r.get::<_, i64>(8)?;
            *estados.entry(estado.clone()).or_insert(0) += 1;
            *idiomas.entry(idioma.clone()).or_insert(0) += 1;
            *tipos.entry(tipo).or_insert(0) += 1;
            *paginas.entry(pagina).or_insert(0) += 1;
            *ei.entry((estado, idioma)).or_insert(0) += 1;
        }
    }
    let mut estado_idioma: Vec<EstadoIdiomaCount> = ei
        .into_iter()
        .map(|((estado, idioma), count)| EstadoIdiomaCount {
            estado,
            idioma,
            count,
        })
        .collect();
    estado_idioma.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.estado.cmp(&b.estado))
            .then_with(|| a.idioma.cmp(&b.idioma))
    });
    let puntuaciones = buckets
        .iter()
        .enumerate()
        .map(|(i, &count)| CountItem {
            value: format!("{:.1}", i as f64 / 2.0),
            count,
        })
        .collect();
    let top_tags = counts(
        conn,
        "SELECT t.nombre, count(*) AS c FROM tags t JOIN juego_tags jt ON jt.tag_id = t.id          JOIN juegos j ON j.id = jt.juego_id GROUP BY t.id          ORDER BY c DESC, t.nombre COLLATE BINARY ASC LIMIT 15",
    )?;
    Ok(Dashboard {
        total,
        estados: sorted_counts(estados),
        idiomas: sorted_counts(idiomas),
        tipos: sorted_counts(tipos),
        paginas: sorted_counts(paginas),
        puntuaciones,
        estado_idioma,
        cobertura: cob,
        top_tags,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
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

/// `use_b`: the sort may read codes from the `bq` alias (juegos_busqueda, no BLOBs).
fn order_by(sort: SortKey, use_b: bool) -> &'static str {
    match sort {
        SortKey::TitleAsc => "j.titulo COLLATE NOCASE, j.id",
        SortKey::TitleDesc => "j.titulo COLLATE NOCASE DESC, j.id",
        SortKey::Code if use_b => {
            "(trim(bq.codigo) = ''), bq.codigo COLLATE NOCASE, j.titulo COLLATE NOCASE, j.id"
        }
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

/// SQL (fixed fragments) and bound parameters of a search. `page_sql` additionally
/// needs `limit` and `offset` appended as the last two parameters. If `two_phase`,
/// `page_sql` returns only ids (phase 1) and the summaries are fetched afterwards
/// with `summaries_sql`; otherwise `page_sql` returns full summary rows.
pub struct SearchSql {
    pub count_sql: String,
    pub page_sql: String,
    pub params: Vec<Value>,
    pub strategy: &'static str,
    pub two_phase: bool,
}

/// Columns of `juegos` located after `imagen` (codigo, ruta, pagina, tipo, puntuacion) force
/// SQLite to walk the BLOB overflow chain, so phase 1 never reads them from `juegos`.
const LIKE_COLS: [&str; 7] = [
    "titulo",
    "codigo",
    "codigo_norm",
    "descripcion",
    "pagina",
    "tipo",
    "tags",
];

/// Validates the request and builds the SQL. Also used by the bench for EXPLAIN QUERY PLAN.
pub fn build_search_sql(
    conn: &Connection,
    req: &SearchRequest,
    fts_available: bool,
) -> Result<SearchSql, AppError> {
    let query = req.query.trim();
    if query.chars().count() > 200 {
        return Err(AppError::InvalidInput(
            "La consulta supera los 200 caracteres.".into(),
        ));
    }
    if let Some(t) = &req.tag {
        if t.chars().count() > 100 {
            return Err(AppError::InvalidInput(
                "El tag supera los 100 caracteres.".into(),
            ));
        }
    }
    if !(1..=200).contains(&req.limit) {
        return Err(AppError::InvalidInput(
            "limit debe estar entre 1 y 200.".into(),
        ));
    }
    if req.offset < 0 {
        return Err(AppError::InvalidInput("offset debe ser >= 0.".into()));
    }

    let has_busqueda = table_exists(conn, "juegos_busqueda")?;
    let mut conds: Vec<String> = Vec::new();
    let mut params_v: Vec<Value> = Vec::new();
    let strategy: &'static str;

    if query.is_empty() {
        strategy = "none";
    } else if let (Some(m), true) = (search::build_match(query), fts_available) {
        strategy = "fts";
        let norm = search::normalize_code(query);
        let norm_len = norm.chars().count();
        if (1..3).contains(&norm_len) {
            // Raw query >= 3 chars but its normalized code is 1-2 chars (e.g. "r-y"): the FTS
            // phrase only finds the literal text, so also match codes by LIKE (legacy parity).
            conds.push(
                "(j.id IN (SELECT rowid FROM juegos_fts WHERE juegos_fts MATCH ?) \
                 OR j.id IN (SELECT juego_id FROM juegos_busqueda WHERE codigo_norm LIKE ? ESCAPE '\\'))"
                    .to_string(),
            );
            params_v.push(Value::Text(m));
            params_v.push(Value::Text(search::like_pattern(&norm)));
        } else {
            conds.push("j.id IN (SELECT rowid FROM juegos_fts WHERE juegos_fts MATCH ?)".into());
            params_v.push(Value::Text(m));
        }
    } else {
        strategy = "like";
        // Non-ASCII queries: SQLite LIKE only folds ASCII, so compare rpg_fold(col) (Unicode
        // lowercase) against the lowercased pattern. ASCII queries keep the plain, faster LIKE.
        let fold = !query.is_ascii();
        let q = if fold {
            query.to_lowercase()
        } else {
            query.to_string()
        };
        let pat = search::like_pattern(&q);
        let col = |c: &str, prefix: &str| {
            if fold {
                format!("rpg_fold({prefix}{c})")
            } else {
                format!("{prefix}{c}")
            }
        };
        if has_busqueda {
            let norm = search::normalize_code(&q);
            let norm_pat = if norm.is_empty() {
                pat.clone()
            } else {
                search::like_pattern(&norm)
            };
            let parts: Vec<String> = LIKE_COLS
                .iter()
                .map(|c| format!("{} LIKE ? ESCAPE '\\'", col(c, "b.")))
                .collect();
            conds.push(format!(
                "j.id IN (SELECT b.juego_id FROM juegos_busqueda b WHERE {})",
                parts.join(" OR ")
            ));
            for c in LIKE_COLS {
                let p = if c == "codigo_norm" { &norm_pat } else { &pat };
                params_v.push(Value::Text(p.clone()));
            }
        } else {
            let parts: Vec<String> = ["titulo", "codigo", "descripcion", "pagina", "tipo"]
                .iter()
                .map(|c| format!("{} LIKE ? ESCAPE '\\'", col(c, "j.")))
                .collect();
            conds.push(format!("({})", parts.join(" OR ")));
            for _ in 0..5 {
                params_v.push(Value::Text(pat.clone()));
            }
        }
    }
    if let Some(e) = &req.estado {
        conds.push("j.estado = ?".into());
        params_v.push(Value::Text(e.clone()));
    }
    if let Some(i) = &req.idioma {
        conds.push("j.idioma = ?".into());
        params_v.push(Value::Text(i.clone()));
    }
    if let Some(t) = &req.tag {
        conds.push(
            "j.id IN (SELECT jt.juego_id FROM juego_tags jt JOIN tags t ON t.id = jt.tag_id \
             WHERE t.nombre = ? COLLATE NOCASE)"
                .into(),
        );
        params_v.push(Value::Text(t.clone()));
    }
    let where_sql = if conds.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conds.join(" AND "))
    };

    let count_sql = format!("SELECT count(*) FROM juegos j{where_sql}");
    // Phase 1 (ids only) is possible when juegos_busqueda exists; otherwise single phase.
    let two_phase = has_busqueda;
    // Code sort: `NOT INDEXED` keeps the planner from driving the loop through the title
    // index (ix_juegos_titulo_nocase, ADR-011/migration v1) and probing `bq` per row, which
    // measured 74 -> 189 ms at 50k rows. Rowid lookups (`j.id IN (...)`) stay available.
    let (from_j, join) = if two_phase && req.sort == SortKey::Code {
        (
            "juegos j NOT INDEXED",
            " LEFT JOIN juegos_busqueda bq ON bq.juego_id = j.id",
        )
    } else {
        ("juegos j", "")
    };
    let cols = if two_phase { "j.id" } else { SUMMARY_COLS };
    let page_sql = format!(
        "SELECT {cols} FROM {from_j}{join}{where_sql} ORDER BY {} LIMIT ? OFFSET ?",
        order_by(req.sort, two_phase)
    );
    Ok(SearchSql {
        count_sql,
        page_sql,
        params: params_v,
        strategy,
        two_phase,
    })
}

/// Phase 2: summaries for the given ids (<= 200), returned in the order of `ids`.
fn fetch_summaries(conn: &Connection, ids: &[i64]) -> Result<Vec<GameSummary>, AppError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let ph = vec!["?"; ids.len()].join(",");
    let sql = format!("SELECT {SUMMARY_COLS} FROM juegos j WHERE j.id IN ({ph})");
    let mut st = conn.prepare(&sql)?;
    let mut by_id: std::collections::HashMap<i64, GameSummary> = st
        .query_map(params_from_iter(ids.iter()), summary_from_row)?
        .map(|r| r.map(|g| (g.id, g)))
        .collect::<Result<_, _>>()?;
    Ok(ids.iter().filter_map(|id| by_id.remove(id)).collect())
}

pub fn search(
    conn: &Connection,
    req: &SearchRequest,
    fts_available: bool,
) -> Result<SearchPage, AppError> {
    let start = Instant::now();
    let q = build_search_sql(conn, req, fts_available)?;
    let total: i64 = conn.query_row(&q.count_sql, params_from_iter(q.params.iter()), |r| {
        r.get(0)
    })?;
    let mut params_v = q.params;
    params_v.push(Value::Integer(req.limit));
    params_v.push(Value::Integer(req.offset));
    let mut st = conn.prepare(&q.page_sql)?;
    let items = if q.two_phase {
        let ids = st
            .query_map(params_from_iter(params_v.iter()), |r| r.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        fetch_summaries(conn, &ids)?
    } else {
        st.query_map(params_from_iter(params_v.iter()), summary_from_row)?
            .collect::<Result<Vec<_>, _>>()?
    };

    Ok(SearchPage {
        items,
        total,
        offset: req.offset,
        limit: req.limit,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
        strategy: q.strategy,
    })
}

/// FNV-1a 64-bit. Every field is length-prefixed (u64 LE) so concatenations cannot collide.
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= x as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn field(&mut self, b: &[u8]) {
        self.bytes(&(b.len() as u64).to_le_bytes());
        self.bytes(b);
    }
}

/// Cheap fingerprint of the cover BLOB: length plus its first and last 16 bytes (Phase 5), so a
/// replaced cover of identical size still changes the record revision without reading all of it.
#[derive(Debug, Default, Clone)]
pub struct CoverFingerprint {
    pub len: i64,
    pub head: Vec<u8>,
    pub tail: Vec<u8>,
}

/// Fingerprint of the editable record state (contract Phase 4/5): titulo, descripcion, estado,
/// idioma, ruta, codigo, pagina, tipo, puntuacion, cover fingerprint and tags (NOCASE order).
pub fn revision_of(
    s: &GameSummary,
    descripcion: &str,
    ruta: &str,
    img: &CoverFingerprint,
    tags: &[String],
) -> String {
    let mut h = Fnv::new();
    for t in [
        &s.titulo,
        descripcion,
        &s.estado,
        &s.idioma,
        ruta,
        &s.codigo,
        &s.pagina,
        &s.tipo,
    ] {
        h.field(t.as_bytes());
    }
    h.field(&s.puntuacion.to_bits().to_le_bytes());
    h.field(&img.len.to_le_bytes());
    h.field(&img.head);
    h.field(&img.tail);
    h.field(&(tags.len() as u64).to_le_bytes());
    for t in tags {
        h.field(t.as_bytes());
    }
    format!("{:016x}", h.0)
}

pub fn get_game(conn: &Connection, id: i64) -> Result<GameDetail, AppError> {
    if id <= 0 {
        return Err(AppError::InvalidInput("id debe ser > 0.".into()));
    }
    let sql = format!(
        "SELECT {SUMMARY_COLS}, coalesce(j.descripcion,''), coalesce(j.ruta,''),          coalesce(length(j.imagen),0),          CAST(coalesce(substr(j.imagen,1,16),x'') AS BLOB), CAST(coalesce(substr(j.imagen,-16),x'') AS BLOB)          FROM juegos j WHERE j.id = ?"
    );
    let row = conn
        .query_row(&sql, [id], |r| {
            Ok((
                summary_from_row(r)?,
                r.get::<_, String>(9)?,
                r.get::<_, String>(10)?,
                CoverFingerprint {
                    len: r.get::<_, i64>(11)?,
                    head: r.get::<_, Vec<u8>>(12)?,
                    tail: r.get::<_, Vec<u8>>(13)?,
                },
            ))
        })
        .optional()?;
    let (summary, descripcion, ruta, img) =
        row.ok_or_else(|| AppError::NotFound(format!("No existe el juego con id {id}.")))?;
    let mut st = conn.prepare(
        "SELECT t.nombre FROM juego_tags jt JOIN tags t ON t.id = jt.tag_id          WHERE jt.juego_id = ? ORDER BY t.nombre COLLATE NOCASE",
    )?;
    let tags = st
        .query_map([id], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let revision = revision_of(&summary, &descripcion, &ruta, &img, &tags);
    Ok(GameDetail {
        summary,
        descripcion,
        ruta,
        tags,
        revision,
    })
}

pub fn mime_of(b: &[u8]) -> &'static str {
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

static PICK_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Small non-cryptographic random index in `0..n` (splitmix64 over clock nanos + counter).
fn random_below(n: usize) -> usize {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let c = PICK_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut z = nanos ^ c.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z % n as u64) as usize
}

pub fn suggest(conn: &Connection, req: &SuggestRequest) -> Result<Suggestion, AppError> {
    let total_count: i64 = conn.query_row("SELECT count(*) FROM juegos", [], |r| r.get(0))?;
    // Only ids of eligible games (estado is stored before the BLOB): no sort, no overflow reads.
    let mut ids: Vec<i64> = conn
        .prepare(&format!("SELECT id FROM juegos WHERE {ELIGIBLE}"))?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let eligible_count = ids.len() as i64;
    if ids.is_empty() {
        return Ok(Suggestion {
            game: None,
            eligible_count,
            total_count,
        });
    }
    if let Some(ex) = req.exclude_id {
        if ids.len() > 1 {
            ids.retain(|&i| i != ex);
        }
    }
    let id = ids[random_below(ids.len())];
    Ok(Suggestion {
        game: Some(get_game(conn, id)?),
        eligible_count,
        total_count,
    })
}
