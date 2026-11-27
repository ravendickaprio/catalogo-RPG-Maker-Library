# Auditoría técnica inicial

## Contexto
La biblioteca RPG Maker está implementada en Python/Tkinter con SQLite. La versión estable de referencia es `catalogo_rpgmaker-v04.py`. La auditoría inicial se basa en la inspección previa del código; se deberá confirmar cada hallazgo en la copia actual del repositorio y registrar cualquier discrepancia. No se ejecutaron benchmarks en esta auditoría.

## Hallazgos de desempeño y UX

### 1. Filtrado y consultas
El flujo de filtrado (`_filtrar()` en la versión auditada) parte de obtener el listado general (`self.db.listar()`), y luego realiza filtrado y ordenamiento en Python. Se observó también acceso adicional a datos de etiquetas y campos asociados a registros (página, tipo, puntuación), lo que puede generar un patrón de consultas N+1. Con colecciones grandes, esto incrementa el trabajo y el tiempo de respuesta.

**Recomendación:** mover filtrado, ordenamiento, búsqueda y paginación al SQL parametrizado; diseñar consultas para obtener solo los datos visibles; revisar índices y medir.

### 2. Búsqueda reactiva
El buscador actual conecta cambios del texto (`trace_add`) a refresco de la lista. La ejecución frecuente de refrescos sin debounce y en el hilo de UI puede provocar lentitud y bloqueos perceptibles.

**Recomendación:** debounce (objetivo inicial 150 ms, ajustable por benchmark), backend asíncrono, cancelación lógica o descarte de resultados obsoletos, y UI no bloqueante.

### 3. Galería
La reconstrucción de tarjetas, carga/procesamiento de imágenes y creación de elementos para resultados puede ser costosa cuando se muestra una colección extensa.

**Recomendación:** virtualización, lazy loading, cache de miniaturas, consultas por páginas y no renderizar registros fuera de pantalla.

### 4. Imágenes
Guardar imágenes como BLOB en SQLite no es inherentemente incorrecto, pero cargar/decodificar muchas imágenes durante el render puede afectar el tiempo de respuesta y el uso de memoria.

**Recomendación:** separar metadatos de carga de imágenes, cache de miniaturas regenerable, límites de memoria, lazy loading y procesamiento fuera del hilo principal.

### 5. Interfaz y Tkinter
Tkinter permite mantener la aplicación portable, pero limita la flexibilidad para una experiencia moderna y compleja, animaciones, layouts, virtualización y consistencia visual.

**Recomendación:** migración gradual a Tauri 2 con frontend moderno, manteniendo backend local.

## Arquitectura recomendada
- Rust: operaciones de datos, búsqueda, importación, validaciones, imágenes, backups y migraciones.
- Tauri 2: contenedor de escritorio local, API controlada.
- React + TypeScript: interfaz.
- SQLite: persistencia local.
- SQLx (o alternativa Rust evaluada) para SQLite y migraciones; documentar la decisión.
- FTS5: búsqueda textual; validar la estrategia de búsquedas parciales/subcadena, porque el tokenizer estándar no cubre todos los casos. Evaluar trigramas o estrategia complementaria.
- TanStack Virtual: listas/grillas virtualizadas.
- Tailwind y componentes accesibles (por ejemplo shadcn/ui) como opción, no requisito inamovible.

## Objetivos iniciales de desempeño (targets, no resultados)
Estos objetivos deberán evaluarse con benchmarks reproducibles, equipos y DB documentados:
- Búsqueda simple: objetivo <= 100 ms.
- Búsqueda combinada: objetivo <= 150 ms.
- Cambios de filtros: objetivo <= 100 ms.
- Abrir ficha: objetivo <= 100 ms.
- Presentación inicial de galería: objetivo <= 300 ms.
- Scroll fluido: objetivo cercano a 60 FPS en escenarios objetivo.

## Benchmarks
Evaluar bases de prueba de 1k, 5k, 10k, 25k y opcional 50k registros. Medir:
- arranque y carga inicial;
- búsquedas simples, combinadas y parciales;
- filtros, ordenamientos y paginación;
- apertura de detalle;
- galería y scroll;
- RAM, CPU, carga/decodificación de imágenes;
- importación, backup y restore.

Registrar entorno, sistema operativo, hardware, tamaño y composición de DB, metodología y resultados. Comparar con Python solo si se puede reproducir la medición en condiciones razonablemente equivalentes.

## Interfaz de producto sugerida
- Sidebar: biblioteca, estados, etiquetas, colecciones y favoritos.
- Buscador global y filtros rápidos.
- Grilla/lista virtualizada.
- Panel de detalles contextual.
- Dashboard de métricas.
- Tema claro, oscuro y otros temas seleccionables.
- Accesibilidad, navegación por teclado y command palette.
- Centro de mantenimiento: descripciones, portadas, etiquetas faltantes, códigos duplicados, rutas inválidas.
- Importador integrado: carpetas/archivos, vista previa, detección de duplicados, progreso, cancelación, errores e historial.

## Riesgos y mitigaciones
- **Compatibilidad DB:** analizar schema y valores reales, usar copia, migraciones versionadas, backup y pruebas de integridad.
- **FTS y coincidencias parciales:** definir requisitos de búsqueda, evaluar estrategia y testear.
- **Tauri/WebView:** probar instaladores y dependencias en Windows y Nobara/Fedora (WebView2 en Windows y WebKitGTK en Linux, según configuración).
- **Carga de imágenes:** probar DB con imágenes, límites de cache y fallback.
- **Alcance:** migrar por fases, mantener Python estable.
- **Rendimiento percibido:** medir UI y DB por separado.

## Conclusión
La prioridad no es sustituir Python por otro lenguaje por sí sola, sino corregir la arquitectura de consulta/renderizado y modernizar UX. Rust + Tauri 2 ofrece un equilibrio adecuado para una app local multiplataforma, sujeto a prototipo y validación.

## Verificación (Fase 0, 2026-11-26)
Los hallazgos de este documento se contrastaron contra el código y una copia de la DB. Ver `docs/PHASE0_AUDIT_VERIFIED.md`. En resumen: los puntos 1 a 3 se confirman (el N+1 es incluso mayor). La carga de BLOB en listados queda refutada. Y la DB **ya contiene FTS5 trigram** con triggers, algo que esta auditoría no recogía.
