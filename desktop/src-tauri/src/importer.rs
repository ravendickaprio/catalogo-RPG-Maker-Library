//! Importador integrado (Fase 6, ADR-015): parser de nombres, escaneo de solo lectura,
//! detección de duplicados, importación transaccional e historial con deshacer.
//! Nunca mueve, renombra, extrae ni modifica archivos: solo lee nombres de entradas.
use crate::commands::Ready;
use crate::error::AppError;
use crate::{repo, search, write};
use regex::Regex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

const EXTENSIONS: [&str; 3] = ["zip", "rar", "7z"];
const SKIP_DIRS: [&str; 4] = [".git", "__pycache__", ".venv", "node_modules"];
const PROGRESS_EVERY: u64 = 200;
pub const MAX_CANDIDATES: usize = 20_000;
const MAX_IMPORT_ITEMS: usize = 50_000;
pub const HISTORY_FILE: &str = "import-history.jsonl";

// ------------------------------------------------------------------ parser

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedName {
    pub titulo: String,
    pub codigo: String,
    pub estado: String,
    pub idioma: String,
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("regex válida"))
}

fn code_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(?i-u:RY[- _]+([A-Z0-9][A-Z0-9-]*))")
}

struct CodeMatch {
    start: usize,
    end: usize,
    codigo: String,
}

fn find_code(s: &str) -> Option<CodeMatch> {
    let r = code_re();
    let mut pos = 0;
    while let Some(caps) = r.captures_at(s, pos) {
        let whole = caps.get(0)?;
        let prev_ok = s[..whole.start()]
            .chars()
            .next_back()
            .map_or(true, |c| !c.is_ascii_alphanumeric());
        if prev_ok {
            let g = caps.get(1)?;
            let mut end = g.end();
            // Excepción: una sola letra seguida de `_<dígitos>` (`RY-d_642996`) es el código completo.
            if g.as_str().len() == 1 && g.as_str().as_bytes()[0].is_ascii_alphabetic() {
                if let Some(d) = s[end..].strip_prefix('_') {
                    let n = d.bytes().take_while(u8::is_ascii_digit).count();
                    if n > 0 {
                        end += 1 + n;
                    }
                }
            }
            let body = s[g.start()..end].trim_end_matches('-');
            if body.is_empty() {
                return None;
            }
            let code_end = g.start() + body.len();
            // Todo lo pegado al código (`_R18`, `_ENG`, `_RJ…`) hasta un espacio, `[` o `(`
            // tampoco es título.
            let run = s[code_end..]
                .find(|c: char| c.is_whitespace() || c == '[' || c == '(')
                .unwrap_or(s.len() - code_end);
            return Some(CodeMatch {
                start: whole.start(),
                end: code_end + run,
                codigo: format!("RY-{}", body.to_uppercase()),
            });
        }
        pos = whole.start() + 1; // 'R' es ASCII: límite de carácter válido
    }
    None
}

fn is_delim(c: char) -> bool {
    c.is_whitespace() || matches!(c, '_' | '-' | '.' | '[' | ']' | '(' | ')')
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    En,
    Es,
    Jp,
}

/// Tokens de idioma: `EN`/`ES`/`ESP`/`JP`/`JPN` delimitados por `_ - . [ ] ( )` o por el borde
/// del nombre (sin distinguir mayúsculas), o con espacios solo si van en mayúsculas.
fn lang_tokens(s: &str) -> Vec<(usize, usize, Lang)> {
    let mut out = Vec::new();
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        if is_delim(chars[i].1) {
            i += 1;
            continue;
        }
        let start = chars[i].0;
        let mut j = i;
        while j < chars.len() && !is_delim(chars[j].1) {
            j += 1;
        }
        let end = if j < chars.len() { chars[j].0 } else { s.len() };
        let tok = &s[start..end];
        let lang = match tok.to_ascii_uppercase().as_str() {
            "EN" => Some(Lang::En),
            "ES" | "ESP" => Some(Lang::Es),
            "JP" | "JPN" => Some(Lang::Jp),
            _ => None,
        };
        if let Some(l) = lang {
            let left_ws = i > 0 && chars[i - 1].1.is_whitespace();
            let right_ws = j < chars.len() && chars[j].1.is_whitespace();
            let upper = tok.chars().all(|c| c.is_ascii_uppercase());
            if !(left_ws || right_ws) || upper {
                out.push((start, end, l));
            }
        }
        i = j;
    }
    out
}

fn replace_spans(s: &str, spans: &[(usize, usize)]) -> String {
    let mut spans = spans.to_vec();
    spans.sort();
    let mut out = String::with_capacity(s.len());
    let mut cur = 0;
    for (a, b) in spans {
        if a < cur {
            continue;
        }
        out.push_str(&s[cur..a]);
        out.push(' ');
        cur = b;
    }
    out.push_str(&s[cur..]);
    out
}

/// Estado y, si procede, el tramo del token inicial `F`/`P` (que no forma parte del título).
/// Un grupo entre corchetes con una palabra `F` suelta (`[F]`, `[Demo F]`) da Terminado; con `P`
/// (`[P bug]`), Empezado; `F` tiene prioridad. Sin corchetes, cuenta `F`/`P` como primer token.
fn parse_estado(original: &str) -> (&'static str, Option<(usize, usize)>) {
    static GROUP: OnceLock<Regex> = OnceLock::new();
    static LEAD: OnceLock<Regex> = OnceLock::new();
    let (mut f, mut p) = (false, false);
    for g in re(&GROUP, r"\[([^\]]*)\]").captures_iter(original) {
        for w in g[1].split(|c: char| !c.is_alphanumeric()) {
            f |= w.eq_ignore_ascii_case("F");
            p |= w.eq_ignore_ascii_case("P");
        }
    }
    if f {
        return ("Terminado", None);
    }
    if p {
        return ("Empezado", None);
    }
    let lead = re(&LEAD, r"^(?:\s*\[[^\]]*\])*\s*");
    let from = lead.find(original).map_or(0, |m| m.end());
    let rest = &original[from..];
    let tok_len = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let tok = &rest[..tok_len];
    if tok.eq_ignore_ascii_case("F") {
        return ("Terminado", Some((from, from + tok_len)));
    }
    if tok.eq_ignore_ascii_case("P") {
        return ("Empezado", Some((from, from + tok_len)));
    }
    ("Sin empezar", None)
}

/// Interpreta el nombre de un archivo (sin extensión) o carpeta. Reglas del ADR-015.
pub fn parse_name(name: &str) -> ParsedName {
    static BRACKETS: OnceLock<Regex> = OnceLock::new();
    static VERSION: OnceLock<Regex> = OnceLock::new();
    static COUNTER: OnceLock<Regex> = OnceLock::new();
    static EMPTY_PAREN: OnceLock<Regex> = OnceLock::new();
    static SEPS: OnceLock<Regex> = OnceLock::new();

    let original = name.trim();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let code = find_code(original);
    if let Some(c) = &code {
        spans.push((c.start, c.end));
    }
    let (estado, state_span) = parse_estado(original);
    spans.extend(state_span);
    // El código (y el token de estado) se quitan ANTES de tocar los corchetes: los offsets se
    // calculan siempre sobre la cadena original (bug de los 906 títulos `RY …` del legacy).
    let t = replace_spans(original, &spans);

    let idioma = {
        let toks = lang_tokens(original);
        if toks.iter().any(|x| x.2 == Lang::En) {
            "Inglés"
        } else if toks.iter().any(|x| x.2 == Lang::Es) {
            "Español"
        } else {
            "Japonés"
        }
    };

    let mut x = re(&BRACKETS, r"\[[^\]]*\]")
        .replace_all(&t, " ")
        .into_owned();
    let version = re(
        &VERSION,
        r"(?i)(^|[\s_.\-(])(?:version[\s.]?|ver\.?|v)\d+(?:[._-]\d+)*[a-z]?([^A-Za-z0-9]|$)",
    );
    for _ in 0..8 {
        let y = version
            .replace_all(&x, |c: &regex::Captures| format!(" {}", &c[2]))
            .into_owned();
        if y == x {
            break;
        }
        x = y;
    }
    let lt: Vec<(usize, usize)> = lang_tokens(&x).iter().map(|t| (t.0, t.1)).collect();
    x = replace_spans(&x, &lt);
    x = re(&COUNTER, r"\s*\(\d+\)\s*$").replace(&x, "").into_owned();
    x = re(&EMPTY_PAREN, r"\(\s*\)")
        .replace_all(&x, " ")
        .into_owned();
    x = re(&SEPS, r"[_\-.]+").replace_all(&x, " ").into_owned();
    let mut words: Vec<&str> = x.split_whitespace().collect();
    // Legado: FINAL/FINISHED/PROGRESS se quitan solo como última palabra (o entre corchetes,
    // que ya se quitaron); nunca en medio del título.
    if words.len() > 1 {
        if let Some(last) = words.last() {
            if ["FINAL", "FINISHED", "PROGRESS"]
                .iter()
                .any(|w| last.eq_ignore_ascii_case(w))
            {
                words.pop();
            }
        }
    }
    let joined = words.join(" ");
    let mut titulo = joined
        .trim_matches(|c| matches!(c, ' ' | '-' | '_' | '.' | '(' | ')' | '[' | ']'))
        .to_string();
    let codigo = code.map(|c| c.codigo).unwrap_or_default();
    if titulo.is_empty() {
        titulo = if codigo.is_empty() {
            original.to_string()
        } else {
            codigo.clone()
        };
    }
    ParsedName {
        titulo,
        codigo,
        estado: estado.into(),
        idioma: idioma.into(),
    }
}

// ------------------------------------------------------------------ DTOs

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub scanned: u64,
    pub candidates: u64,
    pub current_dir: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    Nuevo,
    DuplicadoCodigo,
    DuplicadoRuta,
    DuplicadoLote,
    SinCodigo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateOf {
    /// `None` cuando el duplicado es otro elemento del mismo lote.
    pub id: Option<i64>,
    pub titulo: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportItem {
    pub key: String,
    pub kind: String,
    pub path: String,
    pub name: String,
    pub parsed: ParsedName,
    pub status: ItemStatus,
    pub duplicate_of: Option<DuplicateOf>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportDefaults {
    pub pagina: String,
    pub tipo: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCounts {
    pub total: u64,
    pub nuevo: u64,
    pub duplicado_codigo: u64,
    pub duplicado_ruta: u64,
    pub duplicado_lote: u64,
    pub sin_codigo: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub root: String,
    pub items: Vec<ImportItem>,
    pub defaults: ImportDefaults,
    pub counts: ImportCounts,
    pub cancelled: bool,
    /// Se alcanzó `MAX_CANDIDATES`: hay más candidatos de los listados.
    pub truncated: bool,
    pub scanned: u64,
    /// Directorios que no se pudieron leer (permisos, etc.).
    pub unreadable: u64,
}

// ------------------------------------------------------------------ scan

static CANCEL: AtomicBool = AtomicBool::new(false);
static SCANNING: AtomicBool = AtomicBool::new(false);

/// Pide que el escaneo en curso se detenga (no hace nada si no hay ninguno).
pub fn request_cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// Guarda genérica de "una sola operación a la vez" con bandera de cancelación.
/// Cada módulo aporta sus propias banderas estáticas.
pub struct FlagGuard {
    scanning: &'static AtomicBool,
    cancel: &'static AtomicBool,
}

impl FlagGuard {
    pub fn acquire(
        scanning: &'static AtomicBool,
        cancel: &'static AtomicBool,
        busy_msg: &str,
    ) -> Result<FlagGuard, AppError> {
        if scanning.swap(true, Ordering::SeqCst) {
            return Err(AppError::InvalidInput(busy_msg.into()));
        }
        cancel.store(false, Ordering::SeqCst);
        Ok(FlagGuard { scanning, cancel })
    }

    /// Bandera de cancelación de la operación en curso.
    pub fn cancel_flag(&self) -> &'static AtomicBool {
        self.cancel
    }
}

impl Drop for FlagGuard {
    fn drop(&mut self) {
        self.scanning.store(false, Ordering::SeqCst);
    }
}

/// Reserva el escaneo único de la sesión y reinicia la cancelación.
pub struct ScanGuard(FlagGuard);

impl ScanGuard {
    pub fn acquire() -> Result<ScanGuard, AppError> {
        FlagGuard::acquire(&SCANNING, &CANCEL, "Ya hay un análisis en curso.").map(ScanGuard)
    }

    /// Bandera de cancelación del escaneo en curso.
    pub fn cancel_flag(&self) -> &'static AtomicBool {
        self.0.cancel_flag()
    }
}

#[derive(Debug, Clone)]
pub struct RawCandidate {
    pub kind: &'static str,
    pub path: PathBuf,
    pub name: String,
}

#[derive(Debug, Default)]
pub struct RawScan {
    pub candidates: Vec<RawCandidate>,
    pub scanned: u64,
    pub cancelled: bool,
    pub truncated: bool,
    pub unreadable: u64,
}

fn archive_ext(name: &str) -> Option<usize> {
    let dot = name.rfind('.')?;
    let ext = name[dot + 1..].to_ascii_lowercase();
    EXTENSIONS.contains(&ext.as_str()).then_some(dot)
}

/// Nombre del archivo o carpeta final de una `ruta` (separadores `\` o `/`), sin `.zip/.rar/.7z`.
pub fn file_stem_of_ruta(ruta: &str) -> String {
    let t = ruta.trim().trim_end_matches(['\\', '/']);
    let last = t.rsplit(['\\', '/']).next().unwrap_or(t);
    let lower = last.to_ascii_lowercase();
    for ext in EXTENSIONS {
        if lower.ends_with(&format!(".{ext}")) {
            return last[..last.len() - ext.len() - 1].to_string();
        }
    }
    last.to_string()
}

/// Clave de comparación de rutas: separadores unificados, sin separador final, `casefold`.
pub fn norm_path(p: &str) -> String {
    let u = p.trim().replace('\\', "/");
    let t = u.trim_end_matches('/');
    t.to_lowercase()
}

/// Recorrido de solo lectura (nombres de entradas): sin seguir enlaces simbólicos, omitiendo
/// `SKIP_DIRS`. `on_progress` se llama cada `PROGRESS_EVERY` entradas y al terminar.
pub fn scan_tree(
    root: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(&ScanProgress),
) -> Result<RawScan, AppError> {
    if !root.is_dir() {
        return Err(AppError::InvalidInput(format!(
            "'{}' no es una carpeta existente.",
            root.display()
        )));
    }
    let mut out = RawScan::default();
    let mut stack = vec![root.to_path_buf()];
    'outer: while let Some(dir) = stack.pop() {
        if cancel.load(Ordering::SeqCst) {
            out.cancelled = true;
            break;
        }
        let rd = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => {
                out.unreadable += 1;
                continue;
            }
        };
        for entry in rd.flatten() {
            out.scanned += 1;
            if out.scanned % PROGRESS_EVERY == 0 {
                on_progress(&ScanProgress {
                    scanned: out.scanned,
                    candidates: out.candidates.len() as u64,
                    current_dir: dir.to_string_lossy().into_owned(),
                });
                if cancel.load(Ordering::SeqCst) {
                    out.cancelled = true;
                    break 'outer;
                }
            }
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            let cand = if ft.is_dir() {
                if SKIP_DIRS.contains(&name.as_str()) {
                    continue;
                }
                // Legado: una carpeta con código es un candidato, y también se recorre.
                let c = find_code(&name).is_some().then(|| RawCandidate {
                    kind: "carpeta",
                    path: path.clone(),
                    name: name.clone(),
                });
                stack.push(path);
                c
            } else if ft.is_file() {
                archive_ext(&name).map(|dot| RawCandidate {
                    kind: "archivo",
                    path,
                    name: name[..dot].to_string(),
                })
            } else {
                None
            };
            if let Some(c) = cand {
                // `truncated` solo si de verdad hay un candidato más allá del límite.
                if out.candidates.len() >= MAX_CANDIDATES {
                    out.truncated = true;
                    break 'outer;
                }
                out.candidates.push(c);
            }
        }
    }
    on_progress(&ScanProgress {
        scanned: out.scanned,
        candidates: out.candidates.len() as u64,
        current_dir: root.to_string_lossy().into_owned(),
    });
    out.candidates
        .sort_by_cached_key(|c| norm_path(&c.path.to_string_lossy()));
    Ok(out)
}

#[derive(Clone, Copy)]
enum Column {
    Pagina,
    Tipo,
}

fn most_frequent(conn: &Connection, column: Column) -> Result<Option<String>, AppError> {
    let col = match column {
        Column::Pagina => "pagina",
        Column::Tipo => "tipo",
    };
    let table = if repo::table_exists(conn, "juegos_busqueda")? {
        "juegos_busqueda"
    } else {
        "juegos"
    };
    let sql = format!(
        "SELECT {col} FROM {table} WHERE trim(coalesce({col},'')) <> '' \
         GROUP BY {col} ORDER BY count(*) DESC, {col} LIMIT 1"
    );
    let mut st = conn.prepare(&sql)?;
    let mut rows = st.query([])?;
    Ok(match rows.next()? {
        Some(r) => Some(r.get::<_, String>(0)?),
        None => None,
    })
}

pub fn defaults(conn: &Connection) -> Result<ImportDefaults, AppError> {
    Ok(ImportDefaults {
        pagina: most_frequent(conn, Column::Pagina)?.unwrap_or_default(),
        tipo: most_frequent(conn, Column::Tipo)?.unwrap_or_else(|| "Otro".into()),
    })
}

/// Clasifica los candidatos contra la DB y contra el propio lote (ADR-015).
pub fn build_preview(
    conn: &Connection,
    root: &str,
    raw: RawScan,
) -> Result<ImportPreview, AppError> {
    let mut codes: HashMap<String, (i64, String)> = HashMap::new();
    let mut paths: HashMap<String, (i64, String)> = HashMap::new();
    {
        let mut st = conn.prepare("SELECT id, titulo, codigo, ruta FROM juegos ORDER BY id")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let id: i64 = r.get(0)?;
            let titulo: String = r.get::<_, Option<String>>(1)?.unwrap_or_default();
            let codigo: String = r.get::<_, Option<String>>(2)?.unwrap_or_default();
            let ruta: String = r.get::<_, Option<String>>(3)?.unwrap_or_default();
            let n = search::normalize_code(&codigo);
            if !n.is_empty() {
                codes.entry(n).or_insert((id, titulo.clone()));
            }
            let p = norm_path(&ruta);
            if !p.is_empty() {
                paths.entry(p).or_insert((id, titulo));
            }
        }
    }
    let mut batch_codes: HashMap<String, String> = HashMap::new();
    let mut counts = ImportCounts::default();
    let mut items = Vec::with_capacity(raw.candidates.len());
    for (n, c) in raw.candidates.iter().enumerate() {
        let parsed = parse_name(&c.name);
        let path = c.path.to_string_lossy().into_owned();
        let norm = search::normalize_code(&parsed.codigo);
        let (status, dup) = if parsed.codigo.is_empty() {
            (ItemStatus::SinCodigo, None)
        } else if let Some((id, t)) = codes.get(&norm) {
            (
                ItemStatus::DuplicadoCodigo,
                Some(DuplicateOf {
                    id: Some(*id),
                    titulo: t.clone(),
                }),
            )
        } else if let Some((id, t)) = paths.get(&norm_path(&path)) {
            (
                ItemStatus::DuplicadoRuta,
                Some(DuplicateOf {
                    id: Some(*id),
                    titulo: t.clone(),
                }),
            )
        } else if let Some(first) = batch_codes.get(&norm) {
            (
                ItemStatus::DuplicadoLote,
                Some(DuplicateOf {
                    id: None,
                    titulo: first.clone(),
                }),
            )
        } else {
            batch_codes.insert(norm, parsed.titulo.clone());
            (ItemStatus::Nuevo, None)
        };
        counts.total += 1;
        match status {
            ItemStatus::Nuevo => counts.nuevo += 1,
            ItemStatus::DuplicadoCodigo => counts.duplicado_codigo += 1,
            ItemStatus::DuplicadoRuta => counts.duplicado_ruta += 1,
            ItemStatus::DuplicadoLote => counts.duplicado_lote += 1,
            ItemStatus::SinCodigo => counts.sin_codigo += 1,
        }
        items.push(ImportItem {
            key: format!("i{n}"),
            kind: c.kind.into(),
            path,
            name: c.name.clone(),
            parsed,
            status,
            duplicate_of: dup,
        });
    }
    Ok(ImportPreview {
        root: root.to_string(),
        items,
        defaults: defaults(conn)?,
        counts,
        cancelled: raw.cancelled,
        truncated: raw.truncated,
        scanned: raw.scanned,
        unreadable: raw.unreadable,
    })
}

// ------------------------------------------------------------------ import

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportItemInput {
    pub key: String,
    pub path: String,
    pub titulo: String,
    pub codigo: String,
    pub estado: String,
    pub idioma: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    pub root: String,
    pub items: Vec<ImportItemInput>,
    pub pagina: String,
    pub tipo: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertedGame {
    pub id: i64,
    pub titulo: String,
    pub codigo: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedItem {
    pub key: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub inserted: Vec<InsertedGame>,
    pub skipped: Vec<SkippedItem>,
    pub backup_path: Option<String>,
    pub import_id: Option<String>,
    /// Aviso no fatal (p. ej. no se pudo escribir el historial).
    pub warning: Option<String>,
}

fn check(name: &str, v: &str, min: usize, max: usize) -> Result<String, AppError> {
    write::check_text(name, v, true, min, max)
}

struct Clean {
    titulo: String,
    codigo: String,
    estado: String,
    idioma: String,
}

fn clean_item(i: &ImportItemInput) -> Result<Clean, AppError> {
    if i.path.trim().is_empty() || i.path.contains('\0') || i.path.chars().count() > 1000 {
        return Err(AppError::InvalidInput(
            "ruta no válida (vacía o de más de 1000 caracteres)".into(),
        ));
    }
    Ok(Clean {
        titulo: check("El título", &i.titulo, 1, 300)?,
        codigo: check("El código", &i.codigo, 1, 100)?,
        estado: check("El estado", &i.estado, 1, 50)?,
        idioma: check("El idioma", &i.idioma, 1, 50)?,
    })
}

pub struct InsertedRow {
    pub id: i64,
    pub titulo: String,
    pub codigo: String,
    pub revision: String,
}

/// Dentro de una única transacción `BEGIN IMMEDIATE`: revalida y vuelve a comprobar duplicados
/// (códigos y rutas insertados después del escaneo, y dentro del propio lote), e inserta.
/// `ruta` se guarda tal cual (sin NFC ni cambio de separadores).
pub fn execute_import(
    conn: &Connection,
    req: &ImportRequest,
) -> Result<(Vec<InsertedRow>, Vec<SkippedItem>), AppError> {
    if req.items.is_empty() {
        return Err(AppError::InvalidInput(
            "No hay elementos que importar.".into(),
        ));
    }
    if req.items.len() > MAX_IMPORT_ITEMS {
        return Err(AppError::InvalidInput(format!(
            "Demasiados elementos (máximo {MAX_IMPORT_ITEMS})."
        )));
    }
    let pagina = check("La página", &req.pagina, 0, 100)?;
    let tipo = {
        let t = check("El tipo", &req.tipo, 0, 50)?;
        if t.is_empty() {
            "Otro".to_string()
        } else {
            t
        }
    };
    write::in_tx(conn, || {
        let mut paths: HashSet<String> = HashSet::new();
        {
            let mut st = conn.prepare("SELECT ruta FROM juegos WHERE ruta IS NOT NULL")?;
            let mut rows = st.query([])?;
            while let Some(r) = rows.next()? {
                let p = norm_path(&r.get::<_, String>(0)?);
                if !p.is_empty() {
                    paths.insert(p);
                }
            }
        }
        // La lista de columnas del INSERT debe seguir a `write::create_game`.
        let mut ins = conn.prepare(
            "INSERT INTO juegos (titulo, descripcion, estado, idioma, imagen, ruta, codigo, pagina, tipo, puntuacion) \
             VALUES (?, '', ?, ?, NULL, ?, ?, ?, ?, 0)",
        )?;
        let mut done = Vec::new();
        let mut skipped = Vec::new();
        for it in &req.items {
            let skip = |reason: String| SkippedItem {
                key: it.key.clone(),
                reason,
            };
            let c = match clean_item(it) {
                Ok(c) => c,
                Err(e) => {
                    skipped.push(skip(format!("omitido: {e}")));
                    continue;
                }
            };
            if search::normalize_code(&c.codigo).is_empty() {
                skipped.push(skip("omitido: sin código".into()));
                continue;
            }
            if let Some(t) = write::duplicate_code_title(conn, &c.codigo, -1)? {
                skipped.push(skip(format!("omitido: el código ya lo tiene «{t}»")));
                continue;
            }
            let np = norm_path(&it.path);
            if paths.contains(&np) {
                skipped.push(skip("omitido: la ruta ya está registrada".into()));
                continue;
            }
            ins.execute(params![
                c.titulo, c.estado, c.idioma, it.path, c.codigo, pagina, tipo
            ])?;
            paths.insert(np);
            let id = conn.last_insert_rowid();
            let revision = repo::get_game(conn, id)?.revision;
            done.push(InsertedRow {
                id,
                titulo: c.titulo,
                codigo: c.codigo,
                revision,
            });
        }
        Ok((done, skipped))
    })
}

// ------------------------------------------------------------------ history

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistInserted {
    pub id: i64,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportRecord {
    kind: String,
    id: String,
    timestamp: String,
    root: String,
    inserted: Vec<HistInserted>,
    skipped_count: u64,
    backup_path: Option<String>,
    /// Ruta canónica normalizada de la DB; ausente en registros antiguos (nunca se deshacen).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    db: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UndoRecord {
    kind: String,
    import_id: String,
    timestamp: String,
    deleted: u64,
    kept: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub timestamp: String,
    pub root: String,
    pub inserted_count: u64,
    pub skipped_count: u64,
    pub backup_path: Option<String>,
    pub undone: bool,
    pub undone_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptGame {
    pub id: i64,
    pub titulo: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoResult {
    pub import_id: String,
    pub deleted: u64,
    pub kept: Vec<KeptGame>,
    pub backup_path: Option<String>,
    /// Aviso no fatal (p. ej. no se pudo actualizar el historial).
    pub warning: Option<String>,
}

pub(crate) fn iso_now() -> String {
    crate::backup::iso_utc(crate::backup::now_secs())
}

/// Clave de la DB abierta para vincular el historial: ruta canónica normalizada.
pub fn db_key(ready: &Ready) -> Option<String> {
    let p = ready.db_path.as_ref()?;
    let c = std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
    Some(norm_path(&c.to_string_lossy()))
}

fn new_import_id() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("imp-{ms}")
}

fn append_line(path: &Path, v: &impl Serialize) -> Result<(), String> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut line = serde_json::to_string(v).map_err(|e| e.to_string())?;
    line.push('\n');
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    f.write_all(line.as_bytes()).map_err(|e| e.to_string())
}

fn read_records(path: &Path) -> (Vec<ImportRecord>, Vec<UndoRecord>) {
    let mut imports = Vec::new();
    let mut undos = Vec::new();
    let Ok(text) = std::fs::read_to_string(path) else {
        return (imports, undos);
    };
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue; // línea corrupta: se ignora
        };
        match v.get("kind").and_then(|k| k.as_str()) {
            Some("import") => {
                if let Ok(r) = serde_json::from_value::<ImportRecord>(v) {
                    imports.push(r);
                }
            }
            Some("undo") => {
                if let Ok(r) = serde_json::from_value::<UndoRecord>(v) {
                    undos.push(r);
                }
            }
            _ => {}
        }
    }
    (imports, undos)
}

/// Últimas `limit` importaciones, de la más reciente a la más antigua.
/// Solo las de la DB `db` (las antiguas sin `db` o de otra base no se muestran).
pub fn read_history(path: &Path, db: Option<&str>, limit: usize) -> Vec<HistoryEntry> {
    let (imports, undos) = read_records(path);
    imports
        .into_iter()
        .filter(|r| db.is_some() && r.db.as_deref() == db)
        .rev()
        .take(limit)
        .map(|r| {
            let u = undos.iter().find(|u| u.import_id == r.id);
            HistoryEntry {
                id: r.id,
                timestamp: r.timestamp,
                root: r.root,
                inserted_count: r.inserted.len() as u64,
                skipped_count: r.skipped_count,
                backup_path: r.backup_path,
                undone: u.is_some(),
                undone_at: u.map(|u| u.timestamp.clone()),
            }
        })
        .collect()
}

/// Importa (backup de sesión + una transacción) y añade una línea al historial.
pub fn run_import_sync(
    ready: &Ready,
    req: &ImportRequest,
    history: Option<&Path>,
) -> Result<ImportResult, AppError> {
    ready.ensure_writable()?;
    let (rows, skipped) = ready.with_write(|c| execute_import(c, req))?;
    let backup_path = ready.session_info().last_backup;
    let mut warning = None;
    let mut import_id = None;
    if !rows.is_empty() {
        let id = new_import_id();
        match history {
            Some(p) => {
                let rec = ImportRecord {
                    kind: "import".into(),
                    id: id.clone(),
                    timestamp: iso_now(),
                    root: req.root.clone(),
                    inserted: rows
                        .iter()
                        .map(|r| HistInserted {
                            id: r.id,
                            revision: r.revision.clone(),
                        })
                        .collect(),
                    skipped_count: skipped.len() as u64,
                    backup_path: backup_path.clone(),
                    db: db_key(ready),
                };
                match append_line(p, &rec) {
                    Ok(()) => import_id = Some(id),
                    Err(e) => {
                        warning = Some(format!(
                            "La importación se completó, pero no se pudo guardar el historial (no se podrá deshacer): {e}"
                        ))
                    }
                }
            }
            None => {
                warning = Some(
                    "La importación se completó, pero no hay carpeta de datos para el historial (no se podrá deshacer)."
                        .into(),
                )
            }
        }
    }
    Ok(ImportResult {
        inserted: rows
            .into_iter()
            .map(|r| InsertedGame {
                id: r.id,
                titulo: r.titulo,
                codigo: r.codigo,
            })
            .collect(),
        skipped,
        backup_path,
        import_id,
        warning,
    })
}

/// Borra los juegos de la última importación (de esta DB) cuya revisión no cambió; informa de
/// los demás. Lectura del historial, borrado y línea de deshacer ocurren bajo el mismo bloqueo
/// de escritura, así que dos deshacer simultáneos se serializan.
pub fn undo_last_import_sync(ready: &Ready, history: &Path) -> Result<UndoResult, AppError> {
    ready.ensure_writable()?;
    let key = db_key(ready);
    let (import_id, deleted, kept, warning) = ready.with_write(|conn| {
        let (imports, undos) = read_records(history);
        let last = imports
            .into_iter()
            .rfind(|r| key.is_some() && r.db == key)
            .ok_or_else(|| {
                AppError::NotFound("No hay importaciones de esta base que deshacer.".into())
            })?;
        if undos.iter().any(|u| u.import_id == last.id) {
            return Err(AppError::InvalidInput(
                "La última importación ya fue deshecha.".into(),
            ));
        }
        let (deleted, kept) = write::in_tx(conn, || {
            let mut deleted = 0u64;
            let mut kept = Vec::new();
            for h in &last.inserted {
                match repo::get_game(conn, h.id) {
                    Ok(g) if g.revision == h.revision => {
                        write::delete_game_row(conn, h.id)?;
                        deleted += 1;
                    }
                    Ok(g) => kept.push(KeptGame {
                        id: h.id,
                        titulo: g.summary.titulo,
                        reason: "se editó después de importarlo".into(),
                    }),
                    Err(AppError::NotFound(_)) => kept.push(KeptGame {
                        id: h.id,
                        titulo: String::new(),
                        reason: "ya no existe".into(),
                    }),
                    Err(e) => return Err(e),
                }
            }
            Ok((deleted, kept))
        })?;
        let rec = UndoRecord {
            kind: "undo".into(),
            import_id: last.id.clone(),
            timestamp: iso_now(),
            deleted,
            kept: kept.len() as u64,
        };
        // La DB ya está confirmada: un fallo del historial es un aviso, no un error.
        let warning = append_line(history, &rec).err().map(|e| {
            format!("Se deshizo la importación, pero no se pudo actualizar el historial: {e}")
        });
        Ok((last.id, deleted, kept, warning))
    })?;
    Ok(UndoResult {
        import_id,
        deleted,
        kept,
        backup_path: ready.session_info().last_backup,
        warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(n: &str) -> (String, String, String, String) {
        let r = parse_name(n);
        (r.titulo, r.codigo, r.estado, r.idioma)
    }

    #[test]
    fn golden_table() {
        // (nombre, titulo, codigo, estado, idioma)
        let cases = [
            (
                "[Ryuugames] RY-1065940",
                "RY-1065940",
                "RY-1065940",
                "Sin empezar",
                "Japonés",
            ),
            (
                "[F] RY-d_642996",
                "RY-D_642996",
                "RY-D_642996",
                "Terminado",
                "Japonés",
            ),
            (
                "[F] RY-RJ01718583",
                "RY-RJ01718583",
                "RY-RJ01718583",
                "Terminado",
                "Japonés",
            ),
            (
                "[F] RY-4762560 Some Game v1.2 EN",
                "Some Game",
                "RY-4762560",
                "Terminado",
                "Inglés",
            ),
            (
                "[Ryuugames] Some Game RY-1065940",
                "Some Game",
                "RY-1065940",
                "Sin empezar",
                "Japonés",
            ),
            (
                "Aventura en el bosque RY-1111111",
                "Aventura en el bosque",
                "RY-1111111",
                "Sin empezar",
                "Japonés",
            ),
            ("Juego RY-1 ES", "Juego", "RY-1", "Sin empezar", "Español"),
            (
                "RY-2630020_en",
                "RY-2630020",
                "RY-2630020",
                "Sin empezar",
                "Inglés",
            ),
            (
                "Game Title F RY-1",
                "Game Title F",
                "RY-1",
                "Sin empezar",
                "Japonés",
            ),
            ("F RY-1 Game", "Game", "RY-1", "Terminado", "Japonés"),
            ("[P] x RY-2", "x", "RY-2", "Empezado", "Japonés"),
            (
                "Game P Part RY-3",
                "Game P Part",
                "RY-3",
                "Sin empezar",
                "Japonés",
            ),
            (
                "RY-RJ01711253-",
                "RY-RJ01711253",
                "RY-RJ01711253",
                "Sin empezar",
                "Japonés",
            ),
            (
                "Juego Final RY-5",
                "Juego",
                "RY-5",
                "Sin empezar",
                "Japonés",
            ),
            (
                "Final Fantasy Quest RY-6",
                "Final Fantasy Quest",
                "RY-6",
                "Sin empezar",
                "Japonés",
            ),
            (
                "[Final] Game RY-7",
                "Game",
                "RY-7",
                "Sin empezar",
                "Japonés",
            ),
            (
                "Mi Juego RY-8 (2)",
                "Mi Juego",
                "RY-8",
                "Sin empezar",
                "Japonés",
            ),
            (
                "Mi Juego RY-9 v2.0.1",
                "Mi Juego",
                "RY-9",
                "Sin empezar",
                "Japonés",
            ),
            (
                "Mi Juego_v1.2_EN RY-10",
                "Mi Juego",
                "RY-10",
                "Sin empezar",
                "Inglés",
            ),
            (
                "Dev 2 Quest RY-11",
                "Dev 2 Quest",
                "RY-11",
                "Sin empezar",
                "Japonés",
            ),
            ("Game (EN) RY-12", "Game", "RY-12", "Sin empezar", "Inglés"),
            ("[EN] Game RY-13", "Game", "RY-13", "Sin empezar", "Inglés"),
            ("Game RY-14 ESP", "Game", "RY-14", "Sin empezar", "Español"),
            ("Game-en-RY-15", "Game", "RY-15", "Sin empezar", "Inglés"),
            (
                "[Ryuugames] F RY-16 Name",
                "Name",
                "RY-16",
                "Terminado",
                "Japonés",
            ),
            ("[F][P] Both RY-17", "Both", "RY-17", "Terminado", "Japonés"),
            (
                "ry-18 lowercase",
                "lowercase",
                "RY-18",
                "Sin empezar",
                "Japonés",
            ),
            ("RY 19 spaced", "spaced", "RY-19", "Sin empezar", "Japonés"),
            (
                "Sin codigo aqui",
                "Sin codigo aqui",
                "",
                "Sin empezar",
                "Japonés",
            ),
            ("Mary-5 RY-20", "Mary 5", "RY-20", "Sin empezar", "Japonés"),
            ("Café RY-21", "Café", "RY-21", "Sin empezar", "Japonés"),
            // Patrones de la DB real (títulos y códigos sintéticos).
            (
                "[F] RY-3713480_R18",
                "RY-3713480",
                "RY-3713480",
                "Terminado",
                "Japonés",
            ),
            (
                "[F] RY-RJ01392116_ENG",
                "RY-RJ01392116",
                "RY-RJ01392116",
                "Terminado",
                "Japonés",
            ),
            (
                "RY-RJ01090778_RJ01364426",
                "RY-RJ01090778",
                "RY-RJ01090778",
                "Sin empezar",
                "Japonés",
            ),
            (
                "RY-Danmachi_IC.part1",
                "RY-DANMACHI",
                "RY-DANMACHI",
                "Sin empezar",
                "Japonés",
            ),
            (
                "RY-13190_vβ9",
                "RY-13190",
                "RY-13190",
                "Sin empezar",
                "Japonés",
            ),
            (
                "RY-v55024_EN-this",
                "RY-V55024",
                "RY-V55024",
                "Sin empezar",
                "Inglés",
            ),
            (
                "[Ryuugames] RY-3779560-EN-CN",
                "RY-3779560-EN-CN",
                "RY-3779560-EN-CN",
                "Sin empezar",
                "Inglés",
            ),
            (
                "RY-RJ01252312_en_GMN",
                "RY-RJ01252312",
                "RY-RJ01252312",
                "Sin empezar",
                "Inglés",
            ),
            (
                "RY-RJ391966_EN_FanTL",
                "RY-RJ391966",
                "RY-RJ391966",
                "Sin empezar",
                "Inglés",
            ),
            (
                "[Ryuugames] RY-RJ01576073-V1.02_EN",
                "RY-RJ01576073-V1",
                "RY-RJ01576073-V1",
                "Sin empezar",
                "Inglés",
            ),
            (
                "[P bug] RY-30001 Game",
                "Game",
                "RY-30001",
                "Empezado",
                "Japonés",
            ),
            (
                "[P minigames] RY-30002 Game",
                "Game",
                "RY-30002",
                "Empezado",
                "Japonés",
            ),
            (
                "[P -bueno ] RY-30003 Game",
                "Game",
                "RY-30003",
                "Empezado",
                "Japonés",
            ),
            (
                "[P p] RY-30004 Game",
                "Game",
                "RY-30004",
                "Empezado",
                "Japonés",
            ),
            (
                "[Demo F] RY-30005 Game",
                "Game",
                "RY-30005",
                "Terminado",
                "Japonés",
            ),
            (
                "[P casi F] RY-30006 Game",
                "Game",
                "RY-30006",
                "Terminado",
                "Japonés",
            ),
            (
                "[Falta ver Final] RY-30007 Game",
                "Game",
                "RY-30007",
                "Sin empezar",
                "Japonés",
            ),
            (
                "[F]_RY-30008_EN",
                "RY-30008",
                "RY-30008",
                "Terminado",
                "Inglés",
            ),
            (
                "[F]RY-30009_en",
                "RY-30009",
                "RY-30009",
                "Terminado",
                "Inglés",
            ),
        ];
        for (name, t, c, e, i) in cases {
            assert_eq!(
                p(name),
                (t.into(), c.into(), e.into(), i.into()),
                "nombre: {name}"
            );
        }
    }

    #[test]
    fn code_boundaries() {
        // `RY` dentro de una palabra no es un código.
        assert_eq!(parse_name("Tarry-5 RY-30").codigo, "RY-30");
        assert_eq!(parse_name("Library 12").codigo, "");
        assert_eq!(parse_name("").titulo, "");
    }

    #[test]
    fn norm_path_unifies() {
        assert_eq!(norm_path(r"E:\Juegos\A\"), norm_path("e:/juegos/a"));
        assert_ne!(norm_path("e:/a"), norm_path("e:/b"));
    }
}
