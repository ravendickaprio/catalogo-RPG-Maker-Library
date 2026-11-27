# Benchmarks

Regla: solo se registran valores **medidos**. Los targets de `PROJECT_AUDIT.md` son metas, no resultados.

## B-001 — Backend Rust, Fase 1 (2026-11-26)
- **Comando:** `cd desktop/src-tauri && cargo run --release --bin bench` (por defecto usa `dev-data/juegos.dev.db`; acepta una ruta como argumento).
- **Entorno:** Windows 11 Pro x86_64, Rust 1.100.0 (MSVC), SQLite 3.50.2 (bundled por rusqlite), perfil release.
- **DB:** copia `juegos.dev.db`, 1160 registros, 5 017 600 bytes, FTS disponible y sincronizado. Conexión de solo lectura.
- **Metodología:** 5 iteraciones de calentamiento + 50 medidas por escenario. Mide solo la ruta SQL del backend: **no incluye IPC ni render**.

| Escenario | p50 ms | p95 ms | max ms |
|---|---|---|---|
| library_info | 0.577 | 0.597 | 0.626 |
| búsqueda vacía, orden por título, limit 100 | 2.181 | 2.262 | 2.554 |
| búsqueda "the" (fts) | 1.827 | 1.967 | 2.129 |
| búsqueda "ry-1" (fts, código) | 0.309 | 0.342 | 0.342 |
| búsqueda "a" (like, < 3 caracteres) | 3.129 | 3.979 | 4.095 |
| búsqueda "zzzzqqq" (sin coincidencias) | 0.199 | 0.239 | 0.351 |
| estado='Sin empezar' + idioma='Japonés' | 0.430 | 0.459 | 0.514 |
| get_game (ids aleatorios) | 0.056 | 0.083 | 0.093 |
| get_cover (ids con portada) | 0.033 | 0.049 | 0.057 |
| suggest | 0.748 | 0.788 | 0.799 |

Observado en la app real (Fase 1): la barra de estado mostró 2,8–3,1 ms de tiempo SQL en la carga inicial (búsqueda vacía, 100 filas).

## Referencia legacy (no comparable 1:1)
El patrón SQL de `_filtrar()` replicado en Python (no la app Tk completa), sobre la misma copia y en esta máquina, da **59,5 ms** de mediana con búsqueda vacía (3481 sentencias por refresco). Fuente: auditoría Haiku, `docs/PHASE0_AUDIT_VERIFIED.md`. No incluye el render de Tk, así que no es una comparación equivalente. Una comparación justa requiere medir también la latencia por pulsación en la UI (Fase 2/9).

## Pendiente
- DB sintéticas de 1k/5k/10k/25k/50k registros (Fase 2).
- Latencia de extremo a extremo (pulsación → render), memoria y scroll (Fase 3/5).
- Linux (Nobara/Fedora): no medido.
