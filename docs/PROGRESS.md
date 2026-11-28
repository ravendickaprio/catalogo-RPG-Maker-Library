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


## Fase 8 — Funciones complementarias ✅ (2026-11-27, ADR-017)

**Implementación.** El coordinador escribió el ADR-017 y el contrato. Un Sonnet hizo el backend (`get_dashboard`) y otro el frontend, en paralelo. Un Haiku hizo la auditoría independiente y un tercer Sonnet aplicó las correcciones aprobadas. El coordinador hizo la verificación y las pruebas sobre una copia de la DB real.
- **Estadísticas:**
  - `get_dashboard`, de solo lectura y sin BLOBs;
  - en el diálogo: cobertura en %, barras por estado, idioma, tipo, página, puntuación (11 tramos) y tags, y una tabla estado × idioma;
  - al pulsar una barra se filtra la biblioteca;
  - sin «actividad», porque el esquema no tiene fechas.
- **Favoritos:** tag reservada `Favorito` y comando `set_favorite`, que **solo toca `juego_tags`** y comprueba la revisión. Se marcan con ★ en la ficha, se filtran desde la barra lateral y desde la paleta, y las apps Python los ven como una tag más.
- **Revisión por tandas:** equivale a `catalogo_rpgmaker-editor-completo.py` dentro de la app.
  - Toma N juegos aleatorios (20 por defecto) de un detector: sin descripción, sin portada, sin tags o título `RY`.
  - Se editan con el formulario completo y la portada.
  - Tiene «Guardar y siguiente» (Ctrl+Enter), «Anterior», «Omitir» y «Nueva tanda», y avisa de los cambios sin guardar.
  - Sustituye a la cola de descripciones de la Fase 7; Mantenimiento enlaza a ella.
- **Atajos:** una tabla única (`lib/shortcuts.ts`) alimenta el manejador de teclado y la ayuda (`?` o F1). Los atajos globales no actúan con un diálogo modal abierto.
- **Menú «Herramientas»** en la cabecera: Importar, Mantenimiento, Estadísticas y Revisión por tandas.

### Evidencias ejecutadas
| Verificación | Resultado |
|---|---|
| `cargo test` | **137 ok**, 1 ignorado |
| clippy `-D warnings`, `fmt --check`, `npm test` (119, dos ejecuciones), typecheck, `tauri build --no-bundle` | ok |
| `get_dashboard` sobre una copia de la DB real (1160 juegos) | 5,6–7,4 ms. Estados: 961 / 153 / 46. Idiomas: 831 / 329. Con portada 249, con descripción 247, con tags 3, con ruta 1159 |
| `get_dashboard` en el bench sintético | 5 000 filas: p50 30 ms; 50 000 filas: p50 312 ms (unas 5 veces `library_info`; aceptable porque se carga una vez por diálogo) |
| `set_favorite` sobre una copia de la DB real | Fila idéntica byte a byte (incluida la huella de la portada). La tag aparece y FTS encuentra «Favorito». Al quitarlo vuelve a quedar idéntica; `integrity_check` ok |

### Pendientes
- No se probó en la ventana real (la sesión no controla la GUI).
- No verificado en Linux.

## Prueba de la interfaz real (2026-11-27)
**Método.** Se lanzó el ejecutable de release contra una **copia** de la DB (`RPGLIB_DB_PATH`), con el perfil de WebView2 en una carpeta temporal (`WEBVIEW2_USER_DATA_FOLDER`), y se controló la ventana por CDP (`--remote-debugging-port` en 127.0.0.1) con `playwright-core`, instalado fuera del repo. Se hicieron capturas en cada paso y se registraron los errores de consola. Al terminar se borró la carpeta `Roaming\local.rpgmakerlibrary.desktop` creada por la prueba (solo contenía el historial de la importación de prueba). El hash de `juegos.db` no cambió.

**Recorrido OK:**
- bienvenida;
- galería con la imagen por defecto y tema Noche;
- búsqueda (15 resultados en 2,8 ms);
- ficha, editar y Escape;
- favorito activado, filtro «Mostrar favoritos» y favorito desactivado;
- `?` abre la ayuda (y no se abre al escribir en el buscador);
- estadísticas con clic para filtrar;
- mantenimiento y comprobación de rutas (13);
- Ctrl+N bloqueado con un diálogo abierto;
- revisión por tandas (Ctrl+Enter guarda y avanza; confirmación si hay cambios sin guardar);
- importador: analizar, importar con backup, historial y deshacer;
- juego sugerido y Ajustes.

**0 errores de consola.**

**Defectos encontrados y corregidos (verificados de nuevo en la ventana real):**
1. Escape no cerraba un diálogo cuando el foco caía en `body` (un botón que se desactiva). `useDialog` escucha ahora en `document`; solo reacciona el diálogo superior y devuelve el foco al diálogo.
2. La cabecera ocupaba dos filas a 1280 px. Ahora ocupa una (37 px): «Comandos» pasa a ser un icono y el nombre de la DB se trunca con elipsis.
3. Las barras de estadísticas se escalaban al máximo de cada lista (una tag con el 0,1 % aparecía llena). Ahora se escalan respecto al total, sin barra cuando el valor es 0, y el tiempo sale como «10,8 ms».
4. Fechas del historial en inglés → formato `es-ES` («27/11/26, 14:25»), con un formateador compartido en `lib/format.ts`.
5. En la revisión por tandas los botones de guardar quedaban fuera de la vista; ahora la barra de acciones es fija abajo.
6. Aparecía el aviso «Biblioteca actualizada» tras una acción propia. **Causa real:** `App.tsx` usaba ese mismo texto en el `onChanged` de Importar y de Mantenimiento; se comprobó que las escrituras propias, el backup y el deshacer no cambian `PRAGMA data_version`. Se descartó la ventana de supresión de 2 s propuesta, que partía de una premisa falsa y podía ocultar cambios externos reales. Ahora los textos son distintos: «Cambios aplicados» para las acciones propias y «Biblioteca actualizada por otra aplicación» para los cambios externos.

**Evidencias:** `npm test` 125 ok, typecheck ok y build de release ok.

## Fase 9 — QA y distribución ✅ (2026-11-27, ADR-018)

**Implementación.** El coordinador escribió el ADR-018 y el contrato. Tres Sonnet trabajaron en paralelo (backend, frontend, y herramientas con documentación). Un Haiku hizo la auditoría independiente y un Sonnet aplicó las correcciones aprobadas. El coordinador hizo la verificación, la prueba en la ventana real y corrigió dos fallos encontrados en las pruebas.
- **Primer inicio:** en release ya no hay ruta compilada de desarrollo; sin base se muestra «Elegir base de datos».
- **Copias de seguridad:**
  - `list_backups`, `verify_backup`, `restore_backup` y `check_database`;
  - restaurar = verificar → copia «pre-restauración» → API de backup sobre la conexión abierta, con el estado bloqueado hasta reabrir (aviso si la reapertura falla);
  - las copias pre-restauración rotan aparte (se conservan 5);
  - crear un backup o restaurar desisten con `Busy` a los 3 s en vez de esperar indefinidamente.
- **Exportación CSV/JSON:**
  - sin BLOBs; CSV con BOM, neutralización de fórmulas y escritura atómica;
  - se rechaza escribir sobre la DB o en `backups/`, incluso a través de enlaces simbólicos;
  - permiso `dialog:allow-save`.
- **Distribución:**
  - versión 0.9.0 en los tres manifiestos;
  - instaladores NSIS (por usuario) y MSI;
  - `docs/BUILD_LINUX.md`;
  - `.github/workflows/build.yml` (Ubuntu deb/AppImage, Fedora rpm, Windows NSIS; `permissions: contents: read`; **no se ha subido ni ejecutado**).
- **Herramientas:**
  - `tools/interop_test.py` (coexistencia con Python, con `examples/interop_probe.rs`);
  - `tools/bench_compare.py` (B-003).
- **Documentación:** `MANUAL.md`, `RELEASE_CHECKLIST.md`, `RELEASE_NOTES.md`.

**Fallos encontrados en las pruebas y corregidos por el coordinador:**
- Con la DB bloqueada en exclusiva por otra app, abrirla daba `Config` («No es una base válida») en lugar de `Busy`. Ahora `db::invalid_db` mapea el bloqueo a `Busy`; test `open_while_exclusively_locked_is_busy_not_invalid`.
- Node 26 (instalado en el equipo durante la sesión) expone un `localStorage` experimental que tapaba el de jsdom: fallaban los 140 tests del frontend. `vite.config.ts` pasa ahora `--no-experimental-webstorage` a los workers de test.

### Evidencias ejecutadas
| Verificación | Resultado |
|---|---|
| `cargo test` | **152 ok** (14 de backup), 1 ignorado |
| clippy (`--all-targets --examples -D warnings`), `fmt --check`, typecheck, `npm test` | ok; **141** tests del frontend |
| `python -I tools/interop_test.py` | **36/36 PASS**: escritura cruzada Python ↔ Rust en los dos sentidos; Python `BEGIN IMMEDIATE` y `EXCLUSIVE` → la app devuelve `Busy` en unos 5–7 s sin corromper; journal `delete`; sin `-wal`/`-shm` |
| `tools/bench_compare.py` (B-003, Ryzen 7 5700X) | p50 con 50 000 juegos: búsqueda vacía 4644 ms (legacy) frente a 74 ms (app); «the» 3198 frente a 52,7 ms; «ry» 3489 frente a 134,6 ms (por encima del objetivo de 100 ms) |
| `npx tauri build` | `RPG Maker Library_0.9.0_x64-setup.exe` (5,08 MiB) y `…_x64_en-US.msi` (10,77 MiB) |
| Ventana real (release, sin base configurada) | Pantalla de primer inicio; ruta errónea → aviso; abrir la copia → biblioteca y bienvenida; crear y listar backups (fecha en `es-ES`, 4,8 MB); «Verificar base actual» ok (1160 juegos); restaurar con confirmación → 1160 juegos y copia pre-restauración; `verify_backup` ok; nombres `..\x`, `../x`, `C:\x` y vacío → `InvalidInput`; exportación CSV y JSON de 1160 filas (BOM, 12 columnas); exportar sobre la DB → `InvalidInput`; 0 errores de consola. Se borraron el `settings.json` y la carpeta `Roaming\local.rpgmakerlibrary.desktop` creados por la prueba; el hash de `juegos.db` no cambió |

### Pendientes
- **Linux sin verificar:** no hay WSL en el equipo. El flujo de CI está listo, pero subirlo necesita la aprobación del usuario; falta la prueba manual en Nobara.
- La ruta de fallo al reabrir tras restaurar no tiene test (requeriría inyectar el fallo).
- Las búsquedas de 1–2 caracteres con 50 000 juegos tardan unos 135 ms, por encima del objetivo de 100 ms.
- Los instaladores no están firmados: Windows SmartScreen puede avisar al ejecutarlos. La firma de código requiere un certificado y se deja para cuando se decida publicar.

## Fase 10 — Recuperación de metadatos desde Ryuugames (2026-11-27, ADR-021)
Rama `feature/ryuugames-metadata`. Herramienta independiente en `tools/ryuugames/` (Python + Tkinter + Pillow). No toca `desktop/`.

**Implementado:**
- filtro `RY-` + `R` antes de cualquier solicitud;
- búsqueda por la API REST de WordPress (una solicitud por juego, con la portada incluida mediante `_embed`);
- validación conservadora;
- limpieza del título y descripción de 400 caracteres como máximo;
- portada en WebP 300×300 q65, como el legacy, con caché local;
- sesión reanudable;
- backup con la API de SQLite + `integrity_check`;
- aplicación en una transacción `BEGIN IMMEDIATE` que vuelve a comprobar que la descripción sigue vacía;
- informes TXT/CSV y log;
- GUI con dry run, cancelación, progreso y selección por fila.

### Evidencias ejecutadas
| Verificación | Resultado |
|---|---|
| `py -3.14 -m unittest discover -s tools/ryuugames` | **27 ok**, sin red, con DB sintética |
| Inspección de `juegos.db` (solo lectura) | 1161 juegos, 776 sin descripción, 621 elegibles, 155 excluidos, 0 inválidos |
| Búsqueda real de 9 códigos (3 RA, 1 basura, 5 RJ) sobre una **copia** | 7 verificadas, 1 dudosa (`RA25078923`: el código solo aparece en el contenido), 1 no encontrada (`PR`); 16 solicitudes en 46 s con intervalo de 3 s; ningún bloqueo |
| Aplicar las 7 sobre la copia | cambian exactamente 7 filas, con 7 portadas (13–16 KB); el resto de columnas y `juego_tags` sin cambios; `integrity_check` ok; FTS actualizado por los triggers |
| GUI (instanciada sin interacción, sobre la copia) | contadores, tabla, detalle y marcado correctos |

**Fallos encontrados en la prueba real y corregidos:**
- `PR` se verificaba por coincidir con «**pr**epare» (el límite de palabra solo cubría dígitos). Ahora se exige el límite también con letras y los códigos sin dígitos nunca se verifican.
- Dos códigos tenían dos entradas (japonés e `[ENG]`) y quedaban como dudosos. Ahora se elige la única `[ENG]`.

**Auditoría independiente (agente Haiku, solo lectura) y correcciones:**
- *Corregidos:*
  - códigos cortos (`R1`) se podían verificar: ahora se exigen ≥5 dígitos;
  - un título autorizado se reemplazaba aunque se hubiera editado entretanto: ahora se exige que no haya cambiado;
  - el error de `BEGIN` quedaba oculto por el `ROLLBACK`;
  - `IncompleteRead` no se reintentaba;
  - se podía cerrar la ventana durante «Aplicar»;
  - se aceptaban portadas por `http`;
  - un título formado solo por etiquetas se proponía como título;
  - no se reconocía `src` con comillas simples.
- *Parcialmente aceptado:* la descripción ya no se corta en una imagen inicial, pero sí en una imagen posterior al texto o en «LINK DOWNLOAD». Al dejar de cortar en las imágenes, dos entradas reales incluían «LINK DOWNLOAD ( … MB )», que a veces es un `<span>`.
- *Encontrado al ampliar las pruebas:* con la DB bloqueada por otra app, `Connection.backup()` reintentaba sin límite y «Aplicar» se colgaba. Ahora se toma primero el bloqueo (5 s como máximo) y se hace el backup con una conexión de solo lectura.
- *No aplicado:* `fsync` de la sesión. Si el JSON se corrompe, se informa al abrir la DB y la DB no se ve afectada.

**Ejecución real final** sobre una copia nueva: 3 códigos (2 verificadas aplicadas con portada, 1 dudosa sin tocar). El backup conserva el estado previo y el hash de `juegos.db` no cambió.

### Pendientes
- Ejecución completa sobre `juegos.db` (unos 621 códigos, entre 40 y 60 minutos). La decide el usuario desde la GUI.
- Sin probar en Nobara/Linux. Solo se usan la stdlib, Tkinter y Pillow.
- Algunas descripciones unen líneas separadas por `<br>` sin puntuación, porque así vienen en el original.
