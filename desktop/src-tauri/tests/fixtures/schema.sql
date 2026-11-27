-- DDL extraído de la copia de juegos.db (2026-11-26). Sin datos. Solo para tests.
CREATE TABLE juegos (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                titulo      TEXT NOT NULL,
                descripcion TEXT DEFAULT '',
                estado      TEXT DEFAULT 'Sin empezar',
                idioma      TEXT DEFAULT 'Español',
                imagen      BLOB,
                ruta        TEXT DEFAULT '',
                codigo      TEXT DEFAULT ''
            , pagina TEXT DEFAULT '', tipo TEXT DEFAULT 'Otro', puntuacion REAL DEFAULT 0);

CREATE TABLE tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                nombre TEXT NOT NULL COLLATE NOCASE UNIQUE
            );

CREATE TABLE juego_tags (
                juego_id INTEGER NOT NULL REFERENCES juegos(id) ON DELETE CASCADE,
                tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                PRIMARY KEY (juego_id, tag_id)
            );

CREATE TABLE juegos_busqueda (
                juego_id    INTEGER PRIMARY KEY,
                titulo      TEXT NOT NULL DEFAULT '',
                codigo      TEXT NOT NULL DEFAULT '',
                codigo_norm TEXT NOT NULL DEFAULT '',
                descripcion TEXT NOT NULL DEFAULT '',
                pagina      TEXT NOT NULL DEFAULT '',
                tipo        TEXT NOT NULL DEFAULT '',
                tags        TEXT NOT NULL DEFAULT ''
            );

CREATE VIRTUAL TABLE juegos_fts USING fts5(
                    titulo, codigo, codigo_norm, descripcion, pagina, tipo, tags,
                    content='juegos_busqueda',
                    content_rowid='juego_id',
                    tokenize='trigram'
                );

CREATE TABLE app_metadata (
                clave TEXT PRIMARY KEY,
                valor TEXT NOT NULL
            );

CREATE TRIGGER juegos_busqueda_ai
            AFTER INSERT ON juegos BEGIN
                INSERT INTO juegos_busqueda (juego_id, titulo, codigo, codigo_norm, descripcion, pagina, tipo, tags)
                VALUES (NEW.id, COALESCE(NEW.titulo, ''), COALESCE(NEW.codigo, ''), REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(LOWER(COALESCE(NEW.codigo, '')), char(32), ''), char(9), ''), char(10), ''), char(13), ''), char(11), ''), char(12), ''), char(45), ''), char(95), ''), char(160), ''), COALESCE(NEW.descripcion, ''), COALESCE(NEW.pagina, ''), COALESCE(NEW.tipo, ''), COALESCE((SELECT group_concat(t.nombre, ' ') FROM juego_tags jt JOIN tags t ON t.id=jt.tag_id WHERE jt.juego_id=NEW.id), ''))
                ON CONFLICT(juego_id) DO UPDATE SET titulo=excluded.titulo, codigo=excluded.codigo, codigo_norm=excluded.codigo_norm, descripcion=excluded.descripcion, pagina=excluded.pagina, tipo=excluded.tipo, tags=excluded.tags;
            END;

CREATE TRIGGER juegos_busqueda_au
            AFTER UPDATE OF titulo, codigo, descripcion, pagina, tipo ON juegos BEGIN
                INSERT INTO juegos_busqueda (juego_id, titulo, codigo, codigo_norm, descripcion, pagina, tipo, tags)
                VALUES (NEW.id, COALESCE(NEW.titulo, ''), COALESCE(NEW.codigo, ''), REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(LOWER(COALESCE(NEW.codigo, '')), char(32), ''), char(9), ''), char(10), ''), char(13), ''), char(11), ''), char(12), ''), char(45), ''), char(95), ''), char(160), ''), COALESCE(NEW.descripcion, ''), COALESCE(NEW.pagina, ''), COALESCE(NEW.tipo, ''), COALESCE((SELECT group_concat(t.nombre, ' ') FROM juego_tags jt JOIN tags t ON t.id=jt.tag_id WHERE jt.juego_id=NEW.id), ''))
                ON CONFLICT(juego_id) DO UPDATE SET titulo=excluded.titulo, codigo=excluded.codigo, codigo_norm=excluded.codigo_norm, descripcion=excluded.descripcion, pagina=excluded.pagina, tipo=excluded.tipo, tags=excluded.tags;
            END;

CREATE TRIGGER juegos_busqueda_ad
            AFTER DELETE ON juegos BEGIN
                DELETE FROM juegos_busqueda WHERE juego_id=OLD.id;
            END;

CREATE TRIGGER juego_tags_busqueda_ai
            AFTER INSERT ON juego_tags BEGIN
                UPDATE juegos_busqueda SET tags=COALESCE((
                    SELECT group_concat(t.nombre, ' ')
                    FROM juego_tags jt JOIN tags t ON t.id=jt.tag_id
                    WHERE jt.juego_id=NEW.juego_id
                ), '') WHERE juego_id=NEW.juego_id;
            END;

CREATE TRIGGER juego_tags_busqueda_ad
            AFTER DELETE ON juego_tags BEGIN
                UPDATE juegos_busqueda SET tags=COALESCE((
                    SELECT group_concat(t.nombre, ' ')
                    FROM juego_tags jt JOIN tags t ON t.id=jt.tag_id
                    WHERE jt.juego_id=OLD.juego_id
                ), '') WHERE juego_id=OLD.juego_id;
            END;

CREATE TRIGGER tags_busqueda_au
            AFTER UPDATE OF nombre ON tags BEGIN
                UPDATE juegos_busqueda SET tags=COALESCE((
                    SELECT group_concat(t.nombre, ' ')
                    FROM juego_tags jt JOIN tags t ON t.id=jt.tag_id
                    WHERE jt.juego_id=juegos_busqueda.juego_id
                ), '') WHERE juego_id IN (
                    SELECT juego_id FROM juego_tags WHERE tag_id=NEW.id
                );
            END;

CREATE TRIGGER juegos_fts_ai
                AFTER INSERT ON juegos_busqueda BEGIN
                    INSERT INTO juegos_fts(
                        rowid, titulo, codigo, codigo_norm, descripcion, pagina, tipo, tags
                    ) VALUES (
                        NEW.juego_id, NEW.titulo, NEW.codigo, NEW.codigo_norm,
                        NEW.descripcion, NEW.pagina, NEW.tipo, NEW.tags
                    );
                END;

CREATE TRIGGER juegos_fts_ad
                AFTER DELETE ON juegos_busqueda BEGIN
                    INSERT INTO juegos_fts(
                        juegos_fts, rowid, titulo, codigo, codigo_norm,
                        descripcion, pagina, tipo, tags
                    ) VALUES (
                        'delete', OLD.juego_id, OLD.titulo, OLD.codigo, OLD.codigo_norm,
                        OLD.descripcion, OLD.pagina, OLD.tipo, OLD.tags
                    );
                END;

CREATE TRIGGER juegos_fts_au
                AFTER UPDATE ON juegos_busqueda BEGIN
                    INSERT INTO juegos_fts(
                        juegos_fts, rowid, titulo, codigo, codigo_norm,
                        descripcion, pagina, tipo, tags
                    ) VALUES (
                        'delete', OLD.juego_id, OLD.titulo, OLD.codigo, OLD.codigo_norm,
                        OLD.descripcion, OLD.pagina, OLD.tipo, OLD.tags
                    );
                    INSERT INTO juegos_fts(
                        rowid, titulo, codigo, codigo_norm, descripcion, pagina, tipo, tags
                    ) VALUES (
                        NEW.juego_id, NEW.titulo, NEW.codigo, NEW.codigo_norm,
                        NEW.descripcion, NEW.pagina, NEW.tipo, NEW.tags
                    );
                END;

INSERT INTO app_metadata(clave, valor) VALUES ('busqueda_schema', '1');
