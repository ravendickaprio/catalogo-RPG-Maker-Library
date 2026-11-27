# Fase 0: auditoría verificada (2026-11-26)

Fuentes:
- Auditoría read-only realizada por un subagente Haiku sobre el código y sobre `dev-data/juegos.dev.db` (abierta en modo `ro`).
- Verificación independiente del coordinador sobre la misma copia.
- `juegos.db` no se abrió en ningún momento. La copia se hizo con la API `backup` de SQLite, con el origen abierto en `mode=ro`, y pasó `integrity_check = ok`.

Leyenda: **[F]** hecho verificado · **[H]** hipótesis · **[NM]** no medido.

## Contraste con `PROJECT_AUDIT.md`
| Afirmación preliminar | Veredicto | Evidencia |
|---|---|---|
| `_filtrar()` usa `listar()` y filtra/ordena en Python | **Confirmado** [F] | `catalogo_rpgmaker-v04.py` L981-1022 |
| Patrón N+1 | **Confirmado y peor de lo descrito** [F] | 2 queries por fila en `_filtrar` (L991-993) + 1 por fila al renderizar la lista (L1037). Son 3481 sentencias por refresco con N=1160 |
| Refresco sin debounce (`trace_add`) | **Confirmado** [F] | L776. Cada tecla ejecuta todo el pipeline |
| Galería e imágenes en el hilo de UI, sin virtualización | **Confirmado** [F] | `Galeria.mostrar` L449 destruye y recrea todas las tarjetas. `foto_miniatura` L1146 decodifica en el hilo de UI |
| Carga de BLOB innecesaria en listados | **Refutado** [F] | `listar()` excluye `imagen` |
| Búsqueda en título, código y descripción | **Parcial** [F] | También busca en página, tipo, tags y código normalizado |
| La DB no tiene FTS (implícito) | **Refutado** [F] | La DB tiene `juegos_busqueda`, `juegos_fts` (FTS5 `trigram`), `app_metadata` y 9 triggers. Ningún script del repo los crea ni los usa, tampoco en el historial de Git |

**Medición [F]** (patrón SQL de `_filtrar` replicado en Python, copia de 1160 filas, esta máquina, mediana de 5): 59,5 ms con búsqueda vacía. No es un benchmark de la app completa: no incluye el render de Tk.

## Schema real [F]
- `juegos(id PK AUTOINCREMENT, titulo NOT NULL, descripcion '', estado 'Sin empezar', idioma 'Español', imagen BLOB, ruta '', codigo '', pagina '', tipo 'Otro', puntuacion REAL 0)`
- `tags(id, nombre NOCASE UNIQUE)` y `juego_tags(juego_id, tag_id)` con PK compuesta y FK `ON DELETE CASCADE`. `foreign_keys` está desactivado por defecto en cada conexión.
- `juegos_busqueda`, `juegos_fts` (content table, trigram), `app_metadata{busqueda_schema=1}`.
- `user_version = 0`, `journal_mode = delete`, UTF-8. Sin índices propios.
- Un DDL sin datos, idéntico al real, está en `desktop/src-tauri/tests/fixtures/schema.sql`.

## Datos [F]
- Registros: 1160 juegos, 3 tags, 3 vínculos de tags.
- `estado`: `Sin empezar` 961, `Empezado` 46, `Terminado` 153. No hay variantes.
- `idioma`: `Japonés` 831, `Inglés` 329. Hay 0 `Español`, aunque es el valor por defecto del DDL.
- `pagina`: `ryuugames` en todos. `tipo`: RPG 1153, Otro 4, Visual Novel 2, 3d 1.
- Portadas: 249 registros, todas WebP, entre 3,2 y 27,8 KB.
- Descripción vacía: 913. Código vacío: 3. Sin duplicados exactos de código, pero hay **1 colisión de `codigo_norm`** (ids 124 y 1016, por un guion final).
- **906 títulos con forma `RY …`**: los produce un bug de offsets en `limpiar_titulo` del importador (`Importar_Juegos_RPG_Maker_v2.py` L57-60). Reproducido con nombres sintéticos [F]. Que el origen de estos registros sea el importador es [H].
- Rutas: 1158 usan `E:\…` y 1 `E:/…`. Son rutas de Windows, así que en Linux hará falta un mapeo de rutas (fases posteriores).

## Semántica de estados [F]
- Los únicos literales son `Sin empezar`, `Empezado` y `Terminado`. **Finalizado = `Terminado`.**
- El importador asigna `[F]`→Terminado y `[P]`→Empezado. Tiene falsos positivos con una `F` o `P` aislada.
- Juegos elegibles para la sugerencia: 1007, con 153 excluidos. Ver ADR-005.

## Riesgos principales
1. Hay objetos de búsqueda no documentados: cualquier migración debe preservarlos y comprobarlos al arrancar.
2. Trigram no encuentra nada con consultas de menos de 3 caracteres (`'ry'` → 0). Hace falta un fallback con LIKE (ADR-004).
3. Concurrencia con la app Python en modo rollback journal: el cambio a WAL necesita un ADR (Fase 2/4).
4. v04 sustituye estados, idiomas o tipos desconocidos por los valores por defecto al cargar y los sobrescribe al guardar. El CRUD en Rust no debe repetirlo.
5. v04 escribe sin backup. Los editores hacen un backup único que puede quedar obsoleto.
6. El archivo `juegos.db.backup-before-description-edit` no estaba ignorado por Git. **Se corrigió** con `.gitignore` (`*.db.backup*`, `dev-data/`, `config.json`).
7. Arreglar los 906 títulos requiere una migración versionada con backup y vista previa. No se corrige en silencio.

## Pruebas recomendadas (pendientes en fases posteriores)
- Paridad de búsqueda Rust vs `_filtrar` legacy (mismos ids).
- Interoperabilidad de escritura con los triggers y `integrity-check` de FTS.
- Concurrencia con `busy_timeout`.
- Golden tests del importador.
- Benchmarks sintéticos de 1k a 50k registros.
