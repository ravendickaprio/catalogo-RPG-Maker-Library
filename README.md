# 🎮 RPG Maker Library

**Un catálogo de escritorio para organizar, clasificar y administrar tu colección de juegos RPG Maker.**

RPG Maker Library es una aplicación de escritorio desarrollada en Python que permite gestionar bibliotecas de juegos mediante una interfaz visual, personalizable y respaldada por SQLite.

Organiza tus juegos, conserva sus portadas, registra su progreso, clasifícalos mediante etiquetas y encuentra cualquier título fácilmente. Su vista de galería, inspirada en las bibliotecas modernas de videojuegos, permite explorar tu colección de una manera más visual.

---

## ✨ Características principales

### 📚 Gestión de biblioteca
- Registrar nuevos juegos, editar su información y eliminar entradas.
- Asignar un código identificador único o personalizado a cada juego.
- Guardar descripciones y rutas de las carpetas donde se encuentran los juegos.
- Registrar el estado de progreso:
  - 🟢 Terminado
  - 🟠 Empezado
  - ⚪ Sin empezar
- Clasificar los juegos por idioma:
  - Español
  - Inglés
  - Japonés

### 🖼️ Galería de portadas
- Vista de galería con tarjetas visuales inspiradas en Steam.
- Miniaturas de portada almacenadas directamente en SQLite.
- Compresión de imágenes a WebP para reducir el espacio ocupado.
- Vista de lista para consultar rápidamente los datos de cada juego.

### 🔎 Búsqueda, filtros y ordenamiento
- Búsqueda por título, código identificador y descripción.
- Normalización de códigos para reconocer variantes como `RY-4762560`, `ry 4762560` y `RY4762560`.
- Filtros combinables por estado, idioma y etiquetas.
- Ordenamiento por:
  - Nombre (A-Z)
  - Nombre (Z-A)
  - Código
  - Estado
  - Idioma
  - Más recientes

### 🏷️ Clasificación personalizada
- Asignación de una página o fuente de procedencia:
  - Elonichan
  - Estahi0
  - lewdzone
  - M0RETranslations
  - Otome
  - ryuugames
  - Shorts
  - Youtube
- Clasificación por tipo:
  - Visual Novel
  - RPG
  - Renpy
  - 3d
  - Otro
- Etiquetas personalizadas para organizar la colección según tus propios criterios.
- Puntuación de 0 a 5 estrellas.

### 📊 Estadísticas
- Total de juegos registrados.
- Cantidad de juegos terminados, empezados y pendientes.
- Porcentaje de progreso de la biblioteca.
- Desglose de juegos por idioma.
- Barra visual de progreso.

### 🎨 Personalización
- Tres temas visuales:
  - Claro
  - Oscuro
  - Noche
- Tema Noche con tonos cálidos y menor presencia de luz azul.
- Compatibilidad con Nerd Fonts para mostrar iconos en la interfaz.
- Selección de fuente y tamaño de texto.
- Preferencias guardadas automáticamente en `config.json`.

---

## 🖥️ Capturas de pantalla

> Puedes colocar tus capturas en una carpeta `screenshots/` dentro del repositorio y actualizar las rutas siguientes.

| Vista principal | Galería |
|:---:|:---:|
| ![Vista principal](screenshots/main.png) | ![Galería](screenshots/gallery.png) |

| Tema oscuro | Tema noche |
|:---:|:---:|
| ![Tema oscuro](screenshots/dark.png) | ![Tema noche](screenshots/night.png) |

---

## 🛠️ Tecnologías utilizadas

| Tecnología | Uso |
|---|---|
| Python | Lenguaje principal |
| Tkinter / ttk | Interfaz gráfica |
| SQLite | Base de datos local |
| Pillow | Procesamiento y compresión de imágenes |
| WebP | Almacenamiento optimizado de portadas |
| Nerd Fonts | Iconografía opcional |

La aplicación no necesita un servidor ni una conexión a Internet para administrar la biblioteca.

---

## 📦 Instalación

### Requisitos

- Python instalado.
- Pillow.
- Tkinter disponible en la instalación de Python.

### 1. Clonar el repositorio

```bash
git clone https://github.com/TU-USUARIO/TU-REPOSITORIO.git
```

### 2. Entrar en la carpeta

```bash
cd TU-REPOSITORIO
```

### 3. Instalar dependencias

```bash
pip install pillow
```

### 4. Ejecutar la aplicación

**Windows:**

```bash
python catalogo_rpgmaker-v04.py
```

**Linux:**

```bash
python3 catalogo_rpgmaker-v04.py
```

---

## 🗂️ Estructura de archivos

Al ejecutar la aplicación, se generan los archivos necesarios para conservar la biblioteca y las preferencias:

```text
RPG-Maker-Library/
│
├── catalogo_rpgmaker-v04.py
│
├── juegos.db
│
├── config.json
│
├── fuentes/
│   └── *.ttf / *.otf
│
└── screenshots/
    ├── main.png
    ├── gallery.png
    ├── dark.png
    └── night.png
```

| Archivo o carpeta | Descripción |
|---|---|
| `catalogo_rpgmaker-v04.py` | Aplicación principal |
| `juegos.db` | Base de datos de la biblioteca |
| `config.json` | Preferencias de apariencia |
| `fuentes/` | Fuentes Nerd Font opcionales |
| `screenshots/` | Capturas para documentar el proyecto |

La base de datos se crea automáticamente junto al script si todavía no existe.

---

## 🗄️ Base de datos

El proyecto utiliza SQLite para almacenar la información localmente.

La tabla principal `juegos` almacena:

| Campo | Descripción |
|---|---|
| `id` | Identificador interno |
| `titulo` | Nombre del juego |
| `descripcion` | Descripción |
| `estado` | Progreso del juego |
| `idioma` | Idioma disponible |
| `imagen` | Portada almacenada como BLOB |
| `ruta` | Ubicación de la carpeta del juego |
| `codigo` | Código identificador |
| `pagina` | Fuente o página de procedencia |
| `tipo` | Categoría del juego |
| `puntuacion` | Valoración de 0 a 5 |

También utiliza las tablas `tags` y `juego_tags` para gestionar etiquetas personalizadas y sus relaciones con los juegos.

**Importante:** realiza copias de seguridad periódicas de `juegos.db`. Esta base contiene los registros y las imágenes de tu biblioteca.

---

## 🔤 Nerd Fonts (opcional)

La aplicación puede utilizar Nerd Fonts para mostrar iconos en los controles de la interfaz.

Puedes descargar fuentes compatibles desde:

[https://www.nerdfonts.com](https://www.nerdfonts.com)

También puedes colocar archivos `.ttf` o `.otf` en la carpeta `fuentes/`, situada junto al script. La aplicación intenta cargarlos automáticamente.

Si no tienes ninguna Nerd Font instalada, se utilizará la fuente predeterminada del sistema.

---

## 💾 Copias de seguridad y datos

La biblioteca se almacena localmente en `juegos.db`, por lo que puedes respaldarla copiando ese archivo.

Para migrar tu colección a otro equipo:

1. Cierra la aplicación.
2. Copia `juegos.db` a una ubicación segura.
3. Copia también `config.json` si deseas conservar tus preferencias.
4. Copia la carpeta `fuentes/` si utilizas fuentes personalizadas.
5. Coloca los archivos junto al script en el nuevo equipo.

**Recomendación:** no subas tu `juegos.db` personal al repositorio público. Puede contener información privada sobre tu colección y todas las portadas almacenadas.

---

## 🗺️ Plataformas

El proyecto está desarrollado con Python y Tkinter, y contempla la ejecución en:

- Windows
- Linux
- macOS (con las dependencias correspondientes)

La apertura de carpetas utiliza el explorador de archivos del sistema operativo.

---

## 🚧 Desarrollo

RPG Maker Library es un proyecto en evolución. Su objetivo es ofrecer una herramienta cómoda para mantener organizada una colección personal de juegos, con especial atención a la búsqueda, clasificación y presentación visual.

Las futuras mejoras pueden ampliar las opciones de importación, organización y consulta de la biblioteca.

---

## 📄 Licencia

Este proyecto se distribuye bajo la licencia que se indique en el archivo `LICENSE` del repositorio.

Si deseas reutilizar, modificar o redistribuir el código, consulta los términos de dicha licencia.

---

**Desarrollado con Python 🐍 · SQLite · Tkinter**

*Una biblioteca, todos tus juegos, organizados a tu manera.*