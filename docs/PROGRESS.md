# Progreso

## Fase 0 — Auditoría y plan verificado ✅ (2026-11-26)
- Se creó la rama `feature/desktop-modernization-tauri` desde `feature/description-cleanup-phase1` (ADR-001). `main` no se tocó.
- Copia de la DB en `dev-data/juegos.dev.db`, hecha con la API `backup` y el origen abierto en `mode=ro`. `integrity_check` dio ok.
- La auditoría read-only la hizo un subagente Haiku y el coordinador la verificó de forma independiente: `docs/PHASE0_AUDIT_VERIFIED.md`.
- Documentos generados: ADRs en `docs/DECISIONS.md` y contrato en `docs/CONTRACTS.md`.
- `.gitignore` ahora cubre el backup de la DB, que antes no estaba ignorado, además de `dev-data/` y `config.json`.

## Fase 1 — Prototipo tecnológico ✅ en Windows (2026-11-26)

**Implementación.** Dos subagentes Sonnet implementaron el backend Rust (`desktop/src-tauri`) y el frontend React (`desktop/src`) en paralelo, sobre el contrato. El coordinador revisó el resultado e integró:
- Elegibilidad más robusta frente a tab/CR/LF en `estado`.
- Tests IPC de extremo a extremo con `tauri::test::MockRuntime`.
- Manifest de Windows embebido para los ejecutables de test.
- `default-run`.
- Corrección del escenario de bench.

**Funcionalidad.** Listado virtualizado (TanStack Virtual) con paginación de 100 en 100, búsqueda con debounce de 150 ms y descarte de resultados obsoletos, FTS5 trigram con fallback LIKE, filtros de estado e idioma, 6 órdenes, panel de detalle con portada diferida y diálogo **"Juego sugerido del día"** en solo lectura.

### Evidencias ejecutadas
| Verificación | Resultado |
|---|---|
| `cargo test` | 10 unitarios + 19 integración (repo) + 2 IPC: **31 ok** |
| `cargo clippy --all-targets -D warnings`, `cargo fmt --check` | ok |
| `npm run typecheck`, `npm test` (12 tests), `npm run build` | ok |
| `npx tauri build --no-bundle` | `rpg-library.exe` (release) generado |
| Smoke test de la app real contra la copia | La ventana abre, lista los 1160 juegos y la carga inicial tarda 2,8–3,1 ms de SQL |
| Botón de sugerencia en la app real | El diálogo mostró un juego "Sin empezar", "Elegido entre 1007 juegos no terminados de 1160" (coincide con el conteo de la auditoría) y "Sin portada" |
| Ausencia de escritura | Hash SHA-256 de la copia idéntico antes y después de lanzar la app y usar la sugerencia. En los tests, archivo byte a byte idéntico tras 100 sugerencias + búsquedas, y UPDATE/DELETE/CREATE rechazados |
| Comando no registrado (`execute_sql`) vía IPC | Rechazado |

Los benchmarks están en `docs/BENCHMARKS.md`.

### Pendientes y limitaciones de la Fase 1
- **Linux (Nobara/Fedora) sin compilar ni probar**: no hay entorno disponible en esta sesión.
- La virtualización de `GameList` y el hook `useGameSearch` no tienen tests de UI (jsdom no tiene layout). Se verificaron a mano en la app real.
- "Ver en la lista" selecciona el juego en el detalle, pero no limpia los filtros.
- El fallback LIKE (consultas de < 3 caracteres) solo pliega mayúsculas en ASCII. Ni FTS ni LIKE ignoran acentos (igual que el legacy).
- Icono provisional (placeholder generado).
- No hay benchmarks a escala (1k–50k) ni latencia medida de extremo a extremo en la UI.

## Fase 2 — Motor de búsqueda ✅ (2026-11-27)

**Implementación.** Dos subagentes Sonnet hicieron el backend y el frontend. El coordinador diagnosticó el coste de overflow del BLOB y tomó las decisiones de paridad (ADR-011), las de migraciones (ADR-009) y las de concurrencia (ADR-010).
- **Backend:**
  - filtro por tag y `LibraryInfo.tags`;
  - búsqueda en dos fases;
  - sugerencia sin `ORDER BY random()`;
  - `library_info` en caché;
  - plegado Unicode (`rpg_fold`) en la ruta LIKE;
  - coincidencia de códigos cortos normalizados;
  - desempate de `title_desc` como el legacy.
- **Herramientas:** `gen_synth` (DB sintéticas), `bench` con `--plans` e `--index-experiment`, y `parity` + `tools/legacy_parity.py`.
- **Frontend:**
  - selector de etiquetas y botón "Limpiar";
  - Esc en el buscador borra el texto;
  - "Ver en la lista" limpia los filtros, busca por código, selecciona, hace scroll y enfoca la fila;
  - línea de estado con resultados, cargados y estrategia;
  - icono SVG para la columna de portada;
  - infraestructura de tests con `globals` y `setup`.

### Evidencias ejecutadas
| Verificación | Resultado |
|---|---|
| `cargo test` | 10 unitarios + 26 de repo + 2 IPC: **38 ok** |
| `cargo clippy -D warnings`, `fmt --check` | ok |
| `npm test` (19), `typecheck`, `tauri build --no-bundle` | ok |
| Paridad con el legacy (copia real) | 37/38. La diferencia restante es intencional |
| App real | La sugerencia mostró "1007 de 1160". "Ver en la lista" dejó 1 resultado por FTS en 2,3 ms, con la fila seleccionada y el detalle abierto. El hash de la copia no cambió |

Los benchmarks están en `docs/BENCHMARKS.md` (B-002).

### Pendientes trasladados
- Índice de título y ajuste del join del orden por código, en la primera migración (Fase 4).
- Ruta LIKE de 1–2 caracteres (~100 ms con 50k registros).
- Latencia de extremo a extremo en la UI.
- Compilar y probar en Linux.

## Fase 3 — Interfaz principal ✅ (2026-11-27)

**Implementación.** Un subagente Sonnet hizo toda la fase; el coordinador la revisó y corrigió el CSS de las tarjetas.
- **Backend:** comando `check_data_version` (`PRAGMA data_version`), que invalida la caché de `library_info` (ADR-010).
- **Barra lateral** con Biblioteca, Estado, Idioma y Etiquetas, con conteos y `aria-pressed`. Por debajo de 900 px pasa a ser un panel superpuesto ("☰ Filtros").
- **Galería virtualizada** (TanStack Virtual por filas y `ResizeObserver`) con portadas solo para las tarjetas visibles, caché LRU de 300 entradas y deduplicación de peticiones en curso. Navegación con flechas, Home, End y Enter.
- **Temas** Sistema, Claro, Oscuro y Noche, con contraste AA calculado. Preferencias en `localStorage`: `rpglib.theme`, `rpglib.view` y `rpglib.sidebar`.
- **Refresco ante cambios externos** al recuperar el foco y cada 30 s, con el aviso "Biblioteca actualizada".
- **Paleta de comandos** (Ctrl+K / Ctrl+Shift+P). "/" enfoca el buscador; Ctrl+1 y Ctrl+2 cambian de vista.
- Esqueletos de carga, estados vacíos, error con "Reintentar" y `prefers-reduced-motion`.

### Evidencias ejecutadas
| Verificación | Resultado |
|---|---|
| `cargo test` | 10 + 26 + 3 IPC (incluye detección de escritura externa): **39 ok** |
| clippy `-D warnings`, `fmt --check` | ok |
| `npm test` (36), `typecheck`, `tauri build --no-bundle` | ok |
| App real | Galería con portadas reales, barra lateral con conteos (961/153/46, 831/329, 3 tags), paleta → tema Noche aplicado, hash de la copia sin cambios |

### Pendientes trasladados
- Los conteos de la barra lateral son globales, no facetados.
- Un estado o idioma vacío no se puede filtrar.
- Rendimiento del scroll de la galería no medido.
- No verificado en Linux.

## Fase 4 — CRUD (en curso)
Diseño en `docs/CONTRACTS_PHASE4.md` y ADR-012. El usuario autorizó trabajar sobre `juegos.db` el 2026-11-27.
