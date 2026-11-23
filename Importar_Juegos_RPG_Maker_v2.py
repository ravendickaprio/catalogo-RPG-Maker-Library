#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Importador masivo para Catálogo RPG Maker (catalogo_rpgmaker-v03.py)

Busca recursivamente archivos .zip/.rar y carpetas de juegos, interpreta
código, estado e idioma desde sus nombres y los agrega a juegos.db.

No extrae archivos ni modifica/mueve los juegos. Por defecto evita códigos
duplicados y muestra una vista previa antes de importar.
"""

import argparse
import os
import re
import sqlite3
import sys
import tkinter as tk
from pathlib import Path
from tkinter import filedialog, messagebox, ttk

ESTADOS = ("Sin empezar", "Empezado", "Terminado")
IDIOMAS = ("Español", "Inglés", "Japonés")
EXTENSIONES = {".zip", ".rar", ".7z"}

# Ejemplos reconocidos: RY-4762560, RY-RJ01361917
CODE_RE = re.compile(r"(?<![A-Z0-9])RY[- _]+([A-Z0-9][A-Z0-9-]*)(?![A-Z0-9])", re.IGNORECASE)
VERSION_RE = re.compile(r"(?i)(?:[_ .-]*V(?:ERSION)?\s*\d+(?:[._-]\d+)*)")
PAREN_COUNTER_RE = re.compile(r"\s*\(\d+\)\s*$")


def interpretar_nombre(nombre: str):
    """Devuelve (titulo, codigo, estado, idioma) a partir del nombre."""
    original = nombre.strip()
    estado = "Sin empezar"

    # Las marcas [F] y [P] pueden aparecer al inicio o en cualquier parte.
    if re.search(r"(?i)(?:^|\s|\[)\[?F\]?(?=\s|$)", original):
        estado = "Terminado"
    elif re.search(r"(?i)(?:^|\s|\[)\[?P\]?(?=\s|$)", original):
        estado = "Empezado"

    codigo_match = CODE_RE.search(original)
    codigo = ("RY-" + codigo_match.group(1)).upper() if codigo_match else ""

    # El idioma explícito tiene prioridad; sin marca EN/ES se asume japonés.
    if re.search(r"(?i)(?:^|[^A-Z])EN(?:$|[^A-Z])", original):
        idioma = "Inglés"
    elif re.search(r"(?i)(?:^|[^A-Z])(?:ES|ESP)(?:$|[^A-Z])", original):
        idioma = "Español"
    else:
        idioma = "Japonés"

    titulo = original

    # Quitar etiquetas entre corchetes que suelen ser fuente o estado.
    titulo = re.sub(r"\[[^\]]*\]", " ", titulo)
    # Quitar el identificador, si existe.
    if codigo_match:
        titulo = titulo[:codigo_match.start()] + " " + titulo[codigo_match.end():]
    # Quitar sufijos comunes de versión, idioma y contador de descarga.
    titulo = VERSION_RE.sub(" ", titulo)
    titulo = re.sub(r"(?i)(?:[_ .-]+)(?:EN|ES|ESP|JP|JPN)(?=$|[_ .( -])", " ", titulo)
    titulo = PAREN_COUNTER_RE.sub("", titulo)
    titulo = re.sub(r"(?i)\b(?:FINAL|FINISHED|PROGRESS)\b", " ", titulo)
    titulo = re.sub(r"[_\-.]+", " ", titulo)
    titulo = re.sub(r"\s+", " ", titulo).strip(" -_.()[]")

    # Cuando el nombre solo contiene el código (ej. "[F] RY-723090"),
    # el código también sirve como título para que el registro no quede vacío.
    if not titulo:
        titulo = codigo or original

    return titulo, codigo, estado, idioma


def descubrir_candidatos(raiz: Path):
    """Archivos ZIP/RAR/7Z y carpetas cuyo nombre contiene un código RY."""
    candidatos = []
    for actual, dirs, files in os.walk(raiz):
        dirs[:] = sorted(d for d in dirs if d not in {".git", "__pycache__", ".venv"})
        actual_path = Path(actual)

        # Cada carpeta con identificador se trata como una entrada de juego.
        if actual_path != raiz and CODE_RE.search(actual_path.name):
            candidatos.append((actual_path.name, actual_path, "carpeta"))

        for filename in sorted(files):
            path = actual_path / filename
            if path.suffix.lower() in EXTENSIONES:
                candidatos.append((path.stem, path, "archivo"))

    # Evita entradas repetidas por la misma ruta.
    unicos = {}
    for item in candidatos:
        unicos[str(item[1].resolve()).casefold()] = item
    return sorted(unicos.values(), key=lambda x: str(x[1]).casefold())


def preparar_base(db_path: Path):
    if not db_path.exists():
        raise FileNotFoundError(
            f"No existe la base de datos:\n{db_path}\n\n"
            "Selecciona el archivo juegos.db de tu catálogo."
        )
    con = sqlite3.connect(str(db_path))
    columnas = {row[1] for row in con.execute("PRAGMA table_info(juegos)")}
    requeridas = {"titulo", "descripcion", "estado", "idioma", "ruta", "codigo", "imagen"}
    if not requeridas.issubset(columnas):
        con.close()
        faltan = ", ".join(sorted(requeridas - columnas))
        raise RuntimeError(
            "La base de datos no tiene el esquema esperado del catálogo "
            f"(faltan columnas: {faltan}).\nNo se modificó nada."
        )
    return con


def importar(raiz: Path, db_path: Path, confirmar_callback=None):
    candidatos = descubrir_candidatos(raiz)
    if not candidatos:
        return {"candidatos": 0, "insertados": 0, "duplicados": 0, "sin_codigo": 0}

    con = preparar_base(db_path)
    insertados = duplicados = sin_codigo = 0
    detalles = []
    try:
        codigos_existentes = {
            str(row[0]).strip().casefold()
            for row in con.execute("SELECT codigo FROM juegos WHERE codigo IS NOT NULL AND codigo != ''")
        }
        rutas_existentes = {
            str(row[0]).strip().casefold()
            for row in con.execute("SELECT ruta FROM juegos WHERE ruta IS NOT NULL AND ruta != ''")
        }

        for nombre, path, tipo in candidatos:
            titulo, codigo, estado, idioma = interpretar_nombre(nombre)
            ruta_juego = str(path)

            motivo = ""
            if not codigo:
                sin_codigo += 1
                motivo = "omitido: no se encontró código RY-"
            elif codigo.casefold() in codigos_existentes:
                duplicados += 1
                motivo = "omitido: código ya registrado"
            elif ruta_juego.casefold() in rutas_existentes:
                duplicados += 1
                motivo = "omitido: ruta ya registrada"
            else:
                con.execute(
                    """INSERT INTO juegos
                       (titulo, descripcion, estado, idioma, imagen, ruta, codigo)
                       VALUES (?, '', ?, ?, NULL, ?, ?)""",
                    (titulo, estado, idioma, ruta_juego, codigo),
                )
                codigos_existentes.add(codigo.casefold())
                rutas_existentes.add(ruta_juego.casefold())
                insertados += 1
                motivo = "agregado"
            detalles.append((tipo, titulo, codigo or "—", estado, idioma, motivo, str(path)))

        con.commit()
    except Exception:
        con.rollback()
        raise
    finally:
        con.close()

    return {
        "candidatos": len(candidatos),
        "insertados": insertados,
        "duplicados": duplicados,
        "sin_codigo": sin_codigo,
        "detalles": detalles,
    }


class ImportadorApp:
    def __init__(self, root, carpeta_inicial=None, db_inicial=None):
        self.root = root
        root.title("Importador de juegos RPG Maker")
        root.geometry("1050x650")
        root.minsize(800, 480)

        self.var_carpeta = tk.StringVar(value=str(carpeta_inicial or ""))
        self.var_db = tk.StringVar(value=str(db_inicial or ""))
        self.var_estado = tk.StringVar(value="Selecciona la carpeta de juegos y la base de datos juegos.db.")

        frame = ttk.Frame(root, padding=12)
        frame.pack(fill="both", expand=True)
        frame.columnconfigure(1, weight=1)
        frame.rowconfigure(3, weight=1)

        ttk.Label(frame, text="Carpeta que contiene los juegos:").grid(row=0, column=0, sticky="w", pady=5)
        ttk.Entry(frame, textvariable=self.var_carpeta).grid(row=0, column=1, sticky="ew", padx=8)
        ttk.Button(frame, text="Examinar…", command=self.elegir_carpeta).grid(row=0, column=2)

        ttk.Label(frame, text="Base de datos del catálogo (juegos.db):").grid(row=1, column=0, sticky="w", pady=5)
        ttk.Entry(frame, textvariable=self.var_db).grid(row=1, column=1, sticky="ew", padx=8)
        ttk.Button(frame, text="Examinar…", command=self.elegir_db).grid(row=1, column=2)

        ttk.Label(
            frame,
            text="Detecta ZIP/RAR/7Z y carpetas con código RY. No extrae ni mueve archivos.",
        ).grid(row=2, column=0, columnspan=3, sticky="w", pady=(8, 5))

        cols = ("tipo", "titulo", "codigo", "estado", "idioma", "resultado", "ruta")
        self.tabla = ttk.Treeview(frame, columns=cols, show="headings")
        for col, label, width in [
            ("tipo", "Tipo", 75), ("titulo", "Título detectado", 190),
            ("codigo", "Código", 135), ("estado", "Estado", 100),
            ("idioma", "Idioma", 90), ("resultado", "Resultado", 175),
            ("ruta", "Archivo / carpeta", 300),
        ]:
            self.tabla.heading(col, text=label)
            self.tabla.column(col, width=width, minwidth=60, anchor="w")
        self.tabla.grid(row=3, column=0, columnspan=3, sticky="nsew")
        scroll = ttk.Scrollbar(frame, orient="vertical", command=self.tabla.yview)
        self.tabla.configure(yscrollcommand=scroll.set)
        scroll.grid(row=3, column=3, sticky="ns")

        bottom = ttk.Frame(frame)
        bottom.grid(row=4, column=0, columnspan=3, sticky="ew", pady=(10, 0))
        bottom.columnconfigure(0, weight=1)
        ttk.Label(bottom, textvariable=self.var_estado).grid(row=0, column=0, sticky="w")
        ttk.Button(bottom, text="Importar a la biblioteca", command=self.ejecutar).grid(row=0, column=1, padx=5)
        ttk.Button(bottom, text="Cerrar", command=root.destroy).grid(row=0, column=2)

    def elegir_carpeta(self):
        p = filedialog.askdirectory(title="Selecciona la carpeta que contiene los juegos")
        if p:
            self.var_carpeta.set(p)

    def elegir_db(self):
        p = filedialog.askopenfilename(
            title="Selecciona juegos.db",
            filetypes=[("Base de datos SQLite", "*.db"), ("Todos los archivos", "*.*")]
        )
        if p:
            self.var_db.set(p)

    def ejecutar(self):
        carpeta = Path(self.var_carpeta.get()).expanduser()
        db = Path(self.var_db.get()).expanduser()
        if not carpeta.is_dir():
            messagebox.showerror("Carpeta inválida", "Selecciona una carpeta existente.")
            return
        if not db.is_file():
            messagebox.showerror("Base de datos inválida", "Selecciona el archivo juegos.db de tu catálogo.")
            return
        try:
            # Primera pasada: generar vista previa, sin escribir en la base.
            candidatos = descubrir_candidatos(carpeta)
            if not candidatos:
                messagebox.showinfo("Sin resultados", "No se encontraron archivos ZIP/RAR/7Z ni carpetas con código RY.")
                return
            previa = []
            for nombre, path, tipo in candidatos:
                titulo, codigo, estado, idioma = interpretar_nombre(nombre)
                previa.append((tipo, titulo, codigo or "—", estado, idioma, "pendiente", str(path)))
            self.mostrar_filas(previa)
            if not messagebox.askyesno(
                "Confirmar importación",
                f"Se encontraron {len(candidatos)} candidatos.\n\n"
                "Se agregarán solo los que tengan un código RY- y no estén duplicados.\n"
                "Los nuevos registros tendrán descripción e imagen vacías.\n\n¿Continuar?"
            ):
                self.var_estado.set("Importación cancelada; no se modificó la base de datos.")
                return

            resultado = importar(carpeta, db)
            self.mostrar_filas(resultado.get("detalles", []))
            self.var_estado.set(
                f"Importados: {resultado['insertados']} | Duplicados: {resultado['duplicados']} "
                f"| Sin código: {resultado['sin_codigo']}"
            )
            messagebox.showinfo(
                "Importación terminada",
                f"Registros agregados: {resultado['insertados']}\n"
                f"Duplicados omitidos: {resultado['duplicados']}\n"
                f"Sin código RY-: {resultado['sin_codigo']}\n\n"
                "Abre o actualiza tu catálogo para ver los juegos."
            )
        except Exception as exc:
            messagebox.showerror("Error durante la importación", str(exc))

    def mostrar_filas(self, filas):
        for item in self.tabla.get_children():
            self.tabla.delete(item)
        for fila in filas:
            self.tabla.insert("", "end", values=fila)


def main():
    parser = argparse.ArgumentParser(description="Importa juegos ZIP/RAR/7Z y carpetas a juegos.db.")
    parser.add_argument("--carpeta", help="Carpeta raíz que contiene los juegos.")
    parser.add_argument("--db", help="Ruta completa al archivo juegos.db.")
    args = parser.parse_args()

    root = tk.Tk()
    ImportadorApp(root, args.carpeta, args.db)
    root.mainloop()


if __name__ == "__main__":
    main()
