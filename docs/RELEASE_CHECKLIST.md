# Checklist de release

Marque cada punto con evidencia (salida del comando, hash, captura). **Etiquetar (`git tag`), subir (`git push`), abrir PR, publicar releases y subir instaladores requieren aprobación expresa del usuario**; este documento solo lista los pasos.

## 0. Preparación
- [ ] Rama `feature/desktop-modernization-tauri` (o la de release), `git status` limpio y sin archivos de datos personales (`*.db`, `backups/`, `dev-data/` están en `.gitignore`).
- [ ] Hash SHA-256 de la base real **antes** de empezar: `sha256sum juegos.db` (o `Get-FileHash`). Anótelo.
- [ ] Backup manual de `juegos.db` fuera del repositorio.

## 1. Calidad automática (Windows y, en CI, Linux)
- [ ] `cd desktop/src-tauri && cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test` (anote el recuento; el de la última fase documentada es 137 + los de la Fase 9)
- [ ] `cd desktop && npm test`
- [ ] `npm run typecheck`
- [ ] `npm run build`
- [ ] `npm audit --omit=dev` y `cargo audit` (si está instalado): revisar avisos; no hay dependencias de red en la app.
- [ ] Revisar permisos Tauri: `desktop/src-tauri/capabilities/default.json` solo debe tener `core:default`, `dialog:allow-open`, `dialog:allow-save`.

## 2. Datos y compatibilidad
- [ ] `python -I tools/interop_test.py` → **todo PASS**, código de salida 0. Pega la salida en `docs/PROGRESS.md`.
- [ ] `python -I tools/legacy_parity.py dev-data/juegos.dev.db` → 37/38 (la diferencia intencional del caso 19) o documentar cualquier cambio.
- [ ] `cargo run --release --bin import_validate dev-data/juegos.dev.db` sin regresiones respecto a la Fase 6.
- [ ] `PRAGMA journal_mode` sigue en `delete` y `integrity_check` = ok tras las pruebas.

## 3. Rendimiento
- [ ] `cd desktop/src-tauri && cargo run --release --bin bench dev-data/synth/synth_50000.db` (DB sintéticas, no la real). Compare con `docs/BENCHMARKS.md` (B-002): sin regresiones > 20 %.
- [ ] `python -I tools/bench_compare.py` si cambió la búsqueda; añadir la sección nueva a `BENCHMARKS.md` con el comando y la máquina.

## 4. Prueba de la interfaz real (método de `docs/PROGRESS.md`, «Prueba de la interfaz real»)
- [ ] Compilar release: `cd desktop && npx tauri build --no-bundle`.
- [ ] Lanzar el ejecutable contra una **copia** de la base con `RPGLIB_DB_PATH`, con el perfil de WebView2 en una carpeta temporal (`WEBVIEW2_USER_DATA_FOLDER`) y `--remote-debugging-port` en 127.0.0.1; controlar la ventana por CDP con `playwright-core` instalado **fuera del repo**.
- [ ] Recorrido: bienvenida/elección de DB, galería y lista, búsqueda, ficha, editar y Esc, favorito, estadísticas (clic para filtrar), mantenimiento (comprobar rutas), Ctrl+N bloqueado con diálogo abierto, revisión por tandas (Ctrl+Enter), importar → backup → historial → deshacer, juego sugerido, ajustes (backups, verificar, restaurar en copia), exportar CSV/JSON, `?` ayuda.
- [ ] **0 errores de consola**; capturas guardadas fuera del repo. Borrar la carpeta `%APPDATA%\local.rpgmakerlibrary.desktop` creada por la prueba si no era la del usuario.

## 5. Instaladores
### Windows
- [ ] `cd desktop && npx tauri build` (NSIS y MSI). Anotar nombre y tamaño de `target/release/bundle/nsis/*.exe` y `msi/*.msi`.
- [ ] Instalar en una máquina/usuario limpio (sin admin): inicia, pide base, abre una copia, desinstala sin borrar datos del usuario.
- [ ] (Opcional) Comprobar la firma/SmartScreen: el instalador **no está firmado**, avisar al usuario.
### Linux
- [ ] CI (`.github/workflows/build.yml`, solo tras aprobación para subirlo): artefactos `.deb`, `.AppImage` (Ubuntu) y `.rpm` (Fedora) generados; tests y clippy en verde.
- [ ] **Prueba manual en Nobara** (obligatoria antes de declarar soporte Linux): instalar el `.rpm`, abrir una copia de la base, portadas visibles (`cover://`), selector de archivos, búsqueda, edición, importación, backup/restauración. Registrar en `PROGRESS.md` qué puntos «(?)» de `BUILD_LINUX.md` se confirmaron.

## 6. Integridad de la base real
- [ ] Tras todas las pruebas **sobre copias**, `sha256sum juegos.db` coincide con el hash del paso 0 (la base real no se tocó).
- [ ] Si se hizo alguna prueba sobre la base real (autorizada), `integrity_check` = ok y existe backup previo.

## 7. Documentación
- [ ] `docs/MANUAL.md`, `docs/RELEASE_NOTES.md`, `docs/BUILD_LINUX.md`, `docs/PROGRESS.md`, `docs/BENCHMARKS.md`, `docs/DECISIONS.md` actualizados y coherentes con el comportamiento real.
- [ ] `README.md` lo mantiene el usuario: avisarle de los cambios que convenga reflejar.
- [ ] Sin afirmaciones de rendimiento que no estén en `BENCHMARKS.md`.

## 8. Versión y etiqueta (solo con aprobación del usuario)
- [ ] Versión coherente en `desktop/src-tauri/tauri.conf.json` (`version`), `desktop/src-tauri/Cargo.toml` (`version`) y `desktop/package.json` (`version`). En la 0.9.0 los tres coinciden.
- [ ] Commit(s) locales con mensajes claros. Fusión a `main` solo con autorización.
- [ ] `git tag -a v0.9.0 -m "…"` y subir etiqueta/instaladores: **solo si el usuario lo aprueba**.
- [ ] No hay autoactualización, telemetría ni servicios en segundo plano (verificado en `Cargo.toml` y capacidades).
