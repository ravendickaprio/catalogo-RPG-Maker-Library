//! Protocolo `cover` (Fase 5, ADR-014): sirve el BLOB de portada de un juego como imagen, para
//! usarlo directamente en `<img src>` sin base64. Solo GET; no ejecuta nada del usuario.
use crate::commands::AppState;
use crate::repo;
use rusqlite::{Connection, OptionalExtension};
use std::borrow::Cow;
use tauri::http::{Method, Request, Response};
use tauri::{Manager, Runtime};

pub type Headers = Vec<(&'static str, String)>;

fn plain(status: u16, msg: &str) -> (u16, Headers, Vec<u8>) {
    (
        status,
        vec![("Content-Type", "text/plain; charset=utf-8".into())],
        msg.as_bytes().to_vec(),
    )
}

/// Parses `/<id>` (positive integer, nothing else).
pub fn parse_id(path: &str) -> Option<i64> {
    let s = path.strip_prefix('/')?;
    if s.is_empty() || s.len() > 18 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse::<i64>().ok().filter(|id| *id > 0)
}

/// Pure response logic: 200 with the blob, 404 without cover / unknown id, 400 on a bad id.
pub fn cover_response(conn: &Connection, path: &str) -> (u16, Headers, Vec<u8>) {
    let Some(id) = parse_id(path) else {
        return plain(400, "Identificador de portada no válido");
    };
    match conn
        .query_row("SELECT imagen FROM juegos WHERE id = ?", [id], |r| {
            r.get::<_, Option<Vec<u8>>>(0)
        })
        .optional()
    {
        Ok(Some(Some(b))) if !b.is_empty() => (
            200,
            vec![
                ("Content-Type", repo::mime_of(&b).into()),
                (
                    "Cache-Control",
                    "public, max-age=31536000, immutable".into(),
                ),
                ("X-Content-Type-Options", "nosniff".into()),
            ],
            b,
        ),
        Ok(_) => plain(404, "Sin portada"),
        Err(_) => plain(500, "Error al leer la portada"),
    }
}

fn build(status: u16, headers: Headers, body: Vec<u8>) -> Response<Cow<'static, [u8]>> {
    let mut b = Response::builder().status(status);
    for (k, v) in headers {
        b = b.header(k, v);
    }
    b.body(Cow::Owned(body))
        .unwrap_or_else(|_| Response::new(Cow::Borrowed(&[][..])))
}

/// Entry point registered in `app_builder`; the DB read runs on a blocking thread.
pub fn handle<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    if request.method() != Method::GET {
        let (s, h, b) = plain(405, "Método no permitido");
        responder.respond(build(s, h, b));
        return;
    }
    let path = request.uri().path().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let (s, h, b) = match app.state::<AppState>().ready() {
            Err(_) => plain(503, "Base de datos no disponible"),
            Ok(ready) => match ready.lock_conn() {
                Ok(conn) => cover_response(&conn, &path),
                Err(_) => plain(503, "Base de datos no disponible"),
            },
        };
        responder.respond(build(s, h, b));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE juegos(id INTEGER PRIMARY KEY, imagen BLOB);
             INSERT INTO juegos VALUES (1, X'52494646040000005745425056503820'),
               (2, NULL), (3, X''), (4, X'89504E470D0A1A0A00'), (5, X'FFD8FF00');",
        )
        .unwrap();
        c
    }

    fn hdr<'a>(h: &'a Headers, k: &str) -> Option<&'a str> {
        h.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str())
    }

    #[test]
    fn serves_blob_with_type_and_cache() {
        let c = db();
        for (id, mime) in [(1, "image/webp"), (4, "image/png"), (5, "image/jpeg")] {
            let (s, h, b) = cover_response(&c, &format!("/{id}"));
            assert_eq!(s, 200);
            assert_eq!(hdr(&h, "Content-Type"), Some(mime));
            assert_eq!(
                hdr(&h, "Cache-Control"),
                Some("public, max-age=31536000, immutable")
            );
            assert!(!b.is_empty());
        }
    }

    #[test]
    fn not_found_and_bad_ids() {
        let c = db();
        for p in ["/2", "/3", "/99"] {
            assert_eq!(cover_response(&c, p).0, 404, "{p}");
        }
        for p in [
            "",
            "/",
            "/0",
            "/-1",
            "/abc",
            "/1x",
            "/1/2",
            "/ 1",
            "/99999999999999999999",
        ] {
            assert_eq!(cover_response(&c, p).0, 400, "{p}");
        }
    }
}
