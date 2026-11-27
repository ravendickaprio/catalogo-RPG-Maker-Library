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

## Prioridad
No adelantar fases de lujo antes de que búsqueda, lectura, integridad de datos, rendimiento, sugerencia aleatoria y CRUD sean sólidos. Opus podrá reordenar tareas justificando en `docs/DECISIONS.md`.
