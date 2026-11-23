# RPG Maker Library

Una aplicación de escritorio para organizar, clasificar y administrar bibliotecas de juegos RPG Maker y otros proyectos similares. Permite mantener una colección organizada mediante portadas, códigos identificadores, estados, idiomas, categorías y etiquetas personalizadas.

## ✨ Características principales

- **Gestión de biblioteca:** agrega, edita y elimina juegos de tu colección.
- **Galería y lista:** consulta tu biblioteca en diferentes formatos de visualización.
- **Portadas:** almacena imágenes de los juegos directamente en la base de datos SQLite.
- **Búsqueda y filtros:** encuentra juegos por título, código, estado, idioma, página o tipo.
- **Clasificación personalizada:** organiza los juegos mediante etiquetas.
- **Información detallada:** registra descripción, ruta, código identificador y puntuación.
- **Estadísticas:** consulta información general de tu colección.
- **Temas visuales:** interfaz con modos Claro, Oscuro y Noche.
- **Personalización:** opciones de apariencia y compatibilidad con Nerd Fonts.

## 📥 Importador masivo de juegos

Incluye un script independiente para importar juegos existentes a la biblioteca sin tener que registrarlos manualmente uno por uno.

### Funcionalidades

- **Detección automática:** busca recursivamente archivos `.zip`, `.rar` y `.7z`, además de carpetas cuyo nombre contenga un código identificador RY.
- **Interpretación de nombres:** intenta identificar automáticamente el título, código, estado e idioma del juego.
- **Detección de estados:**
  - `[F]`: Terminado.
  - `[P]`: Empezado.
  - Sin marca reconocida: Sin empezar.
- **Detección de idiomas:**
  - `EN`: Inglés.
  - `ES` o `ESP`: Español.
  - Sin marca de idioma reconocida: Japonés.
- **Prevención de duplicados:** evita importar códigos RY y rutas que ya estén registrados en la base de datos.
- **Vista previa:** muestra los juegos detectados y la información que se utilizará antes de confirmar la importación.
- **Resumen de resultados:** informa cuántos registros se agregaron, cuántos se omitieron por duplicados y cuántos no tenían un código RY.

### Consideraciones

- Solo importa elementos que tengan un código identificador RY reconocido.
- Los registros nuevos se crean con la descripción y la portada vacías, para completarlas posteriormente desde el catálogo.
- No extrae archivos comprimidos.
- No mueve, renombra ni modifica los archivos originales.
- Utiliza la misma base de datos `juegos.db` del catálogo.

### Ejecución

```bash
python Importar_Juegos_RPG_Maker_v2.py
```

También permite indicar las rutas directamente:

```bash
python Importar_Juegos_RPG_Maker_v2.py --carpeta "/ruta/a/tus/juegos" --db "/ruta/a/juegos.db"
```

## 🛠️ Tecnologías

- **Python:** lenguaje principal.
- **Tkinter / ttk:** interfaz gráfica.
- **SQLite:** almacenamiento local de información.
- **Pillow:** procesamiento de imágenes.
- **WebP / JPEG:** formatos utilizados para optimizar las portadas.

## 📂 Archivos principales

| Archivo | Descripción |
|---|---|
| `catalogo_rpgmaker-v04.py` | Aplicación principal del catálogo. |
| `Importar_Juegos_RPG_Maker_v2.py` | Herramienta independiente para importar juegos existentes. |
| `juegos.db` | Base de datos SQLite de la biblioteca. |
| `config.json` | Preferencias y configuración visual. |

## 🗃️ Base de datos

La aplicación utiliza SQLite para almacenar la información de la colección.

Entre los datos principales de cada juego se encuentran:

- `id`: identificador interno.
- `titulo`: nombre del juego.
- `descripcion`: descripción.
- `estado`: Sin empezar, Empezado o Terminado.
- `idioma`: Español, Inglés o Japonés.
- `imagen`: portada almacenada en la base de datos.
- `ruta`: ubicación del juego.
- `codigo`: identificador personalizado.
- `pagina`: fuente o página de procedencia.
- `tipo`: clasificación del juego.
- `puntuacion`: valoración de 0 a 5 estrellas.

También utiliza tablas relacionadas para administrar etiquetas personalizadas.

## 💾 Copias de seguridad

Se recomienda realizar copias periódicas de `juegos.db`, ya que contiene la información de la biblioteca y las portadas almacenadas.

## 🖥️ Plataformas

El proyecto está desarrollado en Python y utiliza tecnologías multiplataforma, con el objetivo de funcionar en Windows y Linux.

## 🚧 Desarrollo

Este proyecto está en desarrollo. Se irán incorporando mejoras y nuevas funcionalidades para facilitar la administración de colecciones grandes de juegos.

## 📄 Licencia

Consulta el archivo `LICENSE` para conocer los términos de uso y distribución del proyecto.
