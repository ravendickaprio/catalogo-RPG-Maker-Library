# Instrucciones de Claude Code — RPG Maker Library

## Misión
Modernizar gradualmente la aplicación `legacy/python/catalogo_rpgmaker-v04.py` para Windows y Linux, con prioridad en velocidad de búsqueda, rendimiento, UX moderna, operación local y preservación de datos, usando Rust + Tauri 2, React + TypeScript y SQLite.

## Equipo y coordinación
- **Claude Opus 5 (coordinador/arquitecto):** define arquitectura, divide tareas, asigna agentes, mantiene el plan, revisa interfaces entre componentes, resuelve conflictos, valida resultados y toma decisiones. No delegues decisiones arquitectónicas críticas sin revisar.
- **Agente Haiku (lector/auditor):** inspección de código, esquema y datos; documenta flujos, dependencias, riesgos, compatibilidad y recomendaciones. Por defecto, no modifica la aplicación ni la DB.
- **Agente Sonnet (implementador):** desarrolla tareas delimitadas por el arquitecto, respetando contratos, pruebas y convenciones. No cambia el esquema, interfaces o arquitectura transversal unilateralmente.
- Si las herramientas disponibles no permiten ejecutar modelos/agentes con las identidades indicadas, informa la limitación y utiliza las capacidades reales disponibles. No afirmes haber delegado si no ocurrió.

## Reglas de seguridad y Git
1. `main` es la versión estable y no debe modificarse directamente.
2. Crear/usar `feature/desktop-modernization-tauri` para el proyecto nuevo.
3. No borrar, renombrar, reescribir ni reemplazar el programa Python estable durante las fases de prototipo.
4. No utilizar la DB original para pruebas, benchmarks, migraciones o importación experimental. Copiarla y verificar que el prototipo opere sobre la copia.
5. No ejecutar operaciones destructivas sobre datos. Toda migración debe ser versionada, transaccional cuando proceda, con copia de seguridad y verificación de integridad.
6. No publicar commits, abrir PRs, hacer merges, releases o subir archivos de datos personales sin autorización explícita del usuario.
7. Antes de cambios importantes, explicar alcance y plan; usar commits pequeños y significativos, pero no pedir aprobación para cada acción rutinaria dentro del alcance autorizado.
8. No inventar pruebas, resultados, benchmarks, agentes, archivos, ni capacidades. Separar resultados medidos de objetivos.
9. Revisar el estado Git antes de editar y respetar los cambios del usuario. No sobrescribir archivos modificados.
10. No descargar ni incluir juegos, portadas con copyright, datos personales ni la base real en commits.

## Proceso de desarrollo
- Comenzar con auditoría Haiku, inspección de Git y estructura; validar el diagnóstico del documento `docs/PROJECT_AUDIT.md`.
- Opus aprueba arquitectura, interfaces, prioridades y descomposición de tareas.
- Sonnet implementa tareas acotadas; Haiku puede realizar revisiones independientes, análisis de riesgos y verificaciones.
- Opus revisa cambios, ejecución, tests, seguridad, regresiones y coherencia.
- Mantener `docs/` actualizado, con decisiones (ADR cuando corresponda), pruebas y avances.
- En cada fase, entregar resumen, archivos afectados, pruebas ejecutadas, pendientes, riesgos y commit(s) locales realizados.

## Compatibilidad y datos
- Respetar el esquema existente, IDs, códigos, estados, etiquetas, páginas, tipos, puntuaciones, rutas, descripciones e imágenes, después de verificarlo.
- No asumir el esquema de SQLite: inspeccionarlo directamente.
- Preservar la interoperabilidad con la aplicación Python si es viable; documentar cambios incompatibles.
- Preferir migraciones explícitas y reversibles o estrategias de exportación/rollback.
- Evitar bloqueos del hilo de UI; las consultas y operaciones pesadas deben ser asíncronas.
- La app debe funcionar localmente y no requerir nube, cuenta, telemetría ni servicio externo.

## Alcance de “Juego sugerido del día”
- Añadir un botón/acción visible en la interfaz.
- Seleccionar al azar un registro cuyo estado NO indique terminado/finalizado. Primero inspeccionar y documentar la semántica real de estados.
- La selección se ejecuta localmente y no modifica datos: no cambia estado, puntuación, fecha, historial ni otros campos.
- Mostrar información suficiente del juego (por ejemplo título, portada disponible, idioma, tipo, código y descripción) sin forzar cambios de estado ni abrir automáticamente su carpeta.
- Si no hay candidatos, mostrar un mensaje claro y no alterar la DB.
- Determinar si la selección se vuelve a calcular al pulsar; el nombre “del día” no autoriza almacenar una fecha ni historial. En MVP, acción de sugerencia aleatoria en cada pulsación salvo que se acuerde otra especificación.
- Respetar filtros/estado del catálogo según el diseño aprobado; definir la semántica antes de implementar.

## UI y rendimiento
- Diseño moderno, accesible, responsive y con teclado.
- Virtualización para listas y galerías; carga diferida de imágenes.
- Consultas SQL filtradas, paginadas y parametrizadas; nunca recuperar toda la biblioteca para filtrar en memoria sin justificación.
- FTS5 y estrategia de búsqueda parcial deben probarse contra datos reales anonimizados o copias.
- Debounce, consultas asíncronas y manejo de resultados obsoletos.
- Nunca cargar BLOBs de portada en consultas que no los necesiten.
- Medir; no afirmar mejoras sin benchmark reproducible.

## Seguridad de ejecución
- No exponer API, shell, archivos, comandos o SQL arbitrarios al frontend.
- Limitar comandos Tauri a funciones de aplicación autorizadas; validar entradas.
- No utilizar `unsafe` en Rust sin justificación, revisión y documentación.
- Auditar permisos/plugins Tauri y dependencias.
- No implementar autoactualización, telemetry, networking remoto ni servicios en segundo plano sin aprobación.

## Definition of Done
Consultar `docs/ACCEPTANCE_CRITERIA.md` y `docs/ROADMAP.md`. Ninguna fase se considera concluida por tener únicamente código generado: debe compilar y tener pruebas/evidencias apropiadas, con pendientes identificados.
