//! Fase 8: `repo::dashboard` (estadísticas de solo lectura, ADR-017).
mod common;
use rpg_library_lib::models::CountItem;
use rpg_library_lib::{db, repo};
use tempfile::TempDir;

/// Seis juegos con valores vacíos/NULL, portadas (presente/NULL/blob vacío) y puntuaciones límite.
fn seed() -> String {
    let mut s = String::new();
    // titulo, estado, idioma, tipo, pagina, puntuacion, imagen, descripcion, ruta
    let rows = [
        (
            "A",
            "'Terminado'",
            "'Español'",
            "'RPG'",
            "'ryuugames'",
            "5.0",
            "X'52494646'",
            "'d'",
            "'C:/a'",
        ),
        (
            "B",
            "'Terminado'",
            "'Español'",
            "'RPG'",
            "'ryuugames'",
            "4.3",
            "X''",
            "''",
            "''",
        ),
        (
            "C",
            "'Empezado'",
            "'Inglés'",
            "'RPG'",
            "'f95'",
            "NULL",
            "NULL",
            "NULL",
            "NULL",
        ),
        ("D", "NULL", "''", "NULL", "''", "9", "X'00'", "'  '", "'x'"),
        ("E", "''", "NULL", "''", "NULL", "-3", "NULL", "'d'", "''"),
        (
            "F",
            "'Empezado'",
            "'Inglés'",
            "'Otro'",
            "'f95'",
            "0.74",
            "NULL",
            "'d'",
            "'y'",
        ),
    ];
    for (t, e, i, ty, p, pu, img, d, r) in rows {
        s += &format!(
            "INSERT INTO juegos(titulo, estado, idioma, tipo, pagina, puntuacion, imagen, descripcion, ruta) \
             VALUES ('{t}', {e}, {i}, {ty}, {p}, {pu}, {img}, {d}, {r});\n"
        );
    }
    for n in 1..=17 {
        s += &format!("INSERT INTO tags(nombre) VALUES ('t{n:02}');\n");
    }
    // t17 en 5 juegos (no en F); t15 y t16 en 2; t01..t14 en el juego 1 (1 cada una).
    for (tag, games) in [
        (17, vec![1, 2, 3, 4, 5]),
        (16, vec![1, 2]),
        (15, vec![1, 2]),
    ] {
        for g in games {
            s += &format!("INSERT INTO juego_tags(juego_id, tag_id) VALUES ({g}, {tag});\n");
        }
    }
    for tag in 1..=14 {
        s += &format!("INSERT INTO juego_tags(juego_id, tag_id) VALUES (1, {tag});\n");
    }
    s
}

fn pairs(v: &[CountItem]) -> Vec<(&str, i64)> {
    v.iter().map(|c| (c.value.as_str(), c.count)).collect()
}

#[test]
fn dashboard_counts_buckets_coverage_and_tags() {
    let dir = TempDir::new().unwrap();
    let path = common::make_db_with(&dir, "d.db", &seed());
    let before = std::fs::read(&path).unwrap();
    // La conexión de solo lectura basta (no escribe).
    let c = db::open_read_only(&path).unwrap();
    let d = repo::dashboard(&c).unwrap();

    assert_eq!(d.total, 6);
    assert_eq!(
        pairs(&d.estados),
        vec![("", 2), ("Empezado", 2), ("Terminado", 2)]
    );
    assert_eq!(
        pairs(&d.idiomas),
        vec![("", 2), ("Español", 2), ("Inglés", 2)]
    );
    assert_eq!(pairs(&d.tipos), vec![("RPG", 3), ("", 2), ("Otro", 1)]);
    assert_eq!(
        pairs(&d.paginas),
        vec![("", 2), ("f95", 2), ("ryuugames", 2)]
    );

    // 11 buckets siempre: NULL->0.0, -3->0.0, 0.74->0.5, 4.3->4.5, 5.0 y 9->5.0.
    let labels: Vec<&str> = d.puntuaciones.iter().map(|c| c.value.as_str()).collect();
    assert_eq!(
        labels,
        ["0.0", "0.5", "1.0", "1.5", "2.0", "2.5", "3.0", "3.5", "4.0", "4.5", "5.0"]
    );
    let counts: Vec<i64> = d.puntuaciones.iter().map(|c| c.count).collect();
    assert_eq!(counts, [2, 1, 0, 0, 0, 0, 0, 0, 0, 1, 2]);
    assert_eq!(counts.iter().sum::<i64>(), d.total);

    let ei: Vec<(&str, &str, i64)> = d
        .estado_idioma
        .iter()
        .map(|x| (x.estado.as_str(), x.idioma.as_str(), x.count))
        .collect();
    assert_eq!(
        ei,
        vec![
            ("", "", 2),
            ("Empezado", "Inglés", 2),
            ("Terminado", "Español", 2)
        ]
    );

    // Portada: solo A y D (NULL y blob vacío no cuentan); descripción: A, E, F ('  ' y NULL no).
    assert_eq!(d.cobertura.con_portada, 2);
    assert_eq!(d.cobertura.con_descripcion, 3);
    assert_eq!(d.cobertura.con_ruta, 3);
    assert_eq!(d.cobertura.con_tags, 5);

    // Límite 15, count desc y nombre asc.
    let names: Vec<&str> = d.top_tags.iter().map(|c| c.value.as_str()).collect();
    assert_eq!(d.top_tags.len(), 15);
    assert_eq!(&names[..3], ["t17", "t15", "t16"]);
    assert_eq!(names[3], "t01");
    assert_eq!(names[14], "t12");
    assert_eq!(d.top_tags[0].count, 5);
    assert!(d.elapsed_ms >= 0.0);

    drop(c);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "la DB no debe cambiar"
    );
}

#[test]
fn dashboard_empty_library_has_all_buckets() {
    let dir = TempDir::new().unwrap();
    let path = common::make_db_with(&dir, "e.db", "");
    let c = db::open_read_only(&path).unwrap();
    let d = repo::dashboard(&c).unwrap();
    assert_eq!(d.total, 0);
    assert!(d.estados.is_empty() && d.top_tags.is_empty() && d.estado_idioma.is_empty());
    assert_eq!(d.puntuaciones.len(), 11);
    assert_eq!(d.cobertura.con_portada, 0);
}

#[test]
fn dashboard_rounds_half_up_to_nearest_half_point() {
    let dir = TempDir::new().unwrap();
    let path = common::make_db_with(
        &dir,
        "r.db",
        "INSERT INTO juegos(titulo,puntuacion) VALUES ('a',4.25),('b',4.2),('c',4.75);",
    );
    let c = db::open_read_only(&path).unwrap();
    let d = repo::dashboard(&c).unwrap();
    let get = |l: &str| d.puntuaciones.iter().find(|x| x.value == l).unwrap().count;
    assert_eq!(get("4.5"), 1, "4.25 -> 4.5");
    assert_eq!(get("4.0"), 1, "4.2 -> 4.0");
    assert_eq!(get("5.0"), 1, "4.75 -> 5.0");
}
