//! `import_validate <db>`: compara el parser nuevo (`importer::parse_name`) y un port fiel del
//! `interpretar_nombre` legacy con el estado, idioma y código guardados en la DB (solo lectura).
//! Para cada fila con `ruta`, usa el nombre de archivo (sin .zip/.rar/.7z) o de carpeta.
//! Por defecto no imprime títulos; las discrepancias se listan como (id, campo, db, nuevo, legacy).
//! `--names` (solo depuración local, no compartir la salida): imprime además el nombre de archivo
//! de cada caso en que el legacy acierta y el parser nuevo no.
use regex::Regex;
use rpg_library_lib::importer::{file_stem_of_ruta, parse_name};
use rpg_library_lib::{db, search};
use std::path::PathBuf;

struct Legacy {
    code: Regex,
    f: Regex,
    p: Regex,
    en: Regex,
    es: Regex,
}

impl Legacy {
    fn new() -> Self {
        Legacy {
            code: Regex::new(r"(?i)RY[- _]+([A-Z0-9][A-Z0-9-]*)").unwrap(),
            f: Regex::new(r"(?i)(?:^|\s|\[)\[?F\]?(?:\s|$)").unwrap(),
            p: Regex::new(r"(?i)(?:^|\s|\[)\[?P\]?(?:\s|$)").unwrap(),
            en: Regex::new(r"(?i)(?:^|[^A-Z])EN(?:$|[^A-Z])").unwrap(),
            es: Regex::new(r"(?i)(?:^|[^A-Z])(?:ES|ESP)(?:$|[^A-Z])").unwrap(),
        }
    }

    /// Port de `CODE_RE` (el lookbehind `(?<![A-Z0-9])` se comprueba a mano; el lookahead
    /// final se cumple siempre porque la clase es codiciosa).
    fn code(&self, s: &str) -> String {
        let mut pos = 0;
        while let Some(c) = self.code.captures_at(s, pos) {
            let m = c.get(0).unwrap();
            let prev_ok = s[..m.start()]
                .chars()
                .next_back()
                .map_or(true, |ch| !ch.is_ascii_alphanumeric());
            if prev_ok {
                return format!("RY-{}", c[1].to_uppercase());
            }
            pos = m.start() + 1;
        }
        String::new()
    }

    fn estado(&self, s: &str) -> &'static str {
        if self.f.is_match(s) {
            "Terminado"
        } else if self.p.is_match(s) {
            "Empezado"
        } else {
            "Sin empezar"
        }
    }

    fn idioma(&self, s: &str) -> &'static str {
        if self.en.is_match(s) {
            "Inglés"
        } else if self.es.is_match(s) {
            "Español"
        } else {
            "Japonés"
        }
    }
}

#[derive(Default)]
struct Score {
    total: u32,
    legacy: u32,
    new: u32,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let show_names = args.iter().any(|a| a == "--names");
    let args: Vec<String> = args.into_iter().filter(|a| a != "--names").collect();
    if args.len() != 2 {
        eprintln!("uso: import_validate <db> [--names]");
        std::process::exit(1);
    }
    let path = PathBuf::from(&args[1]);
    if let Err(e) = db::check_path_allowed(&path, false) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    let conn = db::open_read_only(&path).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let leg = Legacy::new();
    let mut st = conn
        .prepare("SELECT id, ruta, estado, idioma, codigo FROM juegos WHERE trim(coalesce(ruta,'')) <> '' ORDER BY id")
        .unwrap();
    let rows: Vec<(i64, String, String, String, String)> = st
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            ))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    let (mut se, mut si, mut sc, mut scn) = (
        Score::default(),
        Score::default(),
        Score::default(),
        Score::default(),
    );
    let mut diffs: Vec<String> = Vec::new();
    let mut legacy_only_wrong = 0u32; // legacy acierta y el nuevo no
    let mut new_only_right = 0u32; // el nuevo acierta y el legacy no
    for (id, ruta, estado, idioma, codigo) in &rows {
        let name = file_stem_of_ruta(ruta);
        let n = parse_name(&name);
        let (le, li, lc) = (leg.estado(&name), leg.idioma(&name), leg.code(&name));
        for (score, db_v, new_v, leg_v, field) in [
            (&mut se, estado.as_str(), n.estado.as_str(), le, "estado"),
            (&mut si, idioma.as_str(), n.idioma.as_str(), li, "idioma"),
            (
                &mut sc,
                codigo.as_str(),
                n.codigo.as_str(),
                lc.as_str(),
                "codigo",
            ),
        ] {
            score.total += 1;
            let (l_ok, n_ok) = (db_v == leg_v, db_v == new_v);
            score.legacy += l_ok as u32;
            score.new += n_ok as u32;
            if l_ok && !n_ok {
                legacy_only_wrong += 1;
                if show_names {
                    // Solo para depurar en local: imprime el nombre de archivo.
                    println!(
                        "REGRESION id={id} {field} db={db_v:?} nuevo={new_v:?} nombre={name:?}"
                    );
                }
            }
            if n_ok && !l_ok {
                new_only_right += 1;
            }
            if !n_ok && diffs.len() < 40 {
                diffs.push(format!(
                    "id={id} campo={field} db={db_v:?} nuevo={new_v:?} legacy={leg_v:?}"
                ));
            }
        }
        // Código comparado con la normalización de codigo_norm (guiones/espacios/mayúsculas).
        scn.total += 1;
        let dbn = search::normalize_code(codigo);
        scn.legacy += (dbn == search::normalize_code(&lc)) as u32;
        scn.new += (dbn == search::normalize_code(&n.codigo)) as u32;
    }
    println!("Filas con ruta: {}", rows.len());
    println!(
        "{:<18} {:>8} {:>10} {:>8}",
        "campo", "total", "legacy", "nuevo"
    );
    for (label, s) in [
        ("estado", &se),
        ("idioma", &si),
        ("codigo (exacto)", &sc),
        ("codigo (norm.)", &scn),
    ] {
        println!("{:<18} {:>8} {:>10} {:>8}", label, s.total, s.legacy, s.new);
    }
    println!(
        "Casos (campo) donde legacy acierta y el nuevo no: {legacy_only_wrong}; nuevo acierta y legacy no: {new_only_right}"
    );
    println!(
        "Discrepancias del parser nuevo con la DB (máx. 40, sin títulos): {}",
        diffs.len()
    );
    for d in diffs {
        println!("  {d}");
    }
}
