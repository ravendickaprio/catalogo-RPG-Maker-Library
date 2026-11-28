# Arquitectura objetivo

## Principios
Local-first, seguridad, rapidez de consultas, accesibilidad, mantenimiento, portabilidad Windows/Linux, preservación de datos, módulos testeables y migración incremental.

## Stack propuesto
- Rust y Tauri 2.
- React + TypeScript.
- SQLite.
- SQLx (propuesta; evaluar y documentar).
- FTS5 para búsqueda de texto y estrategia complementaria para substring/código según pruebas.
- TanStack Virtual para listas y galerías.
- Tailwind CSS y componentes accesibles; la selección de toolkit debe documentarse.

## Límites entre capas
### Frontend
UI, estado visual, navegación, presentación y validación superficial. Acceso a datos mediante comandos Tauri tipados. No abrir SQLite directamente.

### Comandos Tauri
API explícita y limitada, con validación, DTOs, errores tipados y permisos mínimos. No exponer shell, SQL arbitrario ni acceso irrestricto a archivos.

### Servicios Rust
Casos de uso: búsqueda, filtros, CRUD, juego sugerido, importación, imágenes, backups y validación de datos.

### Persistencia
Repositorio SQLite con consultas parametrizadas, índices, transacciones, migrations y tests. No consultar portada BLOB en listados generales.

## Modelo de consulta
- filtros y sort SQL;
- paginación/limit;
- debounce en frontend;
- operaciones async;
- resultados obsoletos descartados;
- índices revisados a partir de query plans y benchmarks;
- FTS y coincidencias parciales documentadas.

## Imágenes
- metadatos y miniaturas de manera desacoplada;
- carga bajo demanda;
- cache regenerable;
- virtualización;
- límites de memoria y fallback;
- no transformar originales de forma destructiva.

## “Juego sugerido del día”
Comando local de aplicación para elegir un juego elegible al azar cuyo estado no sea terminado. Debe documentarse la interpretación de estados y excluir finalizados con una condición SQL verificada. Mostrar un modal, panel o tarjeta con los datos disponibles. Operación de lectura: no cambiar estado, rating, tags, historial ni datos. Si la biblioteca no tiene candidatos, presentar estado vacío.

## Posible estructura
```text
repository/
├── juegos.db                   # base real (no versionada); la usan la app y el legacy
├── legacy/python/              # app Python legacy (catalogo_rpgmaker-v04.py)
│   └── herramientas/           # importador y editores por tandas legacy
├── desktop/
│   ├── src/
│   │   ├── components/
│   │   ├── views/
│   │   ├── hooks/
│   │   ├── services/
│   │   ├── stores/
│   │   └── styles/
│   ├── src-tauri/
│   │   ├── src/
│   │   │   ├── commands/
│   │   │   ├── database/
│   │   │   ├── search/
│   │   │   ├── images/
│   │   │   ├── importer/
│   │   │   └── main.rs
│   │   └── migrations/
│   └── package.json
├── docs/
│   ├── architecture/
│   ├── benchmarks/
│   └── migration/
└── README.md
```
Esta estructura es orientativa. No reubicar ni modificar archivos estables sin necesidad, y revisar estructura existente antes de adoptarla.
