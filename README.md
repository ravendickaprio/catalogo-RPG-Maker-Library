# RPG Maker Library

Aplicación de escritorio para organizar, clasificar y administrar bibliotecas de juegos RPG Maker y proyectos similares: portadas, códigos identificadores, estados, idiomas, páginas, tipos, puntuaciones y etiquetas personalizadas.

La **aplicación principal** es la de escritorio en `desktop/` (Rust + Tauri 2, React + TypeScript y SQLite) para Windows y Linux. La versión anterior en Python/Tkinter se conserva como **legacy** en `legacy/python/` y sigue funcionando sobre la misma base de datos.

## ✨ Características principales

- **Biblioteca:** vista de galería con tarjetas de portada o de lista, ambas virtualizadas para bibliotecas grandes.
- **Búsqueda rápida:** búsqueda parcial con FTS5, filtros por estado, idioma, página, tipo y etiquetas, y orden configurable.
- **Edición completa:** crear, editar y eliminar juegos, etiquetas y portadas (las portadas se convierten a WebP), con detección de cambios hechos por otras aplicaciones.
- **Juego sugerido del día:** elige al azar un juego no terminado, sin modificar nada.
- **Importador:** detecta juegos en carpetas (`.zip`, `.rar`, `.7z` y carpetas con código), con vista previa e historial para deshacer.
- **Centro de mantenimiento:** detecta datos incompletos, rutas inexistentes y etiquetas huérfanas, aplica correcciones en lote y permite deshacerlas.
- **Revisión por tandas, favoritos y estadísticas.**
- **Contraseñas por página:** guarda la contraseña de descompresión de cada página (por ejemplo, Ryuugames) en el almacén seguro del sistema, no en la base de datos. Puede consultarse y copiarse desde la ficha del juego.
- **Copias de seguridad:** copia automática antes de la primera escritura de cada sesión, y restauración y verificación de integridad.
- **Exportación** a CSV o JSON.
- **Interfaz:** tema oscuro moderno (además de Claro, Noche y Sistema), tipografía Fira, uso completo con teclado y paleta de comandos (`Ctrl+K`).
- **Local:** sin cuenta, nube ni telemetría.

## 🚀 Instalación y uso

Los instaladores (Windows: NSIS/MSI; Linux: `.deb`, `.rpm` y AppImage) se generan con el flujo de GitHub Actions o localmente. Al primer inicio, la app pide elegir el archivo `juegos.db`.

- Manual de usuario: [`docs/MANUAL.md`](docs/MANUAL.md)
- Compilar en Linux (Nobara/Fedora): [`docs/BUILD_LINUX.md`](docs/BUILD_LINUX.md)
- Desarrollo: [`desktop/README.md`](desktop/README.md)

```bash
cd desktop
npm install
npm run tauri dev       # desarrollo
npm run tauri build     # instaladores
```

## 🛠️ Tecnologías

- **Rust + Tauri 2:** backend, acceso a SQLite (`rusqlite`), procesamiento de portadas y almacén seguro (`keyring`).
- **React + TypeScript + Vite:** interfaz.
- **SQLite:** almacenamiento local, con el mismo esquema que la versión legacy y sin WAL, así que ambas pueden compartir la base.

## 📂 Estructura

| Ruta | Descripción |
|---|---|
| `desktop/` | Aplicación de escritorio principal (Tauri). |
| `docs/` | Arquitectura, decisiones (ADR), manual, benchmarks y notas de versión. |
| `tools/` | Herramientas de QA: interoperabilidad con Python, paridad y benchmarks. |
| `legacy/python/` | Aplicación Python legacy y sus herramientas. |
| `juegos.db` | Base de datos SQLite de la biblioteca (no se versiona). |

## 🗃️ Base de datos

Cada juego guarda:

- `id`: identificador interno.
- `titulo`: nombre del juego.
- `descripcion`: descripción.
- `estado`: Sin empezar, Empezado o Terminado.
- `idioma`: Español, Inglés o Japonés.
- `imagen`: portada almacenada en la base.
- `ruta`: ubicación del juego.
- `codigo`: identificador personalizado.
- `pagina`: fuente o página de procedencia.
- `tipo`: clasificación del juego.
- `puntuacion`: valoración de 0 a 5.

Las etiquetas se guardan en tablas relacionadas (`tags`, `juego_tags`). La app añade un índice de búsqueda regenerable (`juegos_busqueda` / `juegos_fts`) y migraciones versionadas que solo se aplican a petición y con copia previa. El archivo `.gitignore` excluye las bases de datos locales para que no se suban por accidente.

## 💾 Copias de seguridad

La app crea `backups/<db>.bak-AAAAMMDD-HHMMSS` junto a la base antes de la primera escritura de cada sesión, y conserva las 20 más recientes. Aun así, se recomienda guardar copias periódicas de `juegos.db` fuera del equipo. Las contraseñas por página no están en la base ni en sus copias: viven en el almacén de credenciales del sistema.

## 🧓 Versión legacy (Python)

La aplicación original en Python/Tkinter se conserva en `legacy/python/` como legado funcional. Usa el `juegos.db` de la raíz del repositorio y puede usarse a la vez que la app nueva (comprobado con `tools/interop_test.py`).

| Archivo | Descripción |
|---|---|
| `legacy/python/catalogo_rpgmaker-v04.py` | Catálogo Python v4 (Tkinter). |
| `legacy/python/herramientas/Importar_Juegos_RPG_Maker_v2.py` | Importador masivo antiguo. |
| `legacy/python/herramientas/catalogo_rpgmaker-descripciones.py` | Completar descripciones por tandas. |
| `legacy/python/herramientas/catalogo_rpgmaker-editor-completo.py` | Editor completo por tandas de 20 juegos. |

Requisitos: Python 3 con Tkinter y Pillow.

```bash
python legacy/python/catalogo_rpgmaker-v04.py
python legacy/python/herramientas/catalogo_rpgmaker-editor-completo.py
python legacy/python/herramientas/catalogo_rpgmaker-descripciones.py
python legacy/python/herramientas/Importar_Juegos_RPG_Maker_v2.py --carpeta "/ruta/a/tus/juegos" --db "/ruta/a/juegos.db"
```

- **Editor completo:** revisa tandas de hasta 20 juegos con descripción pendiente y permite editar todos los campos de la ficha. Crea una copia (`juegos.db.backup-before-description-edit`) antes del primer cambio.
- **Descripciones:** solo edita la descripción, con la misma copia previa.
- **Importador antiguo:** tiene errores conocidos (títulos «RY …», estados falsos, códigos cortados por `_`). Para importaciones nuevas, use el importador de la app de escritorio.

## 📄 Licencia

Consulta el archivo `LICENSE` para conocer los términos de uso y distribución del proyecto.
