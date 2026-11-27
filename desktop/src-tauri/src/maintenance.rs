//! Centro de mantenimiento (Fase 7, ADR-016): detectores de calidad, rutas inexistentes, tags
//! huérfanas, estado del índice de búsqueda y ediciones por lotes con registro de deshacer.
//! Nunca toca `imagen`, `descripcion`, `puntuacion` ni `ruta`; nunca abre ni modifica archivos.
use crate::commands::Ready;
use crate::error::AppError;
use crate::importer::{db_key, file_stem_of_ruta, iso_now, parse_name, FlagGuard, KeptGame};
use crate::models::GameDetail;
use crate::{repo, write};
use rusqlite::{params_from_iter, Connection, ErrorCode, Row};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

pub const UNDO_DIR: &str = "undo";
const MAX_BATCH_IDS: usize = 5000;
const MAX_LIST: i64 = 500;
const LIST_BATCHES: usize = 20;
const PROGRESS_EVERY: u64 = 50;

// ------------------------------------------------------------------ tipos

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckId {
    SinDescripcion,
    SinPortada,
    SinTags,
    CodigoDuplicado,
    CodigoVacio,
    TituloRy,
    EstadoInconsistente,
    RutaInexistente,
}

/// Detectores que se resuelven sin tocar el sistema de archivos.
pub const SQL_CHECKS: [CheckId; 7] = [
    CheckId::SinDescripcion,
    CheckId::SinPortada,
    CheckId::SinTags,
    CheckId::CodigoDuplicado,
    CheckId::CodigoVacio,
    CheckId::TituloRy,
    CheckId::EstadoInconsistente,
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckCount {
    pub check: CheckId,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: i64,
    pub titulo: String,
    pub codigo: String,
    pub estado: String,
    pub ruta: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuePage {
    pub items: Vec<Issue>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRequest {
    pub check: CheckId,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingProgress {
    pub checked: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingScan {
    pub items: Vec<Issue>,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrphanTag {
    pub id: i64,
    pub nombre: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub juegos: i64,
    pub busqueda: i64,
    pub mismatched: i64,
    pub fts_ok: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BatchField {
    Estado,
    Idioma,
    Tipo,
    Pagina,
}

impl BatchField {
    fn column(self) -> &'static str {
        match self {
            BatchField::Estado => "estado",
            BatchField::Idioma => "idioma",
            BatchField::Tipo => "tipo",
            BatchField::Pagina => "pagina",
        }
    }
    fn label(self) -> &'static str {
        match self {
            BatchField::Estado => "El estado",
            BatchField::Idioma => "El idioma",
            BatchField::Tipo => "El tipo",
            BatchField::Pagina => "La página",
        }
    }
    fn max_len(self) -> usize {
        match self {
            BatchField::Pagina => 100,
            _ => 50,
        }
    }
    fn current(self, g: &GameDetail) -> &str {
        match self {
            BatchField::Estado => &g.summary.estado,
            BatchField::Idioma => &g.summary.idioma,
            BatchField::Tipo => &g.summary.tipo,
            BatchField::Pagina => &g.summary.pagina,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BatchAction {
    SetField { field: BatchField, value: String },
    AddTag { tag: String },
    RemoveTag { tag: String },
    FixRyTitles,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRequest {
    pub ids: Vec<i64>,
    pub action: BatchAction,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchChange {
    pub id: i64,
    pub titulo: String,
    pub before: String,
    pub after: String,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSkip {
    pub id: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchPreview {
    pub changes: Vec<BatchChange>,
    pub skipped: Vec<BatchSkip>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub changed: u64,
    pub skipped: Vec<BatchSkip>,
    pub backup_path: Option<String>,
    pub batch_id: Option<String>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchEntry {
    pub id: String,
    pub timestamp: String,
    pub description: String,
    pub changed: u64,
    pub undone: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoBatchResult {
    pub batch_id: String,
    pub restored: u64,
    pub kept: Vec<KeptGame>,
    pub backup_path: Option<String>,
    pub warning: Option<String>,
}

// ------------------------------------------------------------------ cancelación del escaneo

static CANCEL: AtomicBool = AtomicBool::new(false);
static SCANNING: AtomicBool = AtomicBool::new(false);

/// Pide detener la comprobación de rutas en curso (no hace nada si no hay ninguna).
pub fn request_cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// Reserva la comprobación única de rutas y reinicia la cancelación.
pub struct MaintGuard(FlagGuard);

impl MaintGuard {
    pub fn acquire() -> Result<MaintGuard, AppError> {
        FlagGuard::acquire(
            &SCANNING,
            &CANCEL,
            "Ya hay una comprobación de rutas en curso.",
        )
        .map(MaintGuard)
    }

    pub fn cancel_flag(&self) -> &'static AtomicBool {
        self.0.cancel_flag()
    }
}

// ------------------------------------------------------------------ detectores

fn issue_cols(p: &str) -> String {
    format!(
        "{p}id, coalesce({p}titulo,''), coalesce({p}codigo,''), coalesce({p}estado,''), coalesce({p}ruta,'')"
    )
}

fn issue_from_row(r: &Row) -> rusqlite::Result<Issue> {
    Ok(Issue {
        id: r.get(0)?,
        titulo: r.get(1)?,
        codigo: r.get(2)?,
        estado: r.get(3)?,
        ruta: r.get(4)?,
        detail: None,
    })
}

/// Condición SQL (fragmento fijo) de los detectores simples sobre `juegos`.
fn simple_where(check: CheckId) -> Option<&'static str> {
    Some(match check {
        CheckId::SinDescripcion => "trim(coalesce(descripcion,'')) = ''",
        CheckId::SinPortada => "(imagen IS NULL OR length(imagen) = 0)",
        CheckId::SinTags => {
            "NOT EXISTS (SELECT 1 FROM juego_tags jt WHERE jt.juego_id = juegos.id)"
        }
        CheckId::CodigoVacio => "trim(coalesce(codigo,'')) = ''",
        CheckId::TituloRy => {
            "(upper(substr(ltrim(titulo),1,2)) = 'RY' AND (length(ltrim(titulo)) = 2 \
             OR substr(ltrim(titulo),3,1) IN ('-','_',' ')))"
        }
        _ => return None,
    })
}

/// ¿El título empieza por `RY` seguido de `-`, `_`, espacio o fin? (sin distinguir mayúsculas)
pub fn has_ry_prefix(titulo: &str) -> bool {
    let mut it = titulo.trim_start_matches(' ').chars();
    let (Some(a), Some(b)) = (it.next(), it.next()) else {
        return false;
    };
    a.eq_ignore_ascii_case(&'r')
        && b.eq_ignore_ascii_case(&'y')
        && matches!(it.next(), None | Some('-') | Some('_') | Some(' '))
}

fn dup_cte() -> String {
    format!(
        "WITH n AS (SELECT id, {} AS cn FROM juegos), \
         d AS (SELECT cn FROM n WHERE cn <> '' GROUP BY cn HAVING count(*) > 1)",
        write::norm_expr("codigo")
    )
}

/// Juegos con ruta cuyo nombre de archivo lleva una marca de estado distinta del guardado.
fn inconsistent(conn: &Connection) -> Result<Vec<Issue>, AppError> {
    let mut out = Vec::new();
    for mut i in rutas(conn)? {
        let name = file_stem_of_ruta(&i.ruta);
        let p = parse_name(&name);
        if p.estado != "Sin empezar" && p.estado.to_lowercase() != i.estado.trim().to_lowercase() {
            i.detail = Some(format!("archivo: {name} → {}", p.estado));
            out.push(i);
        }
    }
    Ok(out)
}

/// Todos los juegos con `ruta` no vacía, por id.
fn rutas(conn: &Connection) -> Result<Vec<Issue>, AppError> {
    let sql = format!(
        "SELECT {} FROM juegos j WHERE trim(coalesce(j.ruta,'')) <> '' ORDER BY j.id",
        issue_cols("j.")
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st
        .query_map([], issue_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn count_one(conn: &Connection, check: CheckId) -> Result<i64, AppError> {
    if let Some(w) = simple_where(check) {
        return Ok(
            conn.query_row(&format!("SELECT count(*) FROM juegos WHERE {w}"), [], |r| {
                r.get(0)
            })?,
        );
    }
    match check {
        CheckId::CodigoDuplicado => Ok(conn.query_row(
            &format!(
                "{} SELECT count(*) FROM n WHERE cn IN (SELECT cn FROM d)",
                dup_cte()
            ),
            [],
            |r| r.get(0),
        )?),
        CheckId::EstadoInconsistente => Ok(inconsistent(conn)?.len() as i64),
        _ => Err(AppError::InvalidInput(
            "ruta_inexistente se comprueba con scan_missing_paths.".into(),
        )),
    }
}

pub fn summary(conn: &Connection) -> Result<Vec<CheckCount>, AppError> {
    SQL_CHECKS
        .iter()
        .map(|&check| {
            Ok(CheckCount {
                check,
                count: count_one(conn, check)?,
            })
        })
        .collect()
}

fn dup_details(conn: &Connection, items: &mut [Issue], cns: &[String]) -> Result<(), AppError> {
    if items.is_empty() {
        return Ok(());
    }
    let uniq: Vec<&String> = cns.iter().collect::<HashSet<_>>().into_iter().collect();
    let ph = vec!["?"; uniq.len()].join(",");
    let sql = format!(
        "{} SELECT j.id, coalesce(j.titulo,''), n.cn FROM juegos j JOIN n ON n.id = j.id \
         WHERE n.cn IN ({ph}) ORDER BY j.id",
        dup_cte()
    );
    let mut st = conn.prepare(&sql)?;
    let mut groups: HashMap<String, Vec<(i64, String)>> = HashMap::new();
    let rows = st.query_map(params_from_iter(uniq.iter()), |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for r in rows {
        let (id, t, cn) = r?;
        groups.entry(cn).or_default().push((id, t));
    }
    for (it, cn) in items.iter_mut().zip(cns) {
        let others: Vec<String> = groups
            .get(cn)
            .map(|g| {
                g.iter()
                    .filter(|(id, _)| *id != it.id)
                    .map(|(id, t)| format!("#{id} «{t}»"))
                    .collect()
            })
            .unwrap_or_default();
        let shown = others
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let more = others.len().saturating_sub(3);
        it.detail = Some(if more > 0 {
            format!("comparte código con {shown} y {more} más")
        } else {
            format!("comparte código con {shown}")
        });
    }
    Ok(())
}

pub fn list(conn: &Connection, req: &ListRequest) -> Result<IssuePage, AppError> {
    if !(1..=MAX_LIST).contains(&req.limit) {
        return Err(AppError::InvalidInput(format!(
            "limit debe estar entre 1 y {MAX_LIST}."
        )));
    }
    if req.offset < 0 {
        return Err(AppError::InvalidInput("offset debe ser >= 0.".into()));
    }
    let total = count_one(conn, req.check)?;
    let items = if let Some(w) = simple_where(req.check) {
        let sql = format!(
            "SELECT {} FROM juegos WHERE {w} ORDER BY id LIMIT ? OFFSET ?",
            issue_cols("")
        );
        let mut st = conn.prepare(&sql)?;
        let rows = st
            .query_map([req.limit, req.offset], issue_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    } else if req.check == CheckId::CodigoDuplicado {
        let sql = format!(
            "{} SELECT {}, n.cn FROM juegos j JOIN n ON n.id = j.id \
             WHERE n.cn IN (SELECT cn FROM d) ORDER BY j.id LIMIT ? OFFSET ?",
            dup_cte(),
            issue_cols("j.")
        );
        let mut st = conn.prepare(&sql)?;
        let rows = st
            .query_map([req.limit, req.offset], |r| {
                Ok((issue_from_row(r)?, r.get::<_, String>(5)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let (mut items, cns): (Vec<Issue>, Vec<String>) = rows.into_iter().unzip();
        dup_details(conn, &mut items, &cns)?;
        items
    } else {
        inconsistent(conn)?
            .into_iter()
            .skip(req.offset as usize)
            .take(req.limit as usize)
            .collect()
    };
    Ok(IssuePage {
        items,
        total,
        offset: req.offset,
        limit: req.limit,
    })
}

// ------------------------------------------------------------------ rutas inexistentes

/// `Some(motivo)` si la ruta pertenece a otro sistema operativo y por tanto no se comprueba.
pub fn foreign_path(ruta: &str, windows: bool) -> Option<&'static str> {
    let r = ruta.trim();
    if windows {
        r.starts_with('/').then_some("ruta de Linux")
    } else {
        let b = r.as_bytes();
        (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/'))
            .then_some("ruta de Windows")
    }
}

/// Ruta de red UNC (`\\server\share` o `//server/share`): no se toca para no provocar SMB/NTLM.
pub fn is_unc(ruta: &str) -> bool {
    let r = ruta.trim();
    r.starts_with("\\\\") || r.starts_with("//")
}

fn is_absolute_here(ruta: &str) -> bool {
    Path::new(ruta.trim()).is_absolute()
}

/// Juegos con ruta (primera fase del análisis; libera la conexión antes de tocar el disco).
pub fn collect_rutas(conn: &Connection) -> Result<Vec<Issue>, AppError> {
    rutas(conn)
}

/// Comprueba `Path::exists` (solo metadatos) de cada ruta; cancelable.
pub fn check_rutas(
    rows: Vec<Issue>,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(&MissingProgress),
) -> MissingScan {
    let total = rows.len() as u64;
    let mut items = Vec::new();
    let mut cancelled = false;
    let mut checked = 0u64;
    for mut i in rows {
        if cancel.load(Ordering::SeqCst) {
            cancelled = true;
            break;
        }
        if is_unc(&i.ruta) {
            i.detail = Some("ruta de red (no comprobada)".into());
            items.push(i);
        } else if let Some(why) = foreign_path(&i.ruta, cfg!(windows)) {
            i.detail = Some(why.into());
            items.push(i);
        } else if !is_absolute_here(&i.ruta) {
            i.detail = Some("ruta relativa (no comprobada)".into());
            items.push(i);
        } else if !Path::new(i.ruta.trim()).exists() {
            i.detail = Some("no existe".into());
            items.push(i);
        }
        checked += 1;
        if checked % PROGRESS_EVERY == 0 {
            on_progress(&MissingProgress { checked, total });
        }
    }
    on_progress(&MissingProgress { checked, total });
    MissingScan { items, cancelled }
}

pub fn scan_missing_paths(
    conn: &Connection,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(&MissingProgress),
) -> Result<MissingScan, AppError> {
    Ok(check_rutas(collect_rutas(conn)?, cancel, on_progress))
}

// ------------------------------------------------------------------ tags huérfanas

pub fn list_orphan_tags(conn: &Connection) -> Result<Vec<OrphanTag>, AppError> {
    let mut st = conn.prepare(
        "SELECT id, nombre FROM tags WHERE NOT EXISTS \
         (SELECT 1 FROM juego_tags jt WHERE jt.tag_id = tags.id) ORDER BY nombre COLLATE NOCASE, id",
    )?;
    let rows = st
        .query_map([], |r| {
            Ok(OrphanTag {
                id: r.get(0)?,
                nombre: r.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ------------------------------------------------------------------ índice de búsqueda

/// Fila esperada de `juegos_busqueda` para cada juego (misma expresión que los triggers).
fn expected_select() -> String {
    format!(
        "SELECT j.id AS juego_id, COALESCE(j.titulo,'') AS titulo, COALESCE(j.codigo,'') AS codigo, \
         {} AS codigo_norm, COALESCE(j.descripcion,'') AS descripcion, COALESCE(j.pagina,'') AS pagina, \
         COALESCE(j.tipo,'') AS tipo, \
         COALESCE((SELECT group_concat(t.nombre, ' ') FROM juego_tags jt JOIN tags t ON t.id=jt.tag_id \
         WHERE jt.juego_id=j.id), '') AS tags FROM juegos j",
        write::norm_expr("j.codigo")
    )
}

const DIFF: &str = "b.titulo IS NOT e.titulo OR b.codigo IS NOT e.codigo \
     OR b.codigo_norm IS NOT e.codigo_norm OR b.descripcion IS NOT e.descripcion \
     OR b.pagina IS NOT e.pagina OR b.tipo IS NOT e.tipo OR b.tags IS NOT e.tags";

/// `integrity-check` de FTS5. En conexiones de solo lectura (no permiten el comando) se usa una
/// comprobación más débil: cada fila de `juegos_busqueda` tiene su entrada en `juegos_fts_docsize`.
fn fts_ok(conn: &Connection) -> Result<bool, AppError> {
    if !repo::table_exists(conn, "juegos_fts")? {
        return Ok(false);
    }
    match conn.execute(
        "INSERT INTO juegos_fts(juegos_fts, rank) VALUES('integrity-check', 1)",
        [],
    ) {
        Ok(_) => Ok(true),
        Err(rusqlite::Error::SqliteFailure(f, _)) if f.code == ErrorCode::ReadOnly => {
            if !repo::table_exists(conn, "juegos_fts_docsize")? {
                return Ok(false);
            }
            let missing: i64 = conn.query_row(
                "SELECT (SELECT count(*) FROM juegos_busqueda b WHERE NOT EXISTS \
                 (SELECT 1 FROM juegos_fts_docsize d WHERE d.id = b.juego_id)) \
                 + (SELECT count(*) FROM juegos_fts_docsize d WHERE NOT EXISTS \
                 (SELECT 1 FROM juegos_busqueda b WHERE b.juego_id = d.id))",
                [],
                |r| r.get(0),
            )?;
            Ok(missing == 0)
        }
        Err(e) => match AppError::from(e) {
            busy @ AppError::Busy(_) => Err(busy),
            _ => Ok(false),
        },
    }
}

pub fn search_index_status(conn: &Connection) -> Result<IndexStatus, AppError> {
    let juegos: i64 = conn.query_row("SELECT count(*) FROM juegos", [], |r| r.get(0))?;
    if !repo::table_exists(conn, "juegos_busqueda")? {
        return Ok(IndexStatus {
            juegos,
            busqueda: 0,
            mismatched: juegos,
            fts_ok: false,
        });
    }
    let busqueda: i64 = conn.query_row("SELECT count(*) FROM juegos_busqueda", [], |r| r.get(0))?;
    let sql = format!(
        "WITH e AS ({}) SELECT \
         (SELECT count(*) FROM e LEFT JOIN juegos_busqueda b ON b.juego_id = e.juego_id \
          WHERE b.juego_id IS NULL OR {DIFF}) \
         + (SELECT count(*) FROM juegos_busqueda b WHERE NOT EXISTS \
          (SELECT 1 FROM juegos j WHERE j.id = b.juego_id))",
        expected_select()
    );
    let mismatched: i64 = conn.query_row(&sql, [], |r| r.get(0))?;
    Ok(IndexStatus {
        juegos,
        busqueda,
        mismatched,
        fts_ok: fts_ok(conn)?,
    })
}

/// Resincroniza `juegos_busqueda` desde `juegos` (+tags) y reconstruye el índice FTS si existe.
/// No toca `juegos`. Backup de sesión previo.
pub fn rebuild_search_index(ready: &Ready) -> Result<IndexStatus, AppError> {
    ready.ensure_writable()?;
    ready.with_write(|conn| {
        if !repo::table_exists(conn, "juegos_busqueda")? {
            return Err(AppError::InvalidInput(
                "La base no tiene la tabla juegos_busqueda.".into(),
            ));
        }
        write::in_tx(conn, || {
            // Solo se tocan las filas que difieren (evita disparar los triggers de FTS en vano).
            conn.execute(
                &format!(
                    "INSERT INTO juegos_busqueda (juego_id, titulo, codigo, codigo_norm, descripcion, pagina, tipo, tags) \
                     SELECT juego_id, titulo, codigo, codigo_norm, descripcion, pagina, tipo, tags \
                     FROM ({}) WHERE true \
                     ON CONFLICT(juego_id) DO UPDATE SET titulo=excluded.titulo, codigo=excluded.codigo, \
                     codigo_norm=excluded.codigo_norm, descripcion=excluded.descripcion, \
                     pagina=excluded.pagina, tipo=excluded.tipo, tags=excluded.tags \
                     WHERE juegos_busqueda.titulo IS NOT excluded.titulo \
                     OR juegos_busqueda.codigo IS NOT excluded.codigo \
                     OR juegos_busqueda.codigo_norm IS NOT excluded.codigo_norm \
                     OR juegos_busqueda.descripcion IS NOT excluded.descripcion \
                     OR juegos_busqueda.pagina IS NOT excluded.pagina \
                     OR juegos_busqueda.tipo IS NOT excluded.tipo \
                     OR juegos_busqueda.tags IS NOT excluded.tags",
                    expected_select()
                ),
                [],
            )?;
            conn.execute(
                "DELETE FROM juegos_busqueda WHERE NOT EXISTS \
                 (SELECT 1 FROM juegos j WHERE j.id = juegos_busqueda.juego_id)",
                [],
            )?;
            if repo::table_exists(conn, "juegos_fts")? {
                conn.execute("INSERT INTO juegos_fts(juegos_fts) VALUES('rebuild')", [])?;
            }
            Ok(())
        })?;
        search_index_status(conn)
    })
}

// ------------------------------------------------------------------ lotes: planificación

fn validate_request(req: &BatchRequest) -> Result<(Vec<i64>, BatchAction), AppError> {
    if req.ids.is_empty() || req.ids.len() > MAX_BATCH_IDS {
        return Err(AppError::InvalidInput(format!(
            "Un lote admite entre 1 y {MAX_BATCH_IDS} juegos."
        )));
    }
    if req.ids.iter().any(|&i| i <= 0) {
        return Err(AppError::InvalidInput("Los ids deben ser > 0.".into()));
    }
    let mut seen = HashSet::new();
    let ids: Vec<i64> = req
        .ids
        .iter()
        .copied()
        .filter(|i| seen.insert(*i))
        .collect();
    let action = match &req.action {
        BatchAction::SetField { field, value } => BatchAction::SetField {
            field: *field,
            value: write::check_text(field.label(), value, true, 1, field.max_len())?,
        },
        BatchAction::AddTag { tag } => BatchAction::AddTag {
            tag: write::check_text("El tag", tag, true, 1, 100)?,
        },
        BatchAction::RemoveTag { tag } => BatchAction::RemoveTag {
            tag: write::check_text("El tag", tag, true, 1, 100)?,
        },
        BatchAction::FixRyTitles => BatchAction::FixRyTitles,
    };
    Ok((ids, action))
}

enum Change {
    Field(&'static str, String),
    Tags(Vec<String>),
}

struct Plan {
    before: String,
    after: String,
    /// Estado previo guardado en el registro de deshacer.
    undo_before: serde_json::Value,
    change: Change,
}

fn describe(a: &BatchAction) -> String {
    match a {
        BatchAction::SetField { field, value } => {
            format!("Establecer {} = «{value}»", field.column())
        }
        BatchAction::AddTag { tag } => format!("Añadir el tag «{tag}»"),
        BatchAction::RemoveTag { tag } => format!("Quitar el tag «{tag}»"),
        BatchAction::FixRyTitles => "Corregir títulos que empiezan por RY".into(),
    }
}

/// Calcula el cambio para un juego; `Err(motivo)` si se omite.
fn plan(g: &GameDetail, action: &BatchAction) -> Result<Plan, String> {
    match action {
        BatchAction::SetField { field, value } => {
            let cur = field.current(g);
            if cur == value {
                return Err("ya tiene ese valor".into());
            }
            Ok(Plan {
                before: cur.to_string(),
                after: value.clone(),
                undo_before: serde_json::json!({ field.column(): cur }),
                change: Change::Field(field.column(), value.clone()),
            })
        }
        BatchAction::AddTag { tag } => {
            if g.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                return Err("ya tiene el tag".into());
            }
            if g.tags.len() >= 50 {
                return Err("ya tiene 50 tags".into());
            }
            let mut new = g.tags.clone();
            new.push(tag.clone());
            Ok(tag_plan(&g.tags, new))
        }
        BatchAction::RemoveTag { tag } => {
            if !g.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                return Err("no tiene el tag".into());
            }
            let new: Vec<String> = g
                .tags
                .iter()
                .filter(|t| !t.eq_ignore_ascii_case(tag))
                .cloned()
                .collect();
            Ok(tag_plan(&g.tags, new))
        }
        BatchAction::FixRyTitles => {
            if !has_ry_prefix(&g.summary.titulo) {
                return Err("el título no empieza por RY".into());
            }
            let name = file_stem_of_ruta(&g.ruta);
            if name.is_empty() {
                return Err("no tiene ruta".into());
            }
            let t = write::check_text("El título", &parse_name(&name).titulo, true, 1, 300)
                .map_err(|_| "el nombre de archivo no da un título válido".to_string())?;
            if has_ry_prefix(&t) {
                return Err("el nombre de archivo no mejora el título".into());
            }
            if t == g.summary.titulo {
                return Err("ya tiene ese título".into());
            }
            if !t.chars().any(char::is_whitespace) && t.chars().count() <= 4 {
                return Err("propuesta poco fiable".into());
            }
            Ok(Plan {
                before: g.summary.titulo.clone(),
                after: t.clone(),
                undo_before: serde_json::json!({ "titulo": g.summary.titulo }),
                change: Change::Field("titulo", t),
            })
        }
    }
}

fn tag_plan(old: &[String], new: Vec<String>) -> Plan {
    Plan {
        before: old.join(", "),
        after: new.join(", "),
        undo_before: serde_json::json!({ "tags": old }),
        change: Change::Tags(new),
    }
}

fn skip(id: i64, reason: impl Into<String>) -> BatchSkip {
    BatchSkip {
        id,
        reason: reason.into(),
    }
}

/// Lee el juego; `Ok(None)` si no existe.
fn load(conn: &Connection, id: i64) -> Result<Option<GameDetail>, AppError> {
    match repo::get_game(conn, id) {
        Ok(g) => Ok(Some(g)),
        Err(AppError::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn preview_batch(conn: &Connection, req: &BatchRequest) -> Result<BatchPreview, AppError> {
    let (ids, action) = validate_request(req)?;
    let mut changes = Vec::new();
    let mut skipped = Vec::new();
    for id in ids {
        let Some(g) = load(conn, id)? else {
            skipped.push(skip(id, "no existe"));
            continue;
        };
        match plan(&g, &action) {
            Ok(p) => changes.push(BatchChange {
                id,
                titulo: g.summary.titulo.clone(),
                before: p.before,
                after: p.after,
                revision: g.revision,
            }),
            Err(r) => skipped.push(skip(id, r)),
        }
    }
    Ok(BatchPreview { changes, skipped })
}

fn exec_change(conn: &Connection, id: i64, c: &Change) -> Result<(), AppError> {
    match c {
        Change::Field(col, v) => {
            // `col` sale de un conjunto fijo (BatchField::column / "titulo").
            conn.execute(
                &format!("UPDATE juegos SET {col} = ? WHERE id = ?"),
                rusqlite::params![v, id],
            )?;
        }
        Change::Tags(t) => write::save_tags(conn, id, t)?,
    }
    Ok(())
}

// ------------------------------------------------------------------ registro de deshacer

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UndoEntry {
    id: i64,
    before: serde_json::Value,
    revision_after: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchRecord {
    #[serde(default)]
    db: Option<String>,
    id: String,
    timestamp: String,
    description: String,
    #[serde(default)]
    entries: Vec<UndoEntry>,
    /// Nombres de las tags huérfanas borradas (el deshacer las recrea sin vínculos).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    orphan_tags: Vec<String>,
    #[serde(default)]
    undone: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    undone_at: Option<String>,
}

fn record_path(dir: &Path, id: &str) -> std::path::PathBuf {
    dir.join(format!("{id}.json"))
}

/// Crea `<timestamp>-<n>.json` sin sobrescribir ninguno existente.
fn create_record(dir: &Path, mut rec: BatchRecord) -> Result<String, String> {
    use std::io::Write;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let stamp = crate::backup::utc_stamp(crate::backup::now_secs());
    for n in 1..10_000u32 {
        let id = format!("{stamp}-{n:03}");
        let path = record_path(dir, &id);
        let mut f = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        };
        rec.id = id.clone();
        let json = serde_json::to_vec_pretty(&rec).map_err(|e| e.to_string())?;
        if let Err(e) = f.write_all(&json) {
            drop(f);
            let _ = std::fs::remove_file(&path);
            return Err(e.to_string());
        }
        return Ok(id);
    }
    Err("demasiados registros de deshacer en el mismo segundo".into())
}

fn rewrite_record(dir: &Path, rec: &BatchRecord) -> Result<(), String> {
    let path = record_path(dir, &rec.id);
    let tmp = dir.join(format!("{}.json.tmp", rec.id));
    let json = serde_json::to_vec_pretty(rec).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

/// Registros válidos del directorio, del más antiguo al más reciente (los ilegibles se ignoran).
fn read_records(dir: &Path) -> Vec<BatchRecord> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut recs: Vec<BatchRecord> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read(e.path()).ok())
        .filter_map(|b| serde_json::from_slice::<BatchRecord>(&b).ok())
        .collect();
    recs.sort_by(|a, b| a.id.cmp(&b.id));
    recs
}

/// Guarda el registro; `(batchId, warning)`.
fn save_record(
    dir: Option<&Path>,
    key: Option<String>,
    description: String,
    entries: Vec<UndoEntry>,
    orphan_tags: Vec<String>,
) -> (Option<String>, Option<String>) {
    let Some(dir) = dir else {
        return (
            None,
            Some("Los cambios se aplicaron, pero no hay carpeta de datos para el registro de deshacer (no se podrá deshacer).".into()),
        );
    };
    let rec = BatchRecord {
        db: key,
        id: String::new(),
        timestamp: iso_now(),
        description,
        entries,
        orphan_tags,
        undone: false,
        undone_at: None,
    };
    match create_record(dir, rec) {
        Ok(id) => (Some(id), None),
        Err(e) => (
            None,
            Some(format!(
                "Los cambios se aplicaron, pero no se pudo guardar el registro de deshacer (no se podrá deshacer): {e}"
            )),
        ),
    }
}

// ------------------------------------------------------------------ lotes: aplicar / deshacer

/// Aplica el lote (backup de sesión + una transacción). Cada id se omite si su revisión actual
/// difiere de `revisions[id]` (la que vio la vista previa).
pub fn apply_batch(
    ready: &Ready,
    req: &BatchRequest,
    revisions: &HashMap<i64, String>,
    undo_dir: Option<&Path>,
) -> Result<BatchResult, AppError> {
    ready.ensure_writable()?;
    let (ids, action) = validate_request(req)?;
    let key = db_key(ready);
    let (changed, skipped, batch_id, warning) = ready.with_write(|conn| {
        let (entries, skipped) = write::in_tx(conn, || {
            let mut entries = Vec::new();
            let mut skipped = Vec::new();
            for &id in &ids {
                let Some(g) = load(conn, id)? else {
                    skipped.push(skip(id, "no existe"));
                    continue;
                };
                match revisions.get(&id) {
                    Some(r) if *r == g.revision => {}
                    Some(_) => {
                        skipped.push(skip(id, "se modificó desde la vista previa"));
                        continue;
                    }
                    None => {
                        skipped.push(skip(id, "falta la revisión de la vista previa"));
                        continue;
                    }
                }
                let p = match plan(&g, &action) {
                    Ok(p) => p,
                    Err(r) => {
                        skipped.push(skip(id, r));
                        continue;
                    }
                };
                exec_change(conn, id, &p.change)?;
                entries.push(UndoEntry {
                    id,
                    before: p.undo_before,
                    revision_after: repo::get_game(conn, id)?.revision,
                });
            }
            Ok((entries, skipped))
        })?;
        let changed = entries.len() as u64;
        let (batch_id, warning) = if changed > 0 {
            save_record(
                undo_dir,
                key.clone(),
                describe(&action),
                entries,
                Vec::new(),
            )
        } else {
            (None, None)
        };
        Ok((changed, skipped, batch_id, warning))
    })?;
    Ok(BatchResult {
        changed,
        skipped,
        backup_path: ready.session_info().last_backup,
        batch_id,
        warning,
    })
}

/// Borra las tags sin ningún `juego_tags`; el registro de deshacer guarda sus nombres.
pub fn delete_orphan_tags(ready: &Ready, undo_dir: Option<&Path>) -> Result<BatchResult, AppError> {
    ready.ensure_writable()?;
    let key = db_key(ready);
    let (names, batch_id, warning) = ready.with_write(|conn| {
        let names: Vec<String> = write::in_tx(conn, || {
            let names = list_orphan_tags(conn)?
                .into_iter()
                .map(|t| t.nombre)
                .collect::<Vec<_>>();
            conn.execute(
                "DELETE FROM tags WHERE NOT EXISTS \
                 (SELECT 1 FROM juego_tags jt WHERE jt.tag_id = tags.id)",
                [],
            )?;
            Ok(names)
        })?;
        let (batch_id, warning) = if names.is_empty() {
            (None, None)
        } else {
            save_record(
                undo_dir,
                key.clone(),
                "Borrar tags huérfanas".into(),
                Vec::new(),
                names.clone(),
            )
        };
        Ok((names, batch_id, warning))
    })?;
    Ok(BatchResult {
        changed: names.len() as u64,
        skipped: Vec::new(),
        backup_path: ready.session_info().last_backup,
        batch_id,
        warning,
    })
}

/// Últimos 20 lotes de la DB abierta, del más reciente al más antiguo.
pub fn list_batches(ready: &Ready, undo_dir: &Path) -> Vec<BatchEntry> {
    let key = db_key(ready);
    read_records(undo_dir)
        .into_iter()
        .filter(|r| key.is_some() && r.db == key)
        .rev()
        .take(LIST_BATCHES)
        .map(|r| BatchEntry {
            changed: (r.entries.len() + r.orphan_tags.len()) as u64,
            id: r.id,
            timestamp: r.timestamp,
            description: r.description,
            undone: r.undone,
        })
        .collect()
}

const RESTORABLE: [&str; 5] = ["titulo", "estado", "idioma", "tipo", "pagina"];

fn restore_entry(conn: &Connection, e: &UndoEntry) -> Result<(), AppError> {
    let bad = || AppError::InvalidInput("Registro de deshacer no válido.".into());
    let obj = e.before.as_object().ok_or_else(bad)?;
    if let Some(tags) = obj.get("tags") {
        let list: Vec<String> = tags
            .as_array()
            .ok_or_else(bad)?
            .iter()
            .map(|t| t.as_str().map(str::to_string).ok_or_else(bad))
            .collect::<Result<_, _>>()?;
        return write::save_tags(conn, e.id, &list);
    }
    for (k, v) in obj {
        let col = RESTORABLE.iter().find(|c| **c == k).ok_or_else(bad)?;
        let val = v.as_str().ok_or_else(bad)?;
        conn.execute(
            &format!("UPDATE juegos SET {col} = ? WHERE id = ?"),
            rusqlite::params![val, e.id],
        )?;
    }
    Ok(())
}

/// Restaura el último lote (de esta DB) en los juegos cuya revisión actual = `revisionAfter`;
/// informa de los demás. Lectura del registro, restauración y marca de deshecho ocurren bajo el
/// bloqueo de escritura: dos deshacer simultáneos se serializan.
pub fn undo_last_batch(ready: &Ready, undo_dir: &Path) -> Result<UndoBatchResult, AppError> {
    ready.ensure_writable()?;
    let key = db_key(ready);
    let (batch_id, restored, kept, warning) = ready.with_write(|conn| {
        let mut last = read_records(undo_dir)
            .into_iter()
            .rfind(|r| key.is_some() && r.db == key)
            .ok_or_else(|| AppError::NotFound("No hay lotes de esta base que deshacer.".into()))?;
        if last.undone {
            return Err(AppError::InvalidInput(
                "El último lote ya fue deshecho.".into(),
            ));
        }
        let (restored, kept) = write::in_tx(conn, || {
            let mut restored = 0u64;
            let mut kept = Vec::new();
            for e in &last.entries {
                match load(conn, e.id)? {
                    Some(g) if g.revision == e.revision_after => {
                        restore_entry(conn, e)?;
                        restored += 1;
                    }
                    Some(g) => kept.push(KeptGame {
                        id: e.id,
                        titulo: g.summary.titulo,
                        reason: "se editó después del lote".into(),
                    }),
                    None => kept.push(KeptGame {
                        id: e.id,
                        titulo: String::new(),
                        reason: "ya no existe".into(),
                    }),
                }
            }
            for name in &last.orphan_tags {
                restored +=
                    conn.execute("INSERT OR IGNORE INTO tags(nombre) VALUES (?)", [name])? as u64;
            }
            Ok((restored, kept))
        })?;
        last.undone = true;
        last.undone_at = Some(iso_now());
        // La DB ya está confirmada: un fallo del registro es un aviso, no un error.
        let warning = rewrite_record(undo_dir, &last)
            .err()
            .map(|e| format!("Se deshizo el lote, pero no se pudo marcar como deshecho: {e}"));
        Ok((last.id, restored, kept, warning))
    })?;
    Ok(UndoBatchResult {
        batch_id,
        restored,
        kept,
        backup_path: ready.session_info().last_backup,
        warning,
    })
}
