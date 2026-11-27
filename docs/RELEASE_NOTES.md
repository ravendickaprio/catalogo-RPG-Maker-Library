# Notas de la versión 0.9.0 (candidata)

Primera versión de la aplicación de escritorio **RPG Maker Library** (Tauri 2 + Rust + React + SQLite). Es un catálogo local: sin nube, sin cuenta, sin telemetría y sin autoactualización. Los programas Python siguen intactos y compatibles.

## Novedades por fase
- **Fase 1 — Prototipo:** listado virtualizado, búsqueda con FTS5 (trigram) y respaldo por subcadena, filtros por estado e idioma, 6 órdenes, ficha con portada diferida y el diálogo **«Juego sugerido del día»** (aleatorio entre los juegos no terminados; solo lectura, sin historial).
- **Fase 2 — Búsqueda:** filtro por etiqueta, búsqueda en dos fases (sin leer BLOBs), plegado Unicode en la ruta por subcadena, coincidencia de códigos normalizados y paridad con el legacy (37/38 casos; la diferencia es intencional). Herramientas: DB sintéticas, `bench`, `parity`.
- **Fase 3 — Interfaz:** barra lateral con conteos, galería virtualizada con carga diferida de portadas, temas Sistema/Claro/Oscuro/Noche (contraste AA), paleta de comandos, detección de cambios hechos por otra aplicación.
- **Fase 4 — Alta, edición y baja:** formulario completo, aviso de código duplicado, control de conflictos por revisión, backups automáticos por sesión (rotación de 20), migraciones versionadas y modo solo lectura.
- **Fase 5 — Imágenes:** portada por selector, arrastrar, pegar o quitar; WebP 300×300 q65 como el legacy; protocolo `cover` en lugar de base64.
- **Fase 6 — Importador:** analizador de nombres corregido (el que arrastraba errores del importador Python), vista previa editable, duplicados, importación transaccional con backup, historial y deshacer. Nunca toca los archivos analizados.
- **Fase 7 — Mantenimiento:** 8 detectores, análisis de rutas cancelable, índice de búsqueda (comprobar/reconstruir), lotes con vista previa y deshacer.
- **Fase 8 — Complementos:** estadísticas, favoritos (etiqueta «Favorito»), revisión por tandas, ayuda de atajos, menú Herramientas.
- **Fase 9 — QA y distribución:** pantalla de elección de base en el primer inicio, lista/verificación/restauración de backups, comprobación de la base, exportación CSV/JSON, instaladores NSIS/MSI (Windows, generados en local) y paquetes Linux (deb/rpm/AppImage, mediante el flujo de CI, aún sin ejecutar), guía de Linux, flujo de CI, prueba de coexistencia con Python (`tools/interop_test.py`) y comparativa de rendimiento (`tools/bench_compare.py`, B-003 en `BENCHMARKS.md`).

## Compatibilidad
- Mismo esquema SQLite que `catalogo_rpgmaker-v04.py` y los editores; **sin WAL** (`journal_mode = delete`), sin cambios incompatibles en las tablas existentes.
- Los triggers de la base mantienen el índice de búsqueda aunque escriba una herramienta Python.
- Migraciones solo aditivas y opcionales (hoy: un índice por título).
- Probado con `tools/interop_test.py`: lo escrito por las sentencias de las apps Python lo lee la app y viceversa; con la base bloqueada, la otra parte recibe un error recuperable sin corrupción.
- Recomendación: no editar el mismo juego en las dos aplicaciones a la vez.

## Limitaciones conocidas
- **Linux no se ha compilado ni probado en una máquina real.** El flujo de CI y la guía `BUILD_LINUX.md` están preparados, pero no se han ejecutado. Pendiente una prueba manual en Nobara.
- Los instaladores de Windows **no están firmados** (aviso de SmartScreen probable).
- La aplicación no ignora acentos en la búsqueda (igual que Python). La ordenación por título distingue solo ASCII (`NOCASE`), con posibles diferencias de orden con acentos respecto a Python.
- Una búsqueda de 1–2 caracteres en bibliotecas sintéticas de 50 000 juegos tarda unos 135 ms (p50 medido en B-003), por encima del objetivo de 100 ms. Con la biblioteca real (1160 juegos) las búsquedas medidas están entre 2 y 4 ms.
- La latencia extremo a extremo (pulsación → pintado) y el rendimiento del scroll de la galería no se han medido; las cifras de `BENCHMARKS.md` son del SQL del backend.
- Un GIF animado conserva solo el primer fotograma al usarlo como portada.
- No se puede filtrar por estado o idioma vacío; los conteos de la barra lateral son globales.
- Las flujos de la interfaz (selector nativo de archivos, arrastrar y soltar) se verificaron con tests simulados y un recorrido manual en Windows; no en Linux.
- Sin importar desde CSV (a propósito, para evitar sobrescrituras).

## Antes de usarla con sus datos
Haga una copia de `juegos.db`. La app crea backups automáticos en `backups/`, pero una copia propia fuera de esa carpeta es la mejor garantía. Más detalles en `docs/MANUAL.md`.
