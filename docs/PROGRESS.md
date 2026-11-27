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

## Fase 4 — CRUD ✅ (2026-11-27)

**Implementación.** Dos subagentes Sonnet hicieron el backend y el frontend en paralelo, sobre `CONTRACTS.md` (sección Fase 4). El coordinador aplicó `cargo fmt` a los tests y verificó todo.
- **Backend:**
  - módulos `settings`, `backup`, `migrate` y `write`;
  - 10 comandos nuevos;
  - errores `Conflict`, `DuplicateCode`, `ReadOnly` y `Busy`;
  - revisión FNV-1a por registro y normalización NFC;
  - backup automático por sesión con rotación de 20;
  - migración v1 con el ajuste del orden por código (ADR-013).
- **Frontend:**
  - formulario de edición y alta con editor de tags y vocabulario que conserva valores desconocidos;
  - flujos de código duplicado, conflicto, base ocupada y solo lectura;
  - aviso de cambios sin guardar;
  - borrado con confirmación;
  - diálogo de Ajustes (ruta de la DB, solo lectura, backup y migraciones);
  - atajos Ctrl+N y Ctrl+S, y acciones en la paleta.

### Evidencias ejecutadas
| Verificación | Resultado |
|---|---|
| `cargo test` | 21 unitarios + 26 de repo + 18 de escritura + 6 IPC: **71 ok** |
| clippy `-D warnings`, `fmt --check` | ok |
| `npm test` (54), `typecheck`, `tauri build --no-bundle` | ok |
| Interoperabilidad con el legacy (copia temporal) | Las consultas de v04 (`listar`, `obtener`, `tags_juego`, `titulo_con_codigo`) leen bien lo escrito desde Rust. `integrity_check` y el `integrity-check` de FTS dan ok; `juegos_busqueda` queda igual a `juegos` |
| App real (copia desechable) | Ctrl+N → formulario con valores por defecto → código `ry_1092000` → aviso "ya lo tiene «Naked Story Uncensored»" → guardar de todos modos → id 1161, conteos actualizados, aviso "Juego creado", FTS encuentra el juego. Backup automático `backups/crud_test.db.bak-…` con 1160 registros (estado previo) e `integrity_check` ok |

### Pendientes
- Edición de portadas (Fase 5).
- Las tags huérfanas se conservan (igual que el legacy).
- No se probaron en la ventana real los flujos de conflicto ni de "base ocupada" (sí están cubiertos por tests de integración).
- No verificado en Linux.

## Fase 5 — Imágenes ✅ (2026-11-27)

**Implementación** (subagente Sonnet, full-stack; diseño en el ADR-014):
- `images::process_cover`: magic bytes, límites de decodificación, alfa aplanado sobre blanco, máximo 300×300 con Lanczos3 y WebP q65 (libwebp), igual que el legacy.
- Comandos `set_cover` (bytes en bruto por IPC más cabeceras de id y revisión) y `remove_cover`, sobre el mismo camino de escritura segura.
- Protocolo `cover` con caché inmutable e invalidación por etiqueta; ya no se usa base64 en la UI.
- Editor de portada: selector de archivo, arrastrar y soltar, pegar y quitar.
- La revisión incluye ahora una huella de la portada (longitud y primeros/últimos 16 bytes).

### Evidencias ejecutadas
| Verificación | Resultado |
|---|---|
| `cargo test` | 31 + 7 IPC + 26 + 21: **85 ok** (1 bench ignorado) |
| clippy, fmt, `npm test` (61), typecheck, `tauri build` | ok |
| App real (copia desechable) | La galería carga las portadas por `cover://` bajo la CSP. Pegar una imagen de 600×800 desde el portapapeles guarda la portada al instante y la tarjeta se refresca. En la DB queda `RIFF…WEBPVP8 `, 225×300, 1522 bytes; backup previo creado; `integrity_check` ok |
| Tiempos de `process_cover` (release, imágenes sintéticas) | JPEG 2000×1000 → 44,9 ms; PNG 4000×3000 → 165,8 ms |

### Pendientes
- Arrastrar y soltar y el selector de archivo nativo no se probaron en la ventana real (sí con tests).
- Un GIF animado conserva solo el primer fotograma.
- `get_cover` queda sin uso en la UI.

## Fase 6 — Importador ✅ (2026-11-27)

**Implementación.** Se retomó el WIP `f38577f`: el backend compilaba, pero el parser rendía peor que el legacy con los datos reales. Trabajaron un auditor Haiku (revisión independiente del backend), un Sonnet para el backend (parser, arreglos y tests de integración) y otro Sonnet para el frontend. Después, cuatro revisores de limpieza (reutilización, simplificación, eficiencia y altitud) y un Sonnet que aplicó lo aprobado. El coordinador validó y documentó. Contrato en `CONTRACTS.md`, sección Fase 6.
- **Parser:**
  - el `_` termina el código;
  - el estado sale de una palabra F/P dentro de corchetes (`[P bug]`, `[Demo F]`);
  - el idioma se busca en el nombre completo.
- **Backend:**
  - análisis cancelable con progreso por `Channel`;
  - vista previa con duplicados contra la DB y dentro del lote;
  - importación en una transacción con revalidación y backup;
  - historial JSONL **ligado a la DB abierta**;
  - deshacer serializado con el bloqueo de escritura, que solo borra lo que no cambió;
  - `write::delete_game_row` compartido;
  - se añade `tauri-plugin-dialog` con solo `dialog:allow-open`.
- **Frontend:**
  - diálogo «Importar…» (botón y paleta) con selector de carpeta o ruta escrita;
  - progreso y botón Cancelar;
  - tabla editable (virtualizada a partir de 300 filas) con filtros y valores por defecto de página y tipo;
  - confirmación y resumen con omitidos, backup y avisos;
  - pestaña Historial con «Deshacer última importación»;
  - en modo solo lectura se puede analizar, pero no importar ni deshacer.

### Evidencias ejecutadas
| Verificación | Resultado |
|---|---|
| `import_validate` (copia de la DB real, 1159 filas), legacy / antes / ahora | estado 1158 / 1142 / **1156**; idioma 1069 / 1065 / **1069**; código normalizado 1136 / 1056 / **1136**. Solo quedan 3 diferencias intencionales (ver CONTRACTS) |
| Añadir `ENG` como inglés (descartado) | idioma bajaba de 1069 a 1047: la DB guarda la mayoría de los `_ENG` como Japonés. Se respeta el criterio existente |
| `cargo test` | 34 + 12 import + 7 IPC + 26 + 21: **100 ok** (1 bench ignorado) |
| clippy `-D warnings`, `fmt --check`, `npm test` (68), typecheck, `tauri build --no-bundle` | ok |
| Extremo a extremo sobre una copia de la DB real (API del backend, carpeta con nombres sintéticos) | 3 nuevos, 1 duplicado por código real y 1 sin código. Valores por defecto `ryuugames`/`RPG`. Inserta los ids 1161–1163 con backup previo; FTS los encuentra; `integrity_check` ok. `sqlite3` de Python lee las filas, y `juegos_busqueda` coincide con `juegos`. Deshacer borra 3 y vuelven a quedar 1160. El hash de `juegos.db` no cambia |

### Pendientes
- No se probó en la ventana real (selector nativo, progreso y tabla virtualizada), porque esta sesión no controla la GUI. Los flujos están cubiertos con tests simulados y extremo a extremo por la API.
- `cancel_scan` no se asocia a un análisis concreto: un clic tardío podría cancelar el siguiente.
- La comprobación de duplicados en la importación hace una consulta por elemento, aceptable hasta unos miles de elementos.
- No verificado en Linux (el plugin de diálogo usa GTK mediante `rfd`).

## Identidad visual (2026-11-27, petición del usuario)
- **Icono:** los iconos de Tauri se generan desde `imgs/icon` (WebP 1024 → PNG; solo tamaños de escritorio). Logo de 28 px en la cabecera.
- **Bienvenida:** pantalla de inicio con `imgs/portada.jpg` (WebP de 108 KB). No bloquea la carga, muestra el estado o el error de la biblioteca, tiene la preferencia `rpglib.welcome` («Mostrar al iniciar») y se reabre desde la acción «Pantalla de bienvenida» de la paleta.
- **Imagen por defecto:** `imgs/default_image.jpg` (WebP de 300 px y 10 KB) se muestra en la interfaz para los juegos sin portada o con una portada que falla al cargar. **No se escribe en la DB.** `CoverImage` deja de recibir un placeholder distinto en cada sitio.
- Los originales en `imgs/` no están versionados; solo las copias optimizadas en `desktop/src/assets`.

## Fase 7 — Centro de mantenimiento ✅ (2026-11-27)

**Implementación.** El coordinador escribió el contrato (`CONTRACTS.md`, sección Fase 7). Un Sonnet hizo el backend y otro el frontend, en paralelo. Un Haiku hizo la auditoría independiente y un tercer Sonnet aplicó las correcciones aprobadas. El coordinador hizo la verificación y la prueba de extremo a extremo.
- **Backend (`maintenance.rs`, 12 comandos):**
  - 8 detectores paginados, sin BLOBs;
  - análisis de rutas cancelable (sin rutas de red ni relativas, y sin el bloqueo de la DB mientras toca el disco);
  - tags huérfanas;
  - estado y reconstrucción del índice de búsqueda;
  - lotes con vista previa y revisiones (fijar campo, añadir o quitar tag, proponer títulos `RY`);
  - registros de deshacer por DB en `app_data_dir/undo/`;
  - deshacer serializado, que solo restaura lo que no cambió;
  - guarda de análisis compartida con el importador.
- **Frontend (`MaintenanceDialog`):**
  - resumen con conteos;
  - listas por detector («Cargar más», virtualizadas);
  - acciones por lote con vista previa desmarcable fila a fila;
  - historial de lotes y deshacer;
  - cola de descripciones («Guardar y siguiente», Ctrl+Enter, «Saltar»);
  - modo solo lectura;
  - protección contra respuestas obsoletas y doble envío.

### Evidencias ejecutadas
| Verificación | Resultado |
|---|---|
| `cargo test` | **131 ok** (31 de mantenimiento), 1 ignorado |
| clippy `-D warnings`, `fmt --check`, `npm test` (93), typecheck, `tauri build --no-bundle` | ok |
| Extremo a extremo sobre una copia de la DB real (API del backend) | **Resumen en 43 ms:** 913 sin descripción, 911 sin portada, 1157 sin tags, 2 códigos duplicados, 3 vacíos, 906 títulos `RY`, 2 estados inconsistentes. **Índice:** 1160/1160, 0 desajustes, FTS ok. **Rutas:** 13 inexistentes, comprobadas en 4,1 s. **Lote** `add_tag` con las revisiones serializadas como en JSON: 3 cambios con backup; deshacer restaura los 3; `integrity_check` ok e índice intacto |
| `fix_ry_titles` con datos reales | De 500 títulos `RY`, solo 2 propuestas, y las dos eran ruido («R18», «Fix1»). Se descartan las propuestas de una palabra de ≤ 4 caracteres y la vista previa es desmarcable |

### Pendientes
- No se probó en la ventana real (la sesión no controla la GUI); cubierto con tests simulados y extremo a extremo por la API.
- La cola de descripciones no recoge los juegos que se queden sin descripción mientras el diálogo está abierto.
- No verificado en Linux.

