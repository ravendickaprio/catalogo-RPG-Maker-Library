# Criterios de aceptación

## Integridad y compatibilidad
- El prototipo utiliza una DB de prueba/copia, no la DB original.
- Esquema real documentado; IDs, tags, estados, ratings, idiomas, códigos, páginas, tipos, rutas e imágenes preservados.
- Integridad SQLite comprobada.
- Migraciones documentadas, versionadas y probadas sobre copia.
- Backups y procedimiento restore documentados.
- No hay cambios no autorizados a legacy/main.

## Búsqueda y desempeño
- SQL parametrizado y filtros en backend.
- Sin patrón N+1 en listado principal.
- Listado paginado/virtualizado y evita cargar BLOB de portadas innecesariamente.
- Debounce y resultados obsoletos manejados.
- FTS y búsqueda parcial/code probadas con casos definidos.
- Benchmarks reproducibles con entorno y tamaño de DB registrados.
- Targets del audit son metas; reportar valores medidos y no ocultar desviaciones.

## Juego sugerido del día
- Selecciona aleatoriamente entre juegos no finalizados conforme a estados reales auditados.
- No cambia ningún dato ni estado al mostrar.
- Muestra datos del registro y portada si está disponible.
- Maneja biblioteca vacía y todos los registros finalizados.
- Pruebas que confirman elegibilidad y ausencia de escritura en DB.

## UX
- Interfaz moderna, responsive, teclado y accesible.
- Lista/grilla virtualizada.
- Carga diferida de imágenes.
- Estados de carga, error y vacío.
- Temas previstos.

## Plataforma
- Build y ejecución documentadas para Windows y Nobara/Fedora.
- Se identifican dependencias de WebView/compilación.
- No se requiere conexión a Internet para las funciones del catálogo.

## Calidad
- Tests unitarios/integración donde proceda.
- Lint/format y build ejecutados.
- Reporte claro de resultados, errores y pruebas pendientes.
- No se afirma una prueba que no se haya ejecutado.
