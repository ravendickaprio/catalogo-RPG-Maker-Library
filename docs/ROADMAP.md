# Roadmap de modernización

Todas las fases se desarrollan en `feature/desktop-modernization-tauri`. No se modifica main directamente. Cada fase debe producir código ejecutable, pruebas, documentación y commit local significativo cuando proceda. No hacer push, PR ni merge sin autorización.

## Fase 0 — Auditoría y plan verificado
- Haiku inspecciona repo, DB schema, app, scripts, dependencias, estados y manejo de imágenes.
- Documenta mapa de funciones y riesgos.
- Opus compara con auditoría preliminar, actualiza arquitectura y propone tareas.
- Crear rama, comprobar working tree y registrar baseline.
- No tocar DB real ni código legacy salvo que sea estrictamente documental y aprobado.

## Fase 1 — Prototipo tecnológico
- Crear desktop Tauri 2 + React + TypeScript + Rust.
- Confirmar builds de desarrollo Windows/Linux.
- Selección explícita de una copia de la DB de prueba.
- Listado simple, apertura de detalles básicos y búsqueda inicial.
- Comando “Juego sugerido del día” (read-only).
- Instrumentación de benchmark básica.
- Documentar decisiones de tecnología y entorno.

## Fase 2 — Motor de búsqueda
- Repository SQLite y migraciones.
- SQL parametrizado, filtros, sort, paginación.
- Índices, FTS5 y estrategia de substring/code.
- Debounce, ejecución asíncrona, descarte de resultados obsoletos.
- Tests y benchmarks.
- Comparar query plans.

## Fase 3 — Interfaz principal
- Layout moderno: sidebar, buscador, filtros rápidos.
- Lista/grilla virtualizada.
- Panel de detalles.
- Temas claro/oscuro y configuración.
- Navegación por teclado, accesibilidad, estados vacíos y errores.
- Command palette y shortcuts pueden incluirse en esta fase si no distraen del MVP.

## Fase 4 — CRUD
- Crear, editar, eliminar (con confirmación).
- Campos: título, descripción, estado, idioma, ruta, código, página, tipo, puntuación, tags, imágenes, según schema real.
- Validación, transacciones y pruebas.
- Preservar IDs y compatibilidad cuando sea viable.

## Fase 5 — Imágenes
- Lazy loading.
- Cache de thumbnails.
- Cache limitada.
- Fallbacks.
- Optimización y procesamiento sin bloquear UI.
- Evaluar storage actual; no convertir o eliminar imágenes originales sin protección.

## Fase 6 — Importador
- Importación desde carpetas y archivos/archive según capacidad verificada.
- Vista previa, duplicate detection, reglas configurables.
- Progreso, cancelación, errores e historial.
- Operación segura, sin sobrescritura silenciosa.
- Tests con fixture data.

## Fase 7 — Centro de mantenimiento
- Detectar registros sin descripción, portada o tags.
- Detectar códigos duplicados, rutas inválidas y problemas de consistencia.
- Integrar flujo de edición por lotes.
- Cambios con preview y undo/backup cuando sea viable.

## Fase 8 — Funciones complementarias
- Dashboard: conteos por estado, idioma, tipo, página, actividad, etc.
- Colecciones y favoritos, si se implementan sin complicar el MVP.
- Command palette y shortcuts.
- UX refinada.

## Fase 9 — QA y distribución
- Windows y Nobara/Fedora.
- Instaladores y documentación.
- Backup, restore e integridad SQLite.
- Pruebas de regresión e import/export.
- Benchmarks comparativos reproducibles.
- Validar coexistencia con app Python.
- Checklist release, notas de versión y manual.

## Fase 10 — Recuperación de metadatos desde Ryuugames
Requerimiento: `docs/Nuevo_requerimiento.md`. Decisiones: ADR-021. Rama propia: `feature/ryuugames-metadata` (desde `main`). Herramienta Python independiente en `tools/ryuugames/`. No toca la app Rust/Tauri.

### Auditoría inicial (2026-11-27, `juegos.db` abierta en solo lectura, medido)
- Tabla `juegos`: 1161 registros, **776 sin descripción** (todos con `RY-`), **621 elegibles** y **155 excluidos** por no tener `R`. Ningún código vacío entre ellos. Solo hay un código duplicado, `''`, en 3 registros que no son candidatos.
- `imagen` es un **BLOB WebP** (≈300×300, calidad 65; media 13,9 KB) y no una ruta. 618 de los 621 elegibles no tienen imagen. La lógica de «ruta de imagen inexistente» del requerimiento no aplica.
- 596 elegibles tienen título provisional `RY <número>`.
- `journal_mode=delete` (no WAL). Los triggers `juegos_busqueda_au` → `juegos_fts_au` actualizan FTS al cambiar `titulo`/`descripcion`, así que no hay que tocar otras tablas.
- Patrones de los elegibles: `RJ…` 587, `RA…` 27, `R…` 2, y 5 códigos basura (`CSAR`, `DRU`, `HRH`, `NTRP`, `PR`) que la regla de la `R` deja pasar. La validación los clasificará como «No encontrado».
- Ryuugames: `/?s=` responde **403 (desafío de Cloudflare)**, así que el buscador HTML no se puede usar sin eludir la protección. La **API REST pública de WordPress** (`/wp-json/wp/v2/posts?search=`) responde 200 con `link`, `slug`, `title`, `content` y `featured_media`. `robots.txt` no prohíbe nada. Una búsqueda de prueba (`RJ01389169`) devolvió la entrada correcta, con el bloque `DESCRIPTION` en `content` y la portada como `<img class="wp-image-{featured_media}">`.

### Tareas
1. **Núcleo sin red** (`core.py`): candidatos (`descripcion` NULL/vacía/espacios), filtro `RY-` + `R` sin distinguir mayúsculas, clasificación excluido/inválido, validación de coincidencia, extracción de descripción (texto tras `DESCRIPTION` hasta el siguiente encabezado, sin HTML ni entidades, ≤400 caracteres cortando por palabra), detección de título provisional.
2. **Cliente HTTP** (`urllib` de stdlib): una búsqueda REST por código con `_fields`, intervalo configurable (3 s por defecto), timeout, 2 reintentos con espera, y parada inmediata ante 403/429/503 o un desafío de Cloudflare. Sin concurrencia. Los códigos excluidos nunca llegan al cliente.
3. **Imagen**: descarga del `src` de mayor tamaño del `srcset` de `wp-image-{featured_media}`, caché local determinista `<carpeta DB>/ryuugames_cache/<código>.<ext>`, validación con Pillow y conversión a WebP 300×300 q65 (como el legacy).
4. **Sesión y reanudación**: `ryuugames_sesion.json` junto a la DB con el resultado de cada código. Al relanzar se recalculan los candidatos desde la DB y se saltan los códigos ya resueltos (opción «reintentar no encontrados/dudosos»).
5. **Aplicar**: backup con `sqlite3.Connection.backup()` en `backups/juegos.db.bak-<fecha>-ryuugames` + `integrity_check` del backup. Una transacción `BEGIN IMMEDIATE`, `UPDATE … WHERE id=? AND (descripcion IS NULL OR trim(descripcion)='')`. La imagen solo se escribe si está vacía y el título solo si es provisional o el usuario marcó «reemplazar título». No se toca ningún otro campo.
6. **GUI Tkinter** (`recuperar_ryuugames.py`): seleccionar DB, contadores, intervalo, Buscar (dry run), Cancelar, barra de progreso, tabla de previsualización con casillas (solo las verificadas son seleccionables), Aplicar seleccionados.
7. **Informes**: resumen final con todos los contadores del §16 y CSV por registro, en la carpeta de la DB. Log de errores.
8. **Pruebas** (`unittest`): DB sintética + HTML/JSON de ejemplo y cliente HTTP falso. Cubren los 23 puntos del §18, sobre todo «excluido ⇒ 0 solicitudes», «no encontrado/dudoso ⇒ fila idéntica byte a byte» y «otros campos intactos».
9. **Auditoría independiente** (agente Haiku) del filtro, la validación, la seguridad SQLite y el parsing. Corrección (agente Sonnet). Revisión final y prueba real con 5 a 10 códigos sobre una **copia** de la DB.
10. README, `requirements.txt` (solo `Pillow`), entrada en `docs/PROGRESS.md`, commit local.

### Criterios de salida
Se cumplen los criterios del §21 del requerimiento, las pruebas pasan en Windows, la ejecución real sobre la copia está documentada con cifras medidas, y la DB real solo se modifica cuando el usuario pulsa «Aplicar».

## Prioridad
No adelantar fases de lujo antes de que búsqueda, lectura, integridad de datos, rendimiento, sugerencia aleatoria y CRUD sean sólidos. Opus podrá reordenar tareas justificando en `docs/DECISIONS.md`.
