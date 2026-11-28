# Proyecto: Herramienta de recuperación automática de metadatos de RPG Maker Library

## Rol y coordinación

Claude (Opus) será el arquitecto principal, responsable del análisis, diseño, planificación, coordinación de agentes, validación de arquitectura y supervisión de la implementación.

Utiliza los siguientes agentes mediante las capacidades de delegación disponibles en Claude Code:

- **Agente Haiku: lector, analista y auditor independiente.** Su función será inspeccionar la base de datos, analizar el sitio web, revisar las estrategias de extracción, auditar el código, detectar errores, revisar seguridad y validar los resultados. No debe realizar modificaciones principales de implementación salvo que se le solicite expresamente.
- **Agente Sonnet: implementador.** Su función será implementar la herramienta, desarrollar los componentes, solucionar errores, ejecutar pruebas y aplicar las modificaciones técnicas que Claude (Opus) haya planificado y autorizado.

Claude (Opus) será el responsable de integrar los resultados, tomar decisiones arquitectónicas, revisar las modificaciones y asegurar que el trabajo de los agentes sea coherente.

No afirmes que has utilizado agentes si no están realmente disponibles o si no se han ejecutado. En ese caso, informa de las limitaciones y realiza las tareas que puedas directamente.

## Ponytail

Utiliza Ponytail Full durante el desarrollo para optimizar la implementación, la revisión y el mantenimiento del código.

Prioriza:

- Reutilizar código y componentes existentes.
- Mantener la implementación sencilla.
- Evitar dependencias innecesarias.
- Evitar abstracciones excesivas.
- Evitar wrappers y capas innecesarias.
- Evitar duplicación.
- Minimizar el consumo de tokens.
- Minimizar salida de terminal innecesaria.
- Evitar consultas o procesamiento redundantes.
- Utilizar los agentes de manera eficiente.

No sacrifiques seguridad, integridad de datos, calidad, pruebas ni funcionalidad por reducir código o tokens.

---

## 1. Objetivo del proyecto

Desarrollar una herramienta independiente para recuperar automáticamente títulos, descripciones e imágenes de juegos de RPG Maker almacenados en una base de datos SQLite que actualmente carecen de descripción.

La fuente de datos será:

https://www.ryuugames.com/

Actualmente existen aproximadamente 800 registros que podrían beneficiarse de este proceso.

La herramienta deberá seleccionar una base de datos SQLite indicada por el usuario, analizar los registros elegibles, buscar sus códigos en Ryuugames, recuperar los datos correspondientes y permitir revisar y aplicar los resultados de manera segura.

La herramienta debe ser independiente de la aplicación Rust/Tauri principal.

Preferiblemente, desarrollarla en Python, con compatibilidad con Windows y Nobara/Linux.

---

## 2. Inspección inicial de la base de datos

Antes de implementar:

- Inspecciona el esquema real de SQLite.
- Identifica la tabla principal de juegos.
- Identifica los campos `codigo`, `titulo`, `descripcion` e `imagen`.
- Comprueba cómo se almacenan actualmente las imágenes.
- Comprueba cómo se representan las descripciones vacías.
- Identifica índices y restricciones relevantes.
- Comprueba posibles códigos duplicados.
- Comprueba cómo se gestionan las rutas de imágenes.
- Identifica cualquier detalle que pueda afectar a la actualización de registros.

No asumas que el esquema coincide exactamente con los ejemplos de este prompt.

Adapta la implementación a la estructura real.

El agente Haiku deberá realizar una inspección independiente y reportar cualquier riesgo relacionado con la estructura de datos o la actualización de la base de datos.

---

## 3. Selección de la base de datos

El programa deberá permitir seleccionar una base de datos SQLite desde el explorador de archivos.

Preferiblemente, utiliza una interfaz gráfica sencilla con Tkinter.

La interfaz deberá permitir:

- Seleccionar el archivo DB.
- Mostrar la ruta seleccionada.
- Mostrar el total de registros.
- Mostrar el número de registros sin descripción.
- Mostrar el número de registros elegibles para recuperación.
- Mostrar el número de registros excluidos por código.
- Iniciar el proceso.
- Cancelar el proceso de manera segura.
- Mostrar el progreso.

Evita construir una interfaz innecesariamente compleja.

---

## 4. Filtro obligatorio de códigos

Este requisito es obligatorio y debe implementarse antes de realizar cualquier solicitud HTTP.

El programa deberá analizar el campo `codigo` de cada registro cuya descripción esté vacía.

### Paso 1: retirar el prefijo

Si el código comienza con `RY-`, retirar ese prefijo exclusivamente para realizar la búsqueda.

Ejemplos:

- `RY-RJ01234567` → `RJ01234567`
- `RY-RJ1704529` → `RJ1704529`
- `RY-12345678` → `12345678`

El código original almacenado en la base de datos no debe modificarse.

### Paso 2: comprobar la letra R

Una vez retirado el prefijo `RY-`, el código debe contener la letra `R` para ser elegible.

Si el código resultante no contiene la letra `R`, el registro deberá excluirse del procesamiento.

Ejemplos:

| Código original | Código para búsqueda | Resultado |
|---|---|---|
| `RY-RJ01234567` | `RJ01234567` | <ELEGIBLE> |
| `RY-R12345678` | `R12345678` | <ELEGIBLE> |
| `RY-12345678` | `12345678` | <EXCLUIDO> |
| `RY-ABC123456` | `ABC123456` | <EXCLUIDO> |
| `RJ01234567` | `RJ01234567` | <ELEGIBLE> |

La comprobación debe ser insensible a mayúsculas y minúsculas.

Si el código no tiene `RY-`, utilizar el código original como base de búsqueda y aplicar la misma regla de la letra `R`.

Los códigos vacíos, NULL o inválidos deben excluirse y registrarse como tales.

Los registros excluidos no deben provocar solicitudes HTTP.

El programa deberá mostrar el total de registros excluidos por esta regla.

No se deben alterar los códigos almacenados en SQLite.

---

## 5. Identificación de registros a recuperar

Los registros candidatos serán aquellos cuyo campo `descripcion`:

- Sea NULL.
- Sea una cadena vacía.
- Contenga únicamente espacios en blanco.

Los registros que tengan una descripción existente no deben procesarse ni modificarse.

Después de identificar los registros sin descripción, aplicar el filtro de códigos del apartado anterior.

El resultado será la lista definitiva de registros elegibles para búsqueda.

---

## 6. Investigación del mecanismo de búsqueda en Ryuugames

Investiga cómo funciona realmente el sistema de búsqueda de:

https://www.ryuugames.com/

No inventes un endpoint ni asumas que una URL construida manualmente funciona.

Determina:

- Cómo se realizan las búsquedas.
- Si existe un buscador interno.
- Qué parámetros utiliza.
- Cómo devuelve los resultados.
- Si existen redirecciones.
- Si las búsquedas por código RJ funcionan.
- Cómo identificar una publicación concreta.
- Si el sitio depende de JavaScript.
- Qué método de búsqueda es más estable y sencillo.

Utiliza preferentemente solicitudes HTTP y parsing de HTML.

No introduzcas Selenium ni automatización de navegador salvo que sea realmente necesario.

No intentes eludir CAPTCHAs, mecanismos anti-bot, restricciones de acceso ni autenticación.

Respeta las políticas del sitio y utiliza un ritmo de solicitudes moderado.

El agente Haiku deberá investigar o auditar independientemente el mecanismo de búsqueda y contrastar la estrategia propuesta por Opus.

---

## 7. Comportamiento cuando no se encuentra un resultado

Este requisito es fundamental.

Si el programa realiza una búsqueda y no encuentra una publicación válida correspondiente al código del juego:

- No debe modificar el registro de SQLite.
- No debe modificar el título.
- No debe modificar la descripción.
- No debe modificar la imagen.
- No debe modificar ningún otro campo.
- No debe inventar datos.
- No debe utilizar resultados de otros juegos como sustituto.

El registro debe permanecer exactamente igual que antes del procesamiento.

El resultado de la búsqueda debe registrarse en el informe como `No encontrado`, indicando el código buscado y, si corresponde, el motivo.

Si la búsqueda devuelve resultados pero no se puede verificar razonablemente que correspondan al código, el registro también debe permanecer sin modificaciones y clasificarse como `Coincidencia dudosa`.

Solamente los registros con una coincidencia razonablemente verificada podrán ser candidatos a actualización.

---

## 8. Extracción del título

Cuando se encuentre y verifique una publicación válida, extraer el título del elemento:

`<h1 class="entry-title">`

El parsing debe tolerar variaciones razonables en HTML y atributos.

El título recuperado será una propuesta de actualización del campo `titulo`.

No extraigas títulos de páginas de resultados de búsqueda si no se ha verificado que corresponden a la publicación del juego.

---

## 9. Extracción de la imagen

Identifica la imagen principal de la publicación.

En la referencia proporcionada, la imagen aparece en un elemento similar a:

`<img fetchpriority="high" decoding="async" class="alignnone size-full wp-image-172562 td-animation-stack-type0-2" src="...">`

Investiga la estructura real del HTML.

No dependas exclusivamente de las clases exactas de ese ejemplo.

Considera:

- `src`.
- `srcset`.
- Imágenes principales del artículo.
- Imágenes de portada.
- Elementos relacionados con la publicación.
- Posibles URLs de imágenes originales.

Evita capturar logos, iconos, publicidad, avatares, imágenes relacionadas o imágenes de la barra lateral.

La imagen debe corresponder al juego identificado.

### Descarga de imágenes

Descarga la imagen localmente y utiliza el formato de almacenamiento que sea compatible con la DB y la aplicación principal.

Antes de implementar, inspecciona el sistema de almacenamiento de imágenes existente.

Requisitos:

- No introducir incompatibilidades.
- No descargar imágenes duplicadas innecesariamente.
- Utilizar nombres deterministas basados en el código o ID.
- Validar que la imagen descargada sea válida.
- Evitar rutas inválidas.
- No sobrescribir imágenes válidas existentes.
- No modificar el campo `imagen` si la descarga falla.
- Mantener rutas relativas cuando sea apropiado.
- Organizar las imágenes recuperadas en una carpeta coherente.

Si la imagen existente está vacía o su archivo ya no existe, la imagen recuperada podrá proponerse para actualizar el registro.

Si existe una imagen válida, conservarla salvo que el usuario autorice explícitamente sustituirla.

---

## 10. Extracción de la descripción

Localiza el apartado `DESCRIPTION` de la publicación.

Investiga cómo está estructurado realmente el contenido HTML.

Extrae el texto correspondiente a la descripción del juego, evitando:

- Menús.
- Navegación.
- Anuncios.
- Barras laterales.
- Publicaciones relacionadas.
- Comentarios.
- Contenido ajeno a la descripción.

Normaliza el texto.

El resultado deberá:

- Eliminar HTML.
- Eliminar espacios redundantes.
- Mantener el idioma original.
- No traducir.
- Evitar concatenaciones extrañas.
- Limitarse a un máximo de 400 caracteres.
- Intentar no cortar palabras de forma innecesaria sin exceder el límite.

La descripción recuperada será una propuesta de actualización.

---

## 11. Validación de coincidencias

Antes de aceptar un resultado, verifica que la publicación corresponde al código buscado.

Considera:

- Código RJ en la URL.
- Código RJ en el título.
- Código RJ en el contenido.
- Código RJ en otros elementos de la publicación.

La estrategia debe ser conservadora.

Si no se puede verificar razonablemente:

- No actualices la base de datos.
- Clasifica el resultado como dudoso.
- Registra el motivo.
- Permite su revisión posterior.

No utilices una coincidencia dudosa como resultado definitivo.

---

## 12. Protección de la base de datos

### Respaldo

Antes de aplicar cambios:

- Crear un respaldo de la DB.
- Utilizar un método seguro de backup SQLite, teniendo en cuenta WAL si está activo.
- Comprobar que el respaldo se haya generado.
- No sobrescribir respaldos anteriores.
- Mostrar la ubicación del respaldo.
- No utilizar un método de copia que pueda producir una copia inconsistente.

### Actualización de registros

Actualizar exclusivamente registros elegibles y aprobados.

La descripción debe continuar vacía al momento de aplicar los cambios.

No sobrescribir descripciones existentes.

### Título

No sobrescribir títulos significativos sin necesidad.

Si el título existente es vacío o claramente provisional, se podrá actualizar.

Si existe un título significativo, conservarlo por defecto y permitir que el usuario apruebe su sustitución en la previsualización.

### Imagen

No sobrescribir una imagen válida existente.

Solo proponer actualizar el campo si está vacío o la ruta existente apunta a un archivo inexistente.

### Otros campos

No modificar:

- `codigo`.
- `estado`.
- `idioma`.
- `pagina`.
- `tipo`.
- `tags`.
- `puntuacion`.
- Otros campos no incluidos explícitamente en esta recuperación.

No eliminar registros.

No modificar otras tablas innecesariamente.

Utiliza transacciones y operaciones seguras.

---

## 13. Modo Dry Run y revisión

Quiero que el programa permita realizar el scraping sin modificar la base de datos.

La previsualización debe mostrar:

- Código original.
- Código utilizado para búsqueda.
- Título actual.
- Título encontrado.
- URL de la publicación.
- Descripción recuperada.
- Imagen encontrada.
- Resultado de validación.
- Estado de procesamiento.
- Motivo de omisión cuando corresponda.

El usuario debe poder revisar los resultados.

Permite seleccionar qué registros desea aplicar.

No debe modificarse la DB durante el modo de previsualización.

Los registros no encontrados, excluidos o dudosos no deben poder seleccionarse como actualizaciones automáticas.

---

## 14. Control de solicitudes

Implementa:

- Solicitudes moderadas.
- Intervalo configurable.
- Intervalo predeterminado conservador de aproximadamente 2 a 4 segundos.
- Timeout.
- Reintentos limitados.
- Manejo de errores HTTP.
- Manejo de errores de parsing.
- Manejo de conexiones interrumpidas.
- Cancelación.
- Registro de errores.

No envíes cientos de solicitudes simultáneamente.

No utilices reintentos ilimitados.

Si el sitio responde con mecanismos de limitación, respétalos y reduce o detén el procesamiento según corresponda.

No intentes eludir protecciones.

---

## 15. Reanudación

El proceso debe poder reanudarse.

Al ejecutarlo nuevamente:

- Volver a consultar la DB.
- Identificar las descripciones que continúan vacías.
- Aplicar el filtro de códigos.
- Evitar procesar innecesariamente registros ya completados.

Los registros no encontrados deben permanecer sin modificar y podrán ser revisados en futuras ejecuciones, si el usuario así lo decide.

Puedes utilizar un archivo auxiliar de progreso o reportes de sesión.

No introduzcas un sistema de seguimiento innecesariamente complejo.

---

## 16. Informes

Genera un informe final con:

- Total de registros.
- Registros sin descripción.
- Registros elegibles.
- Registros excluidos por no contener `R`.
- Registros con código inválido.
- Registros procesados.
- Coincidencias verificadas.
- Coincidencias dudosas.
- No encontrados.
- Errores HTTP.
- Errores de parsing.
- Imágenes descargadas.
- Imágenes fallidas.
- Registros actualizados.
- Registros omitidos.

Genera un CSV con:

- Código original.
- Código utilizado.
- Título original.
- Título recuperado.
- URL.
- Estado de búsqueda.
- Estado de validación.
- Resultado de la extracción.
- Motivo de omisión o error.

No incluyas contraseñas ni datos ajenos a la recuperación.

---

## 17. Arquitectura

Preferiblemente utiliza:

- Python.
- SQLite.
- Requests o alternativa apropiada.
- BeautifulSoup4.
- Tkinter.
- Pillow cuando sea necesario.

Mantén el código sencillo, modular y fácil de mantener.

Evita:

- Dependencias innecesarias.
- Selenium sin justificación.
- Docker.
- Servidores web.
- Servicios externos.
- Arquitecturas excesivas.
- Sistemas de plugins innecesarios.

La herramienta debe ser local y funcionar preferiblemente en Windows y Nobara/Linux.

---

## 18. Pruebas

Implementa pruebas con una DB de prueba y HTML de ejemplo.

Verifica:

- Selección de registros sin descripción.
- Filtro del prefijo `RY-`.
- Filtro obligatorio de la letra `R`.
- Códigos inválidos.
- Exclusión sin solicitudes HTTP.
- Búsqueda.
- Parsing del título.
- Parsing de imagen.
- Parsing de descripción.
- Límite de 400 caracteres.
- Validación de coincidencias.
- No encontrado sin modificación.
- Coincidencia dudosa sin modificación.
- Descarga de imagen.
- Validación de imágenes.
- Backup.
- Actualización SQLite.
- Protección de otros campos.
- Dry Run.
- Reanudación.
- Cancelación.
- Generación de CSV.
- Manejo de errores.

No utilices la DB original para pruebas.

---

## 19. Plan de ejecución y coordinación

Claude (Opus) será el arquitecto y coordinador.

### Etapa A: análisis

Opus debe:

1. Inspeccionar la estructura real de SQLite.
2. Identificar los registros candidatos.
3. Aplicar conceptualmente el filtro de código.
4. Investigar Ryuugames.
5. Inspeccionar una publicación real.
6. Diseñar la extracción.
7. Definir los mecanismos de respaldo.
8. Presentar un plan de implementación.

Haiku debe realizar una revisión independiente del esquema, la estrategia de scraping y los riesgos.

### Etapa B: implementación

Sonnet implementará la herramienta conforme al plan aprobado por Opus.

Opus supervisará:

- Arquitectura.
- Integración.
- Calidad.
- Seguridad.
- Revisión.
- Coordinación.

### Etapa C: auditoría

Haiku revisará el código implementado, especialmente:

- Filtro de códigos.
- Correspondencia de resultados.
- Seguridad de SQLite.
- No sobrescritura de datos.
- Parsing.
- Errores.
- Riesgos.

Sonnet corregirá los defectos identificados que Opus considere válidos.

Opus ejecutará o supervisará las pruebas finales y verificará los resultados.

No afirmes que los agentes han ejecutado tareas si no han sido realmente delegadas y completadas.

---

## 20. Entregables

Entrega:

- Script Python principal.
- Interfaz de selección de DB.
- Recuperación de metadatos.
- Modo previsualización.
- Selección de resultados.
- Descarga de imágenes.
- Backup seguro.
- Actualización de SQLite.
- CSV.
- Logs.
- README.
- requirements.txt si corresponde.
- Pruebas.

---

## 21. Criterios de aceptación

La herramienta debe:

- Permitir seleccionar la DB.
- Detectar descripciones vacías.
- Excluir códigos que, tras retirar `RY-`, no contengan `R`.
- No enviar solicitudes para códigos excluidos.
- Buscar códigos elegibles en Ryuugames.
- Verificar coincidencias.
- Extraer títulos.
- Extraer imágenes.
- Descargar imágenes localmente.
- Extraer descripciones.
- Limitar descripciones a 400 caracteres.
- No modificar registros no encontrados.
- No modificar registros dudosos.
- No sobrescribir datos existentes innecesariamente.
- Crear backups.
- Permitir revisión.
- Aplicar cambios de manera segura.
- Generar informes.
- Reanudar.
- Manejar errores.
- Funcionar en Windows y Linux en la medida en que sus dependencias lo permitan.

---

## 22. Inicio

Comienza inspeccionando el esquema real de SQLite y el mecanismo de búsqueda de Ryuugames.

Utiliza a Haiku para auditoría e investigación independiente y a Sonnet para implementación, bajo la coordinación de Opus.

Utiliza Ponytail Full.

No modifiques la base de datos original durante la investigación ni las pruebas.

Presenta un plan breve y después procede con la implementación de la herramienta, siempre que el alcance esté autorizado.

No modifiques la aplicación Rust/Tauri principal.

No inventes resultados.

Si Ryuugames no permite una búsqueda fiable o no se puede identificar una publicación correctamente, conserva el registro intacto y documenta el problema.

La integridad de la base de datos tiene prioridad sobre completar la recuperación de todos los registros.