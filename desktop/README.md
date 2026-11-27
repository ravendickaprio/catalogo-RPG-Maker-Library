# RPG Maker Library — escritorio (Fase 4: lectura y edición)

Tauri 2 + React/TypeScript + Rust + SQLite. La app permite consultar y editar la biblioteca (crear, editar y eliminar juegos y tags; las portadas llegan en la Fase 5). Es compatible con la app Python, que sigue funcionando sobre la misma base.

## Base de datos
- **Ajustes → Ruta de la base de datos:** escribe la ruta absoluta (por ejemplo la de tu `juegos.db`) y pulsa "Abrir". Se guarda en `settings.json`, dentro del directorio de configuración de la app.
- **Orden de prioridad:** `RPGLIB_DB_PATH` → ajustes → copia de desarrollo `../dev-data/juegos.dev.db`.
- **Modo solo lectura:** se activa en Ajustes o con `RPGLIB_READ_ONLY=1`.
- **Backups:** antes de la primera escritura de cada sesión se crea `backups/<db>.bak-AAAAMMDD-HHMMSS` junto a la base (se guardan los 20 más recientes). También puedes crear uno manual desde Ajustes.
- **Migraciones:** solo se aplican al pulsar "Aplicar migraciones" en Ajustes, con backup previo. La v1 crea un índice de títulos que acelera el orden y el scroll en bibliotecas grandes.
- La app nunca crea una base nueva y no altera el esquema al abrir. Si faltan columnas, muestra un error.

Para crear o refrescar la copia de forma segura, con el origen abierto en solo lectura y desde la raíz del repo:
```bash
python -I -c "import sqlite3; s=sqlite3.connect('file:juegos.db?mode=ro', uri=True); d=sqlite3.connect('dev-data/juegos.dev.db'); s.backup(d); print(d.execute('pragma integrity_check').fetchone())"
```

## Requisitos
- Node.js ≥ 20.19 y Rust estable (rustup).
- **Windows:** MSVC Build Tools (C++) y WebView2, que viene incluido en Windows 11. Verificado en Windows 11.
- **Nobara/Fedora (no verificado aún):** `sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel` y `sudo dnf group install "c-development"`. Consulta https://v2.tauri.app/start/prerequisites/

## Comandos (desde `desktop/`)
```bash
npm install
npm run tauri dev                    # desarrollo (vite + ventana)
npm run tauri build -- --no-bundle   # ejecutable release: src-tauri/target/release/rpg-library(.exe)
npm test                             # tests frontend (vitest)
npm run typecheck
cd src-tauri && cargo test           # tests Rust (unitarios, repo, IPC)
cd src-tauri && cargo clippy --all-targets -- -D warnings
cd src-tauri && cargo run --release --bin bench [ruta.db]
```

## Arquitectura
Ver `../docs/DECISIONS.md` (ADR) y `../docs/CONTRACTS.md` (comandos y DTO).
