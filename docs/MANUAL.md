# Manual de usuario — RPG Maker Library (versión de escritorio 0.9.0)

RPG Maker Library es un catálogo local de juegos de RPG Maker. Funciona sin conexión, sin cuenta y sin telemetría, y usa **la misma base de datos SQLite (`juegos.db`) que los programas Python** del repositorio.

## 1. Instalación

### Windows
1. Ejecute el instalador `RPG Maker Library_0.9.0_x64-setup.exe` (NSIS). Se instala **por usuario**, sin permisos de administrador. Al compilar en Windows también se genera un instalador `.msi`; el flujo de CI solo produce el de NSIS.
2. Abra «RPG Maker Library» desde el menú Inicio.

El instalador necesita WebView2, que ya viene con Windows 11 y con Windows 10 actualizado.

### Linux (Fedora / Nobara y derivados)
- **`.rpm`:** `sudo dnf install ./rpg-library-0.9.0-1.x86_64.rpm`
- **AppImage:** `chmod +x RPG-Maker-Library_0.9.0_amd64.AppImage && ./RPG-Maker-Library_0.9.0_amd64.AppImage`
- **`.deb`** (Debian/Ubuntu): `sudo apt install ./rpg-library_0.9.0_amd64.deb`

Los nombres exactos de archivo dependen de la herramienta de empaquetado. Para compilar desde el código fuente, vea `docs/BUILD_LINUX.md`. Los paquetes de Linux no se han probado todavía en una máquina real (ver `docs/RELEASE_NOTES.md`).

## 2. Primer inicio: elegir la base de datos
Si la aplicación no encuentra ninguna base, muestra la pantalla **«Elegir base de datos»**:
- pulse el botón para abrir el selector de archivos y elija su `juegos.db`, o
- escriba la ruta a mano.

La app comprueba que el archivo tenga el esquema del catálogo (tablas `juegos`, `tags`, `juego_tags` y las columnas esperadas). **No crea ni modifica nada** al abrirla para comprobarlo. La ruta queda guardada en los ajustes; puede cambiarla luego en **Ajustes**.

Orden de resolución de la ruta: variable de entorno `RPGLIB_DB_PATH` → ruta guardada en los ajustes → pantalla de elección.

## 3. Biblioteca, búsqueda y filtros
- **Barra de búsqueda:** busca en título, código, descripción, página, tipo y etiquetas. Con 3 o más caracteres usa el índice de texto (FTS5); con 1–2 caracteres usa una búsqueda por subcadena. Los códigos se comparan sin guiones, espacios ni guiones bajos (`RY-4762560`, `ry 4762560` y `RY4762560` coinciden). **No ignora acentos**, igual que los programas Python.
- **Esc** en el buscador borra el texto. **/** enfoca el buscador.
- **Barra lateral:** filtros por Estado, Idioma y Etiquetas, con conteos. Los conteos son globales (no cambian con la búsqueda actual). En ventanas estrechas (< 900 px) se abre con el botón «☰ Filtros».
- **Orden:** nombre A–Z/Z–A, código, estado, idioma y más recientes.
- **Vistas:** lista (Ctrl+1) y galería (Ctrl+2). Ambas están virtualizadas: solo se dibuja lo visible. Las consultas se han medido con 50 000 juegos sintéticos (`BENCHMARKS.md`); el desplazamiento de la galería con bibliotecas tan grandes no se ha medido. Las portadas se cargan solo para las tarjetas visibles.
- **Juego sugerido del día:** el botón «Juego sugerido del día» elige al azar un juego **no terminado** (estado distinto de «Terminado»). Solo lee: no cambia estado, puntuación ni ningún otro dato, y no guarda historial. Cada pulsación calcula una sugerencia nueva. «Ver en la lista» lo localiza en la biblioteca.
- **Temas:** Sistema, Claro, Oscuro y Noche.

## 4. Editar juegos y portadas
- Pulse un juego para abrir su ficha; **Editar** abre el formulario. **Ctrl+N** crea un juego nuevo.
- Campos: título, código, descripción, estado, idioma, página, tipo, puntuación (0–5 en pasos de 0,5), ruta y etiquetas. Si la base contiene valores que la app no conoce (por ejemplo otra página), se conservan.
- **Código duplicado:** si otro juego ya usa ese código, la app lo avisa y le deja guardar de todos modos.
- **Conflictos:** si otra aplicación modificó el mismo juego desde que lo abrió, la app lo detecta (control de «revisión») y le pide recargar en lugar de sobrescribir.
- **Base ocupada:** si otra aplicación está escribiendo, verá «La base de datos está ocupada por otra aplicación». Espere unos segundos y reintente.
- **Portada:** en el formulario puede elegir un archivo, arrastrarlo, pegar una imagen del portapapeles o quitarla. La imagen se reduce a un máximo de 300×300 y se guarda en WebP (calidad 65), igual que los programas Python. Un GIF animado conserva solo el primer fotograma.
- **Sin portada:** los juegos sin imagen muestran una imagen por defecto **solo en pantalla**; no se escribe nada en la base.
- **Eliminar:** pide confirmación. Quita el juego y sus vínculos de etiquetas.
- Todo se guarda con una transacción: o se guarda completo o no se guarda.

## 5. Favoritos
En la ficha, la ★ marca o desmarca el juego como favorito. Es la etiqueta reservada **«Favorito»**: la acción solo modifica los vínculos de etiquetas (no toca ningún otro campo). Se filtra desde la barra lateral o con la acción «Mostrar favoritos» de la paleta de comandos. Los programas Python lo ven como una etiqueta más.

## 6. Revisión por tandas
**Herramientas → Revisión por tandas** equivale al editor `catalogo_rpgmaker-editor-completo.py`, dentro de la app: toma al azar N juegos (20 por defecto) de una lista de pendientes (sin descripción, sin portada, sin etiquetas o con título «RY …»). Edita cada uno con el formulario completo; **Guardar y siguiente** es **Ctrl+Enter**. También hay «Anterior», «Omitir» y «Nueva tanda». Si hay cambios sin guardar, pide confirmación antes de salir.

## 7. Importador
**Herramientas → Importar…** (o la paleta de comandos) da de alta juegos desde una carpeta de descargas.
1. Elija la carpeta (selector o ruta escrita) y pulse Analizar. Hay progreso y botón Cancelar.
2. **Qué hace el analizador:** recorre los nombres de archivos y carpetas y deduce de cada nombre el **código** (`RY-…`), el **estado** (una `F` o `P` dentro de corchetes, p. ej. `[F]`, `[Demo P]`), el **idioma** y un **título**. Detecta duplicados contra la base (por código normalizado) y dentro del propio lote.
3. **Nunca modifica, mueve, renombra ni abre los archivos analizados:** solo lee los nombres.
4. La vista previa es una tabla editable: puede corregir campos, filtrar, desmarcar filas y fijar la página y el tipo por defecto.
5. Al confirmar, la app hace un **backup previo** y guarda todo en una sola transacción. Se muestra un resumen (importados, omitidos, avisos).
6. La pestaña **Historial** permite **«Deshacer última importación»**: borra solo lo importado que no se haya modificado después.

En modo solo lectura se puede analizar, pero no importar ni deshacer.

> **No use el importador antiguo** (`Importar_Juegos_RPG_Maker_v2.py`) con datos nuevos: tiene errores conocidos (títulos «RY …», estados falsos, códigos cortados por `_`). El de la app los corrige.

## 8. Centro de mantenimiento
**Herramientas → Mantenimiento** muestra un resumen y listas paginadas de **detectores**: sin descripción, sin portada, sin etiquetas, códigos duplicados o vacíos, títulos «RY …», estados inconsistentes, etiquetas huérfanas, rutas inexistentes (comprobación cancelable; no accede a rutas de red) y estado del **índice de búsqueda** (con botón de reconstruir).

**Acciones por lote** con vista previa (puede desmarcar filas): fijar un campo (estado, idioma, tipo, página), añadir o quitar una etiqueta y proponer títulos a partir de los «RY …». Cada lote hace un backup previo y queda en el historial; **Deshacer** restaura solo lo que no se haya modificado después.

Nota: las rutas de Windows (`C:\…`) y de Linux (`/home/…`) se marcan como «ruta de otro sistema» cuando se abre la misma base en el otro sistema operativo.

## 9. Estadísticas
**Herramientas → Estadísticas**: cobertura (descripción, portada, etiquetas, ruta), barras por estado, idioma, tipo, página, puntuación y etiquetas, y una tabla estado × idioma. Al pulsar una barra se filtra la biblioteca. Solo lee; no hay actividad por fecha porque el esquema no guarda fechas.

## 10. Copias de seguridad, restauración y exportación
- **Backup automático:** antes de la **primera escritura de cada sesión** (y antes de importaciones, lotes y migraciones) la app copia la base con la API de backup de SQLite en la carpeta **`backups/` junto a la base de datos**, con nombre `juegos.db.bak-<fecha>`. Se conservan los 20 más recientes.
- **Ajustes → Copias de seguridad:** lista los backups, permite **verificarlos** (`integrity_check` y esquema) y **restaurar**. Al restaurar, la app crea antes un backup «pre-restauración» de la base actual. Solo se aceptan archivos de la carpeta `backups/`. Si otra aplicación tiene la base bloqueada, la restauración falla sin cambiar nada.
- **Comprobar la base:** ejecuta `integrity_check`, `foreign_key_check` y la comprobación del índice FTS, en solo lectura.
- **Exportar** (Herramientas o paleta): guarda la biblioteca en **CSV** (UTF-8 con BOM, se abre bien en Excel) o **JSON**. Solo exporta metadatos (id, título, código, estado, idioma, tipo, página, puntuación, ruta, descripción, etiquetas, si tiene portada); **no incluye las imágenes**. No puede escribir sobre la propia base ni sobre `backups/`. No hay «importar desde CSV» a propósito.

## 11. Ajustes
- **Ruta de la base de datos:** cambiarla recarga la biblioteca.
- **Modo solo lectura:** abre la base sin permitir ninguna escritura. Recomendado para consultar mientras otra herramienta trabaja.
- **Migraciones:** la app puede aplicar mejoras de esquema versionadas (hoy, un índice por título y `ANALYZE`). Son **opcionales y aditivas**: no alteran las tablas existentes, se hacen en una transacción con backup previo y comprobación de integridad antes de confirmar; los programas Python siguen funcionando igual. Si no las aplica, la app funciona igual (solo más lenta al ordenar por título en bibliotecas enormes).

## 12. Atajos de teclado
La lista completa está en la app (**`?`** o **F1**). Resumen:

| Atajo | Acción | Ámbito |
|---|---|---|
| Ctrl+K / Ctrl+Shift+P | Abrir la paleta de comandos | Global |
| / | Ir al buscador | Global (no al escribir) |
| Ctrl+N | Nuevo juego | Global |
| Ctrl+1 / Ctrl+2 | Vista lista / galería | Global |
| ? / F1 | Mostrar los atajos | Global |
| Ctrl+S | Guardar el formulario | Formulario |
| Esc | Cancelar la edición | Formulario |
| Ctrl+Enter | Guardar y pasar al siguiente | Revisión por tandas |
| ↑ ↓ Inicio Fin Enter | Moverse por la lista; Enter abre el detalle | Lista |
| ← → ↑ ↓ Enter | Moverse por la galería; Enter abre el detalle | Galería |

Los atajos globales no actúan mientras hay un diálogo abierto. En macOS, Ctrl equivale a Cmd.

## 13. Convivencia con las herramientas Python
La app usa **el mismo esquema** que `catalogo_rpgmaker-v04.py`, `catalogo_rpgmaker-editor-completo.py` y `catalogo_rpgmaker-descripciones.py`, y **no usa WAL** (la base sigue en `journal_mode = delete`), así que ambos mundos pueden compartir el archivo.

**Es seguro:**
- Leer con una mientras la otra está abierta; la app detecta los cambios externos al recuperar el foco y cada 30 s («Biblioteca actualizada por otra aplicación»).
- Editar con las dos en momentos distintos; las escrituras de cada una son breves y por transacción.
- Usar las etiquetas, estados y puntuaciones: se leen igual en ambas.
- Los favoritos son una etiqueta más («Favorito») en las apps Python.

**Evite:**
- **Editar el mismo juego en ambas a la vez.** Gana la última escritura; la app detecta el conflicto, las apps Python no.
- Mantener un formulario Python abierto «con cambios» mucho rato mientras trabaja con la app: si ambas escriben a la vez, la que llegue segunda verá «base ocupada» (la app) o «database is locked» (Python) y puede reintentar.
- **No use el importador Python** (ver §7).
- No active WAL en la base mientras use las herramientas Python.

Verificado con `tools/interop_test.py`: las sentencias de escritura de los programas Python son leídas correctamente por la app, y las de la app por las consultas de Python; con la base bloqueada por una escritura, la otra parte recibe un error recuperable y no hay corrupción.

## 14. Dónde se guardan los datos
| Qué | Dónde |
|---|---|
| Base de datos | donde usted la elija (`juegos.db`) |
| Backups automáticos | carpeta `backups/` junto a la base |
| Ajustes (`settings.json`) | directorio de configuración de la app |
| Historial de importaciones y deshacer de lotes | directorio de datos de la app |

El identificador de la app es `local.rpgmakerlibrary.desktop`. Por convención de Tauri 2:
- **Windows:** `%APPDATA%\local.rpgmakerlibrary.desktop` (configuración y datos).
- **Linux:** configuración en `~/.config/local.rpgmakerlibrary.desktop`, datos en `~/.local/share/local.rpgmakerlibrary.desktop` (convención, no verificado en un equipo real).

Las preferencias visuales (tema, vista, barra lateral, pantalla de bienvenida) se guardan en el almacenamiento local del navegador interno de la app (WebView). Desinstalar la app no borra su base ni sus backups.

## 15. Solución de problemas
| Síntoma | Qué hacer |
|---|---|
| «Elegir base de datos» en cada inicio | Elija el `juegos.db` real; compruebe que la ruta existe y que tiene permiso de lectura. |
| «No es una base válida / falta el esquema» | El archivo no es un `juegos.db` del catálogo. Elija otro. |
| «La base de datos está ocupada por otra aplicación» | Cierre o espere a la otra herramienta (editor, importador) y reintente. |
| «El juego fue modificado por otra aplicación» | Recargue la ficha y vuelva a aplicar su cambio. |
| «Base en solo lectura» | Desactive el modo solo lectura en Ajustes, o compruebe que el archivo no es de solo lectura en el sistema. |
| Búsqueda que no encuentra un juego recién cambiado desde Python | Mantenimiento → índice de búsqueda: compruebe y reconstruya. (Los triggers de la base lo actualizan solos si se escribió con las herramientas habituales.) |
| La app va lenta ordenando por título con una biblioteca enorme | Ajustes → aplicar migraciones (crea un índice). |
| Una restauración o lote salió mal | Use **Deshacer** (lotes e importaciones) o restaure un backup desde Ajustes. |
| Linux: no abre el selector de archivos | Compruebe que están instalados GTK 3 y `xdg-desktop-portal`. Puede escribir la ruta a mano. |

Si algo falla, **no borre la carpeta `backups/`**: es su red de seguridad.
