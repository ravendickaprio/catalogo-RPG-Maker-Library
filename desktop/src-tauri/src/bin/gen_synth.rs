//! Synthetic catalogue generator: `gen_synth <out_path> <rows> [seed]`.
//! Creates a NEW database (refuses to overwrite) with the real DDL, mimicking the
//! statistical profile of the real library (docs/PHASE0_AUDIT_VERIFIED.md).
//! Titles/descriptions come from neutral built-in wordlists (no real game names).
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

const SCHEMA: &str = include_str!("../../tests/fixtures/schema.sql");

struct Rng(u64);
impl Rng {
    /// splitmix64
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    /// uniform in [0,1)
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.below((hi - lo + 1) as u64) as usize
    }
    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[self.below(v.len() as u64) as usize]
    }
}

const EN: &[&str] = &[
    "the",
    "dark",
    "girl",
    "forest",
    "castle",
    "sword",
    "dragon",
    "knight",
    "last",
    "lost",
    "of",
    "and",
    "kingdom",
    "moon",
    "star",
    "night",
    "legend",
    "hero",
    "witch",
    "tower",
    "journey",
    "silent",
    "crystal",
    "shadow",
    "fallen",
    "ancient",
    "secret",
    "garden",
    "village",
    "princess",
    "demon",
    "angel",
    "ocean",
    "winter",
    "summer",
    "memory",
    "dream",
    "fate",
    "chronicle",
    "quest",
    "empire",
    "ruins",
    "labyrinth",
    "festival",
    "academy",
    "island",
    "mirror",
    "flame",
    "storm",
    "tale",
    "épic",
    "Épica",
    "café",
    "crown",
    "thief",
    "fox",
    "wolf",
    "rain",
    "glass",
    "iron",
    "golden",
    "little",
    "great",
    "a",
    "in",
    "to",
    "with",
];
const ROMAJI: &[&str] = &[
    "sora",
    "yume",
    "hoshi",
    "tsuki",
    "kaze",
    "mori",
    "shiro",
    "kuro",
    "hana",
    "umi",
    "yuki",
    "hikari",
    "kage",
    "tabi",
    "ou",
    "ken",
    "mahou",
    "shoujo",
    "densetsu",
    "toki",
    "koi",
    "uta",
    "sakura",
    "kokoro",
    "mura",
    "kuni",
    "shima",
    "ame",
    "natsu",
    "fuyu",
    "haru",
    "aki",
    "tori",
    "kitsune",
    "ryu",
    "oni",
    "tenshi",
    "majo",
    "kishi",
    "shiro",
    "seiya",
    "hoshizora",
    "yoake",
];
const TAGS: &[&str] = &[
    "dificil",
    "Favorito",
    "corto",
    "largo",
    "Pendiente",
    "mazmorras",
    "humor",
    "dark",
    "retro",
    "Puzles",
    "multi-final",
    "cooperativo",
    "nuevo",
    "revisar",
    "sin-traduccion",
];
const OTHER_TYPES: &[&str] = &["Visual Novel", "Otro", "3d", "Simulador"];

fn word(r: &mut Rng) -> &'static str {
    if r.below(100) < 58 {
        r.pick(EN)
    } else {
        r.pick(ROMAJI)
    }
}

fn capitalize(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn title(r: &mut Rng) -> String {
    let n = r.range(2, 12);
    let mut parts: Vec<String> = (0..n).map(|_| capitalize(word(r))).collect();
    if r.below(100) < 78 {
        // ~78% of the real titles start with 'RY ' (importer bug, audit)
        parts.insert(0, "RY".into());
    }
    let mut t = parts.join(" ");
    if t.chars().count() > 130 {
        t = t.chars().take(130).collect();
    }
    t
}

fn description(r: &mut Rng) -> String {
    let target = r.range(50, 1200);
    let mut s = String::new();
    let mut words_in_sentence = 0;
    while s.chars().count() < target {
        s.push_str(word(r));
        words_in_sentence += 1;
        if words_in_sentence >= r.range(6, 14) {
            s.push('.');
            s.push(if r.below(100) < 12 { '\n' } else { ' ' });
            words_in_sentence = 0;
        } else {
            s.push(' ');
        }
    }
    s.chars().take(target).collect::<String>()
}

fn webp_blob(r: &mut Rng) -> Vec<u8> {
    let len = r.range(3 * 1024, 28 * 1024);
    let mut v = Vec::with_capacity(len);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&((len - 8) as u32).to_le_bytes());
    v.extend_from_slice(b"WEBPVP8 ");
    while v.len() < len {
        v.extend_from_slice(&r.next().to_le_bytes());
    }
    v.truncate(len);
    v
}

fn code(r: &mut Rng, used: &mut HashSet<String>) -> String {
    let roll = r.unit();
    if roll < 0.003 {
        return String::new();
    }
    loop {
        let d = 1_000_000 + r.below(9_000_000);
        let c = if roll < 0.80 {
            format!("RY-{d:07}")
        } else if roll < 0.92 {
            format!("RY-RJ0{d:07}")
        } else {
            format!("RJ{d:08}")
        };
        if used.insert(c.clone()) {
            return c;
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("uso: gen_synth <out_path> <rows> [seed]");
        std::process::exit(2);
    }
    let out = Path::new(&args[1]);
    let rows: usize = args[2].parse().unwrap_or_else(|_| {
        eprintln!("rows inválido");
        std::process::exit(2)
    });
    let seed: u64 = args
        .get(3)
        .map(|s| s.parse().expect("seed inválida"))
        .unwrap_or(42);
    let fname = out
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if fname == "juegos.db" {
        eprintln!("Se rechaza escribir 'juegos.db'.");
        std::process::exit(1);
    }
    if out.exists() {
        eprintln!("'{}' ya existe; no se sobrescribe.", out.display());
        std::process::exit(1);
    }
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).expect("no se pudo crear el directorio");
        }
    }

    let t0 = Instant::now();
    let mut conn = Connection::open(out).expect("no se pudo crear la DB");
    conn.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;")
        .unwrap();
    conn.execute_batch(SCHEMA).expect("DDL");
    let mut r = Rng(seed);
    let mut used = HashSet::new();
    let mut n_desc = 0usize;
    let mut n_img = 0usize;
    let mut n_tagged = 0usize;
    let mut n_term = 0usize;

    let tx = conn.transaction().unwrap();
    for (i, t) in TAGS.iter().enumerate() {
        tx.execute(
            "INSERT INTO tags(id, nombre) VALUES (?, ?)",
            params![i as i64 + 1, t],
        )
        .unwrap();
    }
    {
        let mut ins = tx
            .prepare(
                "INSERT INTO juegos(titulo, descripcion, estado, idioma, imagen, ruta, codigo, pagina, tipo, puntuacion) \
                 VALUES (?,?,?,?,?,?,?,?,?,?)",
            )
            .unwrap();
        let mut ins_tag = tx
            .prepare("INSERT OR IGNORE INTO juego_tags(juego_id, tag_id) VALUES (?, ?)")
            .unwrap();
        for i in 0..rows {
            let u = r.unit();
            let estado = if u < 0.83 {
                "Sin empezar"
            } else if u < 0.87 {
                "Empezado"
            } else {
                "Terminado"
            };
            let term = estado == "Terminado";
            if term {
                n_term += 1;
            }
            let idioma = if r.unit() < 0.72 {
                "Japonés"
            } else {
                "Inglés"
            };
            let tipo = if r.unit() < 0.99 {
                "RPG"
            } else {
                r.pick(OTHER_TYPES)
            };
            let c = code(&mut r, &mut used);
            let desc = if r.unit() < 0.21 {
                n_desc += 1;
                description(&mut r)
            } else {
                String::new()
            };
            let has_img = if term {
                r.unit() < 0.90
            } else {
                r.unit() < 0.106
            };
            let img: Option<Vec<u8>> = if has_img {
                n_img += 1;
                Some(webp_blob(&mut r))
            } else {
                None
            };
            let punt = if r.unit() < 0.9 {
                0.0
            } else {
                (r.range(1, 10) as f64) / 2.0
            };
            let ruta = format!(
                "E:\\Games\\{}",
                if c.is_empty() {
                    format!("g{i}")
                } else {
                    c.clone()
                }
            );
            ins.execute(params![
                title(&mut r),
                desc,
                estado,
                idioma,
                img,
                ruta,
                c,
                "ryuugames",
                tipo,
                punt
            ])
            .unwrap();
            let id = tx.last_insert_rowid();
            if r.unit() < 0.005 {
                n_tagged += 1;
                for _ in 0..r.range(1, 3) {
                    ins_tag
                        .execute(params![id, r.below(TAGS.len() as u64) as i64 + 1])
                        .unwrap();
                }
            }
        }
    }
    tx.commit().unwrap();
    let insert_ms = t0.elapsed().as_secs_f64() * 1000.0;

    conn.execute(
        "INSERT INTO juegos_fts(juegos_fts) VALUES('integrity-check')",
        [],
    )
    .expect("FTS integrity-check falló");
    let ic: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    let cnt = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap() };
    let (j, b) = (
        cnt("SELECT count(*) FROM juegos"),
        cnt("SELECT count(*) FROM juegos_busqueda"),
    );
    let links = cnt("SELECT count(*) FROM juego_tags");
    drop(conn);
    let size = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    println!(
        "{}: juegos={j} busqueda={b} terminados={n_term} con_desc={n_desc} con_imagen={n_img} \
         con_tags={n_tagged} vinculos_tags={links} integrity_check={ic} fts_integrity=ok \
         size={size} bytes ({:.1} MiB) seed={seed} tiempo={:.0} ms",
        out.display(),
        size as f64 / 1_048_576.0,
        t0.elapsed().as_secs_f64() * 1000.0,
    );
    let _ = insert_ms;
}
