# Recuperar metadatos desde Ryuugames

Herramienta independiente (Python + Tkinter) que rellena **descripción**, **título** y **portada** de los juegos sin descripción consultando la API REST pública de WordPress de ryuugames.com. No depende de la app de escritorio. Las decisiones de diseño están en `docs/DECISIONS.md` (ADR-021).

## Requisitos
- Python 3.10 o superior, con Tkinter (Windows: incluido; Fedora/Nobara: `sudo dnf install python3-tkinter`).
- Pillow: `python -m pip install -r tools/ryuugames/requirements.txt`.

## Uso
```
python tools/ryuugames/recuperar_ryuugames.py
```
1. **Seleccionar DB…** muestra el total de registros, los que no tienen descripción, los elegibles, los excluidos y los pendientes.
2. **Buscar (no modifica la DB)** consulta cada código elegible, con una solicitud cada 3 s por defecto (mínimo 1 s). Se puede cancelar en cualquier momento y el progreso se guarda tras cada juego.
3. Revisa la tabla y el panel de detalle (descripción, URL y portada). Marca con clic o Espacio en la columna **Aplicar**. Solo se pueden marcar las coincidencias **verificadas** que tengan descripción.
4. **Aplicar seleccionados…** crea un backup y aplica los cambios en una transacción.

Las sesiones se pueden reanudar: al volver a abrir la misma DB solo se buscan los registros pendientes. Los errores HTTP se reintentan siempre. Los «no encontrado» y los «dudoso» solo se vuelven a buscar si marcas *Volver a buscar no encontrados y dudosos*.

## Reglas
- **Candidatos:** registros con `descripcion` NULL, vacía o solo con espacios.
- **Código:** se quita `RY-` solo para la búsqueda y el código guardado no cambia. Si el resultado no contiene la letra `R` (sin distinguir mayúsculas), el registro se excluye sin hacer ninguna solicitud. Los códigos vacíos o con caracteres raros son «inválidos».
- **Verificada:** el código aparece como palabra completa en la URL o en el título de exactamente una entrada. Si hay varias y solo una es `[ENG]`, se elige esa y se anota el motivo.
- **Dudosa:** el código solo aparece en el contenido, hay varias entradas sin una `[ENG]` única, o el código tiene menos de 5 dígitos seguidos.
- **No encontrado o dudoso:** el registro no se toca.
- **Título:** se limpia de `[ENG]`, versiones `(V1.2)` y códigos `(RJ…)`. Solo sustituye títulos provisionales (`RY 12345`, vacío o igual al código). Para sustituir un título significativo, haz clic en «Reemplazar título» de esa fila.
- **Descripción:** el texto del bloque `DESCRIPTION`, sin HTML, con el idioma original y un máximo de 400 caracteres, cortando por palabra.
- **Portada:** la imagen destacada de la entrada. Se guarda en `imagen` como WebP de 300×300 con calidad 65, el mismo formato que usa la app Python, y **solo si el registro no tiene imagen**.
- No se modifican `codigo`, `estado`, `idioma`, `ruta`, `pagina`, `tipo`, `puntuacion` ni las etiquetas. Si un registro recibió una descripción después de la búsqueda (desde la app o el editor), se omite.
- Si el sitio responde 403/429/503 o con un desafío anti-bot, el proceso **se detiene**. No se intenta eludir.

## Archivos generados
Junto a la DB:
- `backups/<db>.bak-<fecha>-ryuugames`: copia hecha con la API de backup de SQLite y verificada con `integrity_check`. No sobrescribe backups anteriores.
- `recuperacion_ryuugames/`, con:
  - `cache/<código>.<ext>`: imágenes originales, para no descargarlas dos veces;
  - `<db>_sesion.json`: resultado por registro, que permite reanudar;
  - `informe_<fecha>.txt` y `.csv`: resumen y detalle por registro;
  - `ryuugames_<fecha>.log`.

## Pruebas
```
python -m unittest discover -s tools/ryuugames -v
```
Usan una DB sintética y respuestas de ejemplo, sin red.
