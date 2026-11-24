
import sqlite3
import tkinter as tk
from tkinter import filedialog, messagebox
from pathlib import Path
from datetime import datetime


def seleccionar_db():
    root = tk.Tk()
    root.withdraw()

    archivo = filedialog.askopenfilename(
        title="Selecciona tu base de datos RPG Maker",
        filetypes=[("Bases de datos SQLite", "*.db"), ("Todos los archivos", "*.*")]
    )

    root.destroy()
    return Path(archivo) if archivo else None


def buscar_columna(columnas, alternativas):
    """Busca una columna ignorando mayúsculas y minúsculas."""
    mapa = {col.lower(): col for col in columnas}

    for alternativa in alternativas:
        if alternativa.lower() in mapa:
            return mapa[alternativa.lower()]

    return None


def citar_identificador(nombre):
    """Protege nombres de tablas y columnas en SQL."""
    return '"' + nombre.replace('"', '""') + '"'


def reparar_base_datos(archivo):
    conexion = sqlite3.connect(archivo)

    try:
        # Comprobar que exista la tabla de juegos.
        tablas = conexion.execute("""
            SELECT name
            FROM sqlite_master
            WHERE type = 'table'
        """).fetchall()

        tabla = next(
            (t[0] for t in tablas if t[0].lower() == "juegos"),
            None
        )

        if not tabla:
            raise Exception("No se encontró la tabla 'juegos'.")

        # Obtener las columnas existentes.
        columnas = [
            fila[1]
            for fila in conexion.execute(
                f"PRAGMA table_info({citar_identificador(tabla)})"
            )
        ]

        columna_pagina = buscar_columna(
            columnas, ["pagina", "paginas", "page", "pages"]
        )

        columna_tipo = buscar_columna(
            columnas, ["tipo", "tipos", "type", "types"]
        )

        if not columna_pagina or not columna_tipo:
            raise Exception(
                "No se encontraron las columnas necesarias.\n"
                f"Columnas disponibles: {', '.join(columnas)}"
            )

        # Crear respaldo consistente de SQLite.
        marca_tiempo = datetime.now().strftime("%Y%m%d_%H%M%S")
        respaldo = archivo.with_name(
            f"{archivo.stem}_respaldo_{marca_tiempo}{archivo.suffix}"
        )

        conexion_respaldo = sqlite3.connect(respaldo)
        conexion.backup(conexion_respaldo)
        conexion_respaldo.close()

        # Realizar las modificaciones en una transacción.
        cursor = conexion.cursor()

        cursor.execute(
            f"""
            UPDATE {citar_identificador(tabla)}
            SET {citar_identificador(columna_pagina)} = ?
            """,
            ("ryuugames",)
        )
        paginas_actualizadas = cursor.rowcount

        cursor.execute(
            f"""
            UPDATE {citar_identificador(tabla)}
            SET {citar_identificador(columna_tipo)} = ?
            WHERE LOWER(TRIM({citar_identificador(columna_tipo)})) = ?
            """,
            ("RPG", "otro")
        )
        tipos_actualizados = cursor.rowcount

        conexion.commit()

        return respaldo, paginas_actualizadas, tipos_actualizados

    except Exception:
        conexion.rollback()
        raise

    finally:
        conexion.close()


def main():
    archivo = seleccionar_db()

    if not archivo:
        return

    try:
        respaldo, paginas, tipos = reparar_base_datos(archivo)

        mensaje = (
            "¡Base de datos actualizada correctamente!\n\n"
            f"Juegos con página asignada: {paginas}\n"
            f"Tipos cambiados de Otro a RPG: {tipos}\n\n"
            f"Copia de seguridad creada:\n{respaldo.name}"
        )

        print(mensaje)
        messagebox.showinfo("Proceso completado", mensaje)

    except Exception as error:
        mensaje = f"No se pudo completar la operación:\n\n{error}"
        print(mensaje)
        messagebox.showerror("Error", mensaje)


if __name__ == "__main__":
    main()
