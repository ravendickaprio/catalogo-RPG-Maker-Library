#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Catálogo de juegos RPG Maker  (v4)
----------------------------------
- Base de datos SQLite (juegos.db, se crea junto a este script)
- Código identificador editable (ej. RY-4762560), visible y buscable
- Buscador (título, código, descripción) + filtros combinables por estado e idioma, y orden
- Ruta de la carpeta del juego, con botón para abrirla
- Estadísticas de la biblioteca
- Vista de lista y vista de galería de portadas (estilo Steam)
- Temas: Claro / Oscuro / Noche (tonos cálidos, menos luz azul)
- Soporte de Nerd Fonts (iconos en la interfaz)
- Imagen reducida y comprimida (WebP) para ocupar el mínimo espacio

Requisitos:  pip install pillow
Ejecutar:    python catalogo_rpgmaker.py      (Windows)
             python3 catalogo_rpgmaker.py     (Linux)

Nerd Font: instala una (https://www.nerdfonts.com) o copia su archivo .ttf/.otf
en una carpeta llamada "fuentes" junto a este script; se carga sola.
Las preferencias (tema, fuente, tamaño) se guardan en config.json.
"""

import io
import json
import os
import re
import sqlite3
import subprocess
import sys
import tkinter as tk
import tkinter.font as tkfont
from tkinter import filedialog, messagebox, simpledialog, ttk

try:
    from PIL import Image, ImageDraw, ImageOps, ImageTk
except ImportError:
    _r = tk.Tk()
    _r.iconbitmap("VisorRPG.ico")
    _r.withdraw()
    messagebox.showerror("Falta Pillow", "Necesitas instalar Pillow:\n\n    pip install pillow")
    sys.exit(1)

# ----------------------------------------------------------------------------
# Configuración
# ----------------------------------------------------------------------------
ESTADOS = ["Sin empezar", "Empezado", "Terminado"]
IDIOMAS = ["Español", "Inglés", "Japonés"]
PAGINAS = ["Elonichan", "Estahi0", "lewdzone", "M0RETranslations",
           "Otome", "ryuugames", "Shorts", "Youtube"]
TIPOS = ["Visual Novel", "RPG", "Renpy", "3d", "Otro"]
PUNTUACIONES = [f"{x / 2:.1f}" for x in range(0, 11)]
ORDENES = ["Nombre (A-Z)", "Nombre (Z-A)", "Código", "Estado", "Idioma", "Más recientes"]

IMG_MAX = (300, 300)              # tamaño máximo de la miniatura guardada
IMG_CALIDAD = 65                  # calidad WebP (menor = menos espacio)
PORTADA_W, PORTADA_H = 140, 190   # tamaño de portada en la galería

if getattr(sys, "frozen", False):          # por si lo empaquetas con PyInstaller
    BASE_DIR = os.path.dirname(sys.executable)
else:
    BASE_DIR = os.path.dirname(os.path.abspath(__file__))
DB_PATH = os.path.join(BASE_DIR, "juegos.db")
CONFIG_PATH = os.path.join(BASE_DIR, "config.json")
FUENTES_DIR = os.path.join(BASE_DIR, "fuentes")

# ----------------------------------------------------------------------------
# Temas (colores suaves, sin negro ni blanco puros)
# ----------------------------------------------------------------------------
TEMAS = {
    "Claro": dict(
        fondo="#eceef1", superficie="#e1e4e9", campo="#ffffff", borde="#c3c9d2",
        texto="#2b303a", tenue="#6b7280", acento="#3b6ea5", seleccion="#c9dcf2",
        hover="#d3d8df", ok="#3f8f2a", warn="#b26a00", neutro="#6b7785",
        galeria="#d9dde3", tarjeta="#ffffff", badge_texto="#ffffff",
    ),
    "Oscuro": dict(
        fondo="#1e2127", superficie="#2a2f37", campo="#262a31", borde="#3b414c",
        texto="#c8cfda", tenue="#8791a1", acento="#6f9bc7", seleccion="#34445a",
        hover="#343a44", ok="#7aa86a", warn="#cf9f5a", neutro="#76808f",
        galeria="#181b20", tarjeta="#22262d", badge_texto="#14171c",
    ),
    # Noche: tonos cálidos y contraste moderado (menos luz azul)
    "Noche": dict(
        fondo="#1c1a18", superficie="#2a2623", campo="#242120", borde="#3d3630",
        texto="#d4c8b4", tenue="#8f8372", acento="#c49a6c", seleccion="#4a3d30",
        hover="#342f2a", ok="#8aa66b", warn="#c9954f", neutro="#7d7366",
        galeria="#161412", tarjeta="#211e1b", badge_texto="#16130f",
    ),
}

# ----------------------------------------------------------------------------
# Iconos Nerd Font (Font Awesome). Solo se usan si la fuente activa es Nerd Font
# ----------------------------------------------------------------------------
ICONOS = {
    "juego": "\uf11b", "buscar": "\uf002", "nuevo": "\uf067", "guardar": "\uf0c7",
    "eliminar": "\uf1f8", "carpeta": "\uf07b", "abrir": "\uf07c", "imagen": "\uf03e",
    "lista": "\uf03a", "galeria": "\uf00a", "ok": "\uf00c", "curso": "\uf04b",
    "pendiente": "\uf017", "stats": "\uf080", "limpiar": "\uf12d", "tema": "\uf186",
    "fuente": "\uf031", "idioma": "\uf1ab", "orden": "\uf0dc", "quitar": "\uf00d",
    "codigo": "\uf02b",
}

PREFERIDAS = [
    "JetBrainsMono Nerd Font", "JetBrainsMono NF", "FiraCode Nerd Font", "FiraCode NF",
    "CaskaydiaCove Nerd Font", "CaskaydiaCove NF", "Hack Nerd Font", "Hack NF",
    "MesloLGS Nerd Font", "MesloLGS NF", "UbuntuMono Nerd Font", "Iosevka Nerd Font",
    "SauceCodePro Nerd Font",
]


# ----------------------------------------------------------------------------
# Preferencias
# ----------------------------------------------------------------------------
CONFIG_DEF = {"tema": "Oscuro", "fuente": "Automática", "tamano": 10}


def cargar_config():
    cfg = dict(CONFIG_DEF)
    try:
        with open(CONFIG_PATH, encoding="utf-8") as f:
            datos = json.load(f)
        if datos.get("tema") in TEMAS:
            cfg["tema"] = datos["tema"]
        if isinstance(datos.get("fuente"), str):
            cfg["fuente"] = datos["fuente"]
        if isinstance(datos.get("tamano"), int):
            cfg["tamano"] = min(18, max(8, datos["tamano"]))
    except Exception:
        pass
    return cfg


def guardar_config(cfg):
    try:
        with open(CONFIG_PATH, "w", encoding="utf-8") as f:
            json.dump(cfg, f, ensure_ascii=False, indent=2)
    except Exception:
        pass


# ----------------------------------------------------------------------------
# Fuentes
# ----------------------------------------------------------------------------
def cargar_fuentes_locales():
    """Carga (solo para esta app) los .ttf/.otf de la carpeta 'fuentes'."""
    if not os.path.isdir(FUENTES_DIR):
        return
    archivos = [os.path.join(FUENTES_DIR, f) for f in sorted(os.listdir(FUENTES_DIR))
                if f.lower().endswith((".ttf", ".otf"))]
    if not archivos:
        return
    try:
        if sys.platform.startswith("win"):
            import ctypes
            for a in archivos:
                ctypes.windll.gdi32.AddFontResourceExW(a, 0x10, 0)   # FR_PRIVATE
        elif sys.platform.startswith("linux"):
            import ctypes
            import ctypes.util
            nombre = ctypes.util.find_library("fontconfig")
            if nombre:
                fc = ctypes.CDLL(nombre)
                fc.FcConfigAppFontAddFile.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
                fc.FcConfigAppFontAddFile.restype = ctypes.c_int
                for a in archivos:
                    fc.FcConfigAppFontAddFile(None, a.encode("utf-8"))
        # macOS: instala la fuente normalmente (doble clic en el .ttf)
    except Exception:
        pass


def es_nerd(nombre):
    n = nombre.lower()
    if "symbols" in n:               # "Symbols Nerd Font" solo trae iconos, no letras
        return False
    return "nerd font" in n or n.endswith((" nf", " nfm", " nfp")) or " nf " in n


def fuentes_nerd(familias):
    candidatas = {f for f in familias if not f.startswith("@") and es_nerd(f)}
    ordenadas = [f for f in PREFERIDAS if f in candidatas]
    ordenadas += sorted(candidatas - set(ordenadas), key=str.casefold)
    return ordenadas


# ----------------------------------------------------------------------------
# Base de datos
# ----------------------------------------------------------------------------
class BaseDatos:
    def __init__(self, ruta):
        self.con = sqlite3.connect(ruta)
        self.con.execute(
            """
            CREATE TABLE IF NOT EXISTS juegos (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                titulo      TEXT NOT NULL,
                descripcion TEXT DEFAULT '',
                estado      TEXT DEFAULT 'Sin empezar',
                idioma      TEXT DEFAULT 'Español',
                imagen      BLOB,
                ruta        TEXT DEFAULT '',
                codigo      TEXT DEFAULT ''
            )
            """
        )
        # Migración: bases creadas con la versión 1 no tienen la columna "ruta"
        columnas = [f[1] for f in self.con.execute("PRAGMA table_info(juegos)")]
        if "ruta" not in columnas:
            self.con.execute("ALTER TABLE juegos ADD COLUMN ruta TEXT DEFAULT ''")
        if "codigo" not in columnas:
            self.con.execute("ALTER TABLE juegos ADD COLUMN codigo TEXT DEFAULT ''")
        for columna, definicion in (
            ("pagina", "TEXT DEFAULT ''"),
            ("tipo", "TEXT DEFAULT 'Otro'"),
            ("puntuacion", "REAL DEFAULT 0"),
        ):
            if columna not in columnas:
                self.con.execute(f"ALTER TABLE juegos ADD COLUMN {columna} {definicion}")
        self.con.execute("""
            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                nombre TEXT NOT NULL COLLATE NOCASE UNIQUE
            )
        """)
        self.con.execute("""
            CREATE TABLE IF NOT EXISTS juego_tags (
                juego_id INTEGER NOT NULL REFERENCES juegos(id) ON DELETE CASCADE,
                tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                PRIMARY KEY (juego_id, tag_id)
            )
        """)
        self.con.execute("PRAGMA foreign_keys = ON")
        self.con.commit()

    def listar(self):
        """(id, titulo, descripcion, estado, idioma, ruta, codigo) sin imágenes."""
        return self.con.execute(
            "SELECT id, titulo, descripcion, estado, idioma, ruta, codigo FROM juegos"
        ).fetchall()

    def obtener(self, juego_id):
        """Registro completo del juego."""
        return self.con.execute(
            "SELECT id, titulo, descripcion, estado, idioma, ruta, codigo, imagen, "
            "pagina, tipo, puntuacion FROM juegos WHERE id = ?", (juego_id,)
        ).fetchone()

    def tags_juego(self, juego_id):
        return [r[0] for r in self.con.execute(
            "SELECT t.nombre FROM tags t JOIN juego_tags jt ON jt.tag_id=t.id "
            "WHERE jt.juego_id=? ORDER BY t.nombre COLLATE NOCASE", (juego_id,))]

    def todos_tags(self):
        return [r[0] for r in self.con.execute(
            "SELECT nombre FROM tags ORDER BY nombre COLLATE NOCASE")]

    def guardar_tags(self, juego_id, nombres):
        self.con.execute("DELETE FROM juego_tags WHERE juego_id=?", (juego_id,))
        for nombre in nombres:
            nombre = nombre.strip()
            if not nombre:
                continue
            self.con.execute("INSERT OR IGNORE INTO tags(nombre) VALUES (?)", (nombre,))
            tag_id = self.con.execute(
                "SELECT id FROM tags WHERE nombre=? COLLATE NOCASE", (nombre,)).fetchone()[0]
            self.con.execute("INSERT OR IGNORE INTO juego_tags(juego_id,tag_id) VALUES (?,?)",
                             (juego_id, tag_id))
        self.con.commit()

    def obtener_imagen(self, juego_id):
        fila = self.con.execute("SELECT imagen FROM juegos WHERE id = ?", (juego_id,)).fetchone()
        return fila[0] if fila else None

    def titulo_con_codigo(self, codigo, excluir_id=None):
        """Título de otro juego que ya use ese código (sin distinguir mayúsculas), o None."""
        fila = self.con.execute(
            "SELECT titulo FROM juegos WHERE codigo = ? COLLATE NOCASE AND id != ? LIMIT 1",
            (codigo, excluir_id or -1),
        ).fetchone()
        return fila[0] if fila else None

    def crear(self, titulo, descripcion, estado, idioma, ruta, codigo, imagen,
              pagina, tipo, puntuacion):
        cur = self.con.execute(
            "INSERT INTO juegos (titulo, descripcion, estado, idioma, ruta, codigo, imagen, "
            "pagina, tipo, puntuacion) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (titulo, descripcion, estado, idioma, ruta, codigo, imagen, pagina, tipo, puntuacion),
        )
        self.con.commit()
        return cur.lastrowid

    def actualizar(self, juego_id, titulo, descripcion, estado, idioma, ruta, codigo, imagen,
                   pagina, tipo, puntuacion):
        self.con.execute(
            "UPDATE juegos SET titulo=?, descripcion=?, estado=?, idioma=?, ruta=?, "
            "codigo=?, imagen=?, pagina=?, tipo=?, puntuacion=? WHERE id=?",
            (titulo, descripcion, estado, idioma, ruta, codigo, imagen, pagina, tipo,
             puntuacion, juego_id),
        )
        self.con.commit()

    def eliminar(self, juego_id):
        self.con.execute("DELETE FROM juego_tags WHERE juego_id = ?", (juego_id,))
        self.con.execute("DELETE FROM juegos WHERE id = ?", (juego_id,))
        self.con.commit()

    def estadisticas(self):
        total = self.con.execute("SELECT COUNT(*) FROM juegos").fetchone()[0]
        por_estado = dict(self.con.execute("SELECT estado, COUNT(*) FROM juegos GROUP BY estado"))
        por_idioma = dict(self.con.execute("SELECT idioma, COUNT(*) FROM juegos GROUP BY idioma"))
        return total, por_estado, por_idioma


# ----------------------------------------------------------------------------
# Utilidades
# ----------------------------------------------------------------------------
def comprimir_imagen(ruta):
    """Abre una imagen, la reduce y la devuelve como bytes WebP."""
    img = Image.open(ruta)
    if img.mode in ("RGBA", "LA") or (img.mode == "P" and "transparency" in img.info):
        img = img.convert("RGBA")
        fondo = Image.new("RGB", img.size, (255, 255, 255))
        fondo.paste(img, mask=img.split()[3])
        img = fondo
    else:
        img = img.convert("RGB")

    img.thumbnail(IMG_MAX, Image.LANCZOS)
    buf = io.BytesIO()
    try:
        img.save(buf, "WEBP", quality=IMG_CALIDAD, method=6)
    except Exception:                      # por si WebP no estuviera disponible
        buf = io.BytesIO()
        img.save(buf, "JPEG", quality=IMG_CALIDAD, optimize=True)
    return buf.getvalue()


def abrir_en_explorador(ruta):
    """Abre una carpeta con el explorador del sistema (Windows, Linux, macOS)."""
    if os.path.isfile(ruta):
        ruta = os.path.dirname(ruta)
    if sys.platform.startswith("win"):
        os.startfile(ruta)                          # noqa
    elif sys.platform == "darwin":
        subprocess.Popen(["open", ruta])
    else:
        subprocess.Popen(["xdg-open", ruta])


def indice(lista, valor):
    return lista.index(valor) if valor in lista else len(lista)


def normalizar_codigo(texto):
    """'RY-4762560' y 'ry 4762560' y 'RY4762560' se comparan igual."""
    return re.sub(r"[\s\-_]", "", (texto or "").casefold())


# ----------------------------------------------------------------------------
# Galería de portadas
# ----------------------------------------------------------------------------
class Galeria(tk.Frame):
    CARD_W = 148

    def __init__(self, master, paleta, obtener_foto, al_click, al_doble_click):
        super().__init__(master, bg=paleta["galeria"])
        self.p = paleta
        self.obtener_foto = obtener_foto
        self.al_click = al_click
        self.al_doble_click = al_doble_click
        self.tarjetas = []          # [(id, frame)]
        self.cols = 1
        self.seleccionado = None
        self.card_h = 262

        base = tkfont.nametofont("TkDefaultFont")
        self.f_titulo = base.copy()
        self.f_titulo.configure(weight="bold")
        self.f_badge = base.copy()
        self.f_badge.configure(weight="bold")
        self.f_codigo = base.copy()

        self.canvas = tk.Canvas(self, bg=paleta["galeria"], highlightthickness=0)
        self.sb = ttk.Scrollbar(self, orient="vertical", command=self.canvas.yview)
        self.canvas.configure(yscrollcommand=self.sb.set)
        self.sb.pack(side="right", fill="y")
        self.canvas.pack(side="left", fill="both", expand=True)

        self.interior = tk.Frame(self.canvas, bg=paleta["galeria"])
        self.ventana = self.canvas.create_window((0, 0), window=self.interior, anchor="nw")
        self.canvas.bind("<Configure>", self._al_redimensionar)
        self.interior.bind(
            "<Configure>", lambda e: self.canvas.configure(scrollregion=self.canvas.bbox("all"))
        )

        # Rueda del ratón (Windows/macOS: MouseWheel; Linux: Button-4/5)
        self.bind_all("<MouseWheel>", self._rueda, add="+")
        self.bind_all("<Button-4>", self._rueda, add="+")
        self.bind_all("<Button-5>", self._rueda, add="+")

    # -- apariencia
    def set_colores(self, paleta):
        self.p = paleta
        self.configure(bg=paleta["galeria"])
        self.canvas.configure(bg=paleta["galeria"])
        self.interior.configure(bg=paleta["galeria"])

    def actualizar_fuentes(self, familia, tam):
        self.f_titulo.configure(family=familia, size=max(8, tam - 1), weight="bold")
        self.f_badge.configure(family=familia, size=max(7, tam - 2), weight="bold")
        self.f_codigo.configure(family=familia, size=max(7, tam - 2), weight="normal")
        self.card_h = (PORTADA_H + 6 + 2 * self.f_titulo.metrics("linespace") + 4
                       + self.f_badge.metrics("linespace") + 12 + 4
                       + self.f_codigo.metrics("linespace") + 2)

    def _color_estado(self, estado):
        return {"Sin empezar": self.p["neutro"], "Empezado": self.p["warn"],
                "Terminado": self.p["ok"]}.get(estado, self.p["neutro"])

    # -- scroll
    def _rueda(self, e):
        if not self.winfo_ismapped():
            return
        w = self.winfo_containing(e.x_root, e.y_root)
        if w is None or not str(w).startswith(str(self)):
            return
        if getattr(e, "num", 0) == 4:
            d = -1
        elif getattr(e, "num", 0) == 5:
            d = 1
        else:
            d = -1 if e.delta > 0 else 1
        self.canvas.yview_scroll(d * 2, "units")

    def _al_redimensionar(self, e):
        self.canvas.itemconfigure(self.ventana, width=e.width)
        nuevas = max(1, e.width // (self.CARD_W + 12))
        if nuevas != self.cols:
            self.cols = nuevas
            self._colocar()

    def _colocar(self):
        cols = max(1, self.cols)
        for i, (_, tarjeta) in enumerate(self.tarjetas):
            tarjeta.grid(row=i // cols, column=i % cols, padx=6, pady=6)

    # -- contenido
    def mostrar(self, juegos):
        """juegos: lista de (id, titulo, estado, codigo)"""
        for w in self.interior.winfo_children():
            w.destroy()
        self.tarjetas = []
        if not juegos:
            tk.Label(self.interior, text="No hay juegos que mostrar",
                     bg=self.p["galeria"], fg=self.p["tenue"]).grid(padx=20, pady=20)
            return
        for jid, titulo, estado, codigo in juegos:
            self.tarjetas.append((jid, self._crear_tarjeta(jid, titulo, estado, codigo)))
        self._colocar()
        self.seleccionar(self.seleccionado)

    def _crear_tarjeta(self, jid, titulo, estado, codigo):
        p = self.p
        card = tk.Frame(self.interior, width=self.CARD_W, height=self.card_h, bg=p["tarjeta"],
                        highlightthickness=2, highlightbackground=p["galeria"],
                        highlightcolor=p["galeria"])
        card.pack_propagate(False)

        foto = self.obtener_foto(jid)
        portada = tk.Label(card, image=foto, bg=p["tarjeta"], bd=0)
        portada.image = foto
        portada.pack(pady=(4, 2))

        badge = tk.Label(card, text=estado, bg=self._color_estado(estado),
                         fg=p["badge_texto"], font=self.f_badge)
        badge.pack(side="bottom", fill="x", padx=4, pady=(0, 4))

        lbl_codigo = tk.Label(card, text=codigo or " ", bg=p["tarjeta"], fg=p["tenue"],
                              font=self.f_codigo)
        lbl_codigo.pack(fill="x", padx=4)

        corto = titulo if len(titulo) <= 42 else titulo[:41] + "…"
        lbl = tk.Label(card, text=corto, bg=p["tarjeta"], fg=p["texto"], font=self.f_titulo,
                       wraplength=self.CARD_W - 14, justify="center", height=2)
        lbl.pack(fill="x", padx=4)

        for w in (card, portada, lbl_codigo, lbl, badge):
            w.bind("<Button-1>", lambda e, j=jid: self.al_click(j))
            w.bind("<Double-Button-1>", lambda e, j=jid: self.al_doble_click(j))
        return card

    def seleccionar(self, jid):
        self.seleccionado = jid
        for tid, tarjeta in self.tarjetas:
            color = self.p["acento"] if tid == jid else self.p["galeria"]
            tarjeta.configure(highlightbackground=color, highlightcolor=color)


# ----------------------------------------------------------------------------
# Aplicación
# ----------------------------------------------------------------------------
class App(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title("Catálogo de juegos RPG Maker")
        self.geometry("1320x900")
        self.minsize(1120, 760)

        self.cfg = cargar_config()
        self.estilo = ttk.Style(self)
        self.familia_sistema = tkfont.nametofont("TkDefaultFont").actual("family")
        self.nerds = fuentes_nerd(tkfont.families(self))
        self._resolver_fuente()
        self.p = TEMAS[self.cfg["tema"]]

        self.db = BaseDatos(DB_PATH)
        self.juego_actual = None      # id del juego en edición (None = nuevo)
        self.imagen_bytes = None      # imagen (comprimida) del formulario
        self.foto_tk = None           # referencia para que tkinter no la borre
        self.cache_fotos = {}         # id -> PhotoImage de la galería
        self._textos = []             # widgets con texto + icono (se refrescan al cambiar fuente)
        self.combos = []              # comboboxes (para colorear su lista desplegable)

        self._crear_widgets()
        self.aplicar_fuente()
        self.aplicar_tema()
        self.actualizar_estadisticas()
        self.bind("<Control-f>", lambda e: self.entry_buscar.focus_set())

    # ------------------------------------------------------- Fuente / iconos
    def _resolver_fuente(self):
        eleccion = self.cfg["fuente"]
        if eleccion == "Predeterminada del sistema":
            fam = self.familia_sistema
        elif eleccion in self.nerds:
            fam = eleccion
        elif self.nerds:
            fam = self.nerds[0]
        else:
            fam = self.familia_sistema
        self.familia = fam
        self.nerd = es_nerd(fam)

    def ic(self, icono, texto):
        if self.nerd and icono in ICONOS:
            return f"{ICONOS[icono]}  {texto}"
        return texto

    def reg(self, widget, icono, texto):
        """Asigna texto (con icono si hay Nerd Font) y lo recuerda para refrescarlo."""
        self._textos.append((widget, icono, texto))
        widget.configure(text=self.ic(icono, texto))
        return widget

    def _actualizar_textos(self):
        for widget, icono, texto in self._textos:
            widget.configure(text=self.ic(icono, texto))

    def aplicar_fuente(self):
        tam = self.cfg["tamano"]
        for nombre in ("TkDefaultFont", "TkTextFont", "TkMenuFont", "TkHeadingFont",
                       "TkCaptionFont", "TkTooltipFont"):
            try:
                tkfont.nametofont(nombre).configure(family=self.familia, size=tam)
            except tk.TclError:
                pass
        self.galeria.actualizar_fuentes(self.familia, tam)
        alto = tkfont.nametofont("TkDefaultFont").metrics("linespace") + 8
        self.estilo.configure("Treeview", rowheight=alto)
        if self.nerd:
            aviso = ""
        elif self.nerds:
            aviso = "Elige una Nerd Font para ver los iconos"
        else:
            aviso = "Sin Nerd Font: copia un .ttf en la carpeta «fuentes» y reinicia"
        self.lbl_aviso.config(text=aviso)

    # ------------------------------------------------------------------ Tema
    def aplicar_tema(self):
        p = self.p = TEMAS[self.cfg["tema"]]
        st = self.estilo
        st.theme_use("clam")           # único tema ttk que se deja recolorear en todos los sistemas
        self.configure(bg=p["fondo"])

        st.configure(".", background=p["fondo"], foreground=p["texto"],
                     fieldbackground=p["campo"], bordercolor=p["borde"],
                     darkcolor=p["borde"], lightcolor=p["borde"],
                     troughcolor=p["superficie"], focuscolor=p["acento"],
                     selectbackground=p["seleccion"], selectforeground=p["texto"])
        st.configure("TFrame", background=p["fondo"])
        st.configure("TLabel", background=p["fondo"], foreground=p["texto"])
        st.configure("Tenue.TLabel", background=p["fondo"], foreground=p["tenue"])
        st.configure("TLabelframe", background=p["fondo"], bordercolor=p["borde"])
        st.configure("TLabelframe.Label", background=p["fondo"], foreground=p["tenue"])

        st.configure("TButton", background=p["superficie"], foreground=p["texto"],
                     bordercolor=p["borde"], padding=(10, 5), relief="flat")
        st.map("TButton",
               background=[("pressed", p["seleccion"]), ("active", p["hover"])],
               bordercolor=[("focus", p["acento"])])
        st.configure("Toolbutton", background=p["superficie"], foreground=p["texto"],
                     bordercolor=p["borde"], padding=(10, 4), relief="flat")
        st.map("Toolbutton",
               background=[("selected", p["seleccion"]), ("active", p["hover"])])

        for nombre in ("TEntry", "TSpinbox"):
            st.configure(nombre, fieldbackground=p["campo"], foreground=p["texto"],
                         insertcolor=p["texto"], bordercolor=p["borde"],
                         arrowcolor=p["texto"], background=p["superficie"])
            st.map(nombre,
                   bordercolor=[("focus", p["acento"])],
                   lightcolor=[("focus", p["acento"])],
                   darkcolor=[("focus", p["acento"])])
        st.configure("TCombobox", fieldbackground=p["campo"], background=p["superficie"],
                     foreground=p["texto"], arrowcolor=p["texto"], bordercolor=p["borde"],
                     selectbackground=p["campo"], selectforeground=p["texto"])
        st.map("TCombobox",
               fieldbackground=[("readonly", p["campo"])],
               foreground=[("readonly", p["texto"])],
               selectbackground=[("readonly", p["campo"])],
               selectforeground=[("readonly", p["texto"])],
               background=[("active", p["hover"])],
               bordercolor=[("focus", p["acento"])])

        st.configure("Vertical.TScrollbar", background=p["borde"], troughcolor=p["fondo"],
                     bordercolor=p["fondo"], arrowcolor=p["texto"])
        st.map("Vertical.TScrollbar", background=[("active", p["tenue"])])
        st.configure("Horizontal.TProgressbar", background=p["ok"], troughcolor=p["campo"],
                     bordercolor=p["borde"], lightcolor=p["ok"], darkcolor=p["ok"])

        st.configure("Treeview", background=p["campo"], fieldbackground=p["campo"],
                     foreground=p["texto"], bordercolor=p["borde"], borderwidth=0)
        st.map("Treeview",
               background=[("selected", p["seleccion"])],
               foreground=[("selected", p["texto"])])
        st.configure("Treeview.Heading", background=p["superficie"], foreground=p["texto"],
                     relief="flat", bordercolor=p["borde"])
        st.map("Treeview.Heading", background=[("active", p["hover"])])
        self.tabla.tag_configure("Terminado", foreground=p["ok"])
        self.tabla.tag_configure("Empezado", foreground=p["warn"])

        # Widgets de tk "clásico" (no ttk)
        self.txt_desc.configure(bg=p["campo"], fg=p["texto"], insertbackground=p["texto"],
                                selectbackground=p["seleccion"], selectforeground=p["texto"],
                                highlightbackground=p["borde"], highlightcolor=p["acento"])
        self.marco_portada.configure(bg=p["campo"], highlightbackground=p["borde"])
        self.lbl_img.configure(bg=p["campo"], fg=p["tenue"])

        # Lista desplegable de los combobox (es un Listbox de tk)
        self.option_add("*TCombobox*Listbox.background", p["campo"])
        self.option_add("*TCombobox*Listbox.foreground", p["texto"])
        self.option_add("*TCombobox*Listbox.selectBackground", p["seleccion"])
        self.option_add("*TCombobox*Listbox.selectForeground", p["texto"])
        for cb in self.combos:
            try:
                pop = self.tk.call("ttk::combobox::PopdownWindow", str(cb))
                self.tk.call(f"{pop}.f.l", "configure", "-background", p["campo"],
                             "-foreground", p["texto"], "-selectbackground", p["seleccion"],
                             "-selectforeground", p["texto"])
            except tk.TclError:
                pass

        self.galeria.set_colores(p)
        self.cache_fotos.clear()          # los marcadores "Sin portada" dependen del tema
        self._barra_titulo(self.cfg["tema"] != "Claro")
        self.refrescar_lista()

    def _barra_titulo(self, oscuro):
        """Barra de título oscura en Windows 10/11 (en Linux la decide tu gestor de ventanas)."""
        if not sys.platform.startswith("win"):
            return
        try:
            import ctypes
            self.update_idletasks()
            hwnd = ctypes.windll.user32.GetParent(self.winfo_id())
            valor = ctypes.c_int(1 if oscuro else 0)
            for atributo in (20, 19):
                if ctypes.windll.dwmapi.DwmSetWindowAttribute(
                        hwnd, atributo, ctypes.byref(valor), ctypes.sizeof(valor)) == 0:
                    break
        except Exception:
            pass

    def cambiar_tema(self, _e=None):
        self.cfg["tema"] = self.var_tema.get()
        guardar_config(self.cfg)
        self.aplicar_tema()

    def cambiar_fuente(self, _e=None):
        try:
            tam = int(self.var_tamano.get())
        except ValueError:
            tam = self.cfg["tamano"]
        tam = min(18, max(8, tam))
        self.var_tamano.set(str(tam))
        self.cfg["fuente"] = self.var_fuente.get()
        self.cfg["tamano"] = tam
        guardar_config(self.cfg)
        self._resolver_fuente()
        self.aplicar_fuente()
        self._actualizar_textos()
        self.actualizar_estadisticas()
        self.refrescar_lista()

    # ------------------------------------------------------------------ UI
    def _nuevo_combo(self, padre, variable, valores, ancho):
        cb = ttk.Combobox(padre, textvariable=variable, values=valores,
                          state="readonly", width=ancho)
        self.combos.append(cb)
        return cb

    def _crear_widgets(self):
        cont = ttk.Frame(self, padding=10)
        cont.pack(fill="both", expand=True)
        cont.columnconfigure(0, weight=3)
        cont.columnconfigure(1, weight=2)
        cont.rowconfigure(2, weight=1)

        # ---------- Apariencia ----------
        apar = ttk.Frame(cont)
        apar.grid(row=0, column=0, columnspan=2, sticky="ew", pady=(0, 8))
        apar.columnconfigure(0, weight=1)
        self.lbl_aviso = ttk.Label(apar, text="", style="Tenue.TLabel")
        self.lbl_aviso.grid(row=0, column=0, sticky="w")

        ctrl = ttk.Frame(apar)
        ctrl.grid(row=0, column=1, sticky="e")
        self.reg(ttk.Label(ctrl), "tema", "Tema").pack(side="left", padx=(0, 4))
        self.var_tema = tk.StringVar(value=self.cfg["tema"])
        cb = self._nuevo_combo(ctrl, self.var_tema, list(TEMAS), 8)
        cb.pack(side="left", padx=(0, 12))
        cb.bind("<<ComboboxSelected>>", self.cambiar_tema)

        self.reg(ttk.Label(ctrl), "fuente", "Fuente").pack(side="left", padx=(0, 4))
        valores_fuente = ["Automática"] + self.nerds + ["Predeterminada del sistema"]
        inicial = self.cfg["fuente"] if self.cfg["fuente"] in valores_fuente else "Automática"
        self.var_fuente = tk.StringVar(value=inicial)
        cb = self._nuevo_combo(ctrl, self.var_fuente, valores_fuente, 28)
        cb.pack(side="left", padx=(0, 6))
        cb.bind("<<ComboboxSelected>>", self.cambiar_fuente)

        self.var_tamano = tk.StringVar(value=str(self.cfg["tamano"]))
        sp = ttk.Spinbox(ctrl, from_=8, to=18, width=4, textvariable=self.var_tamano,
                         command=self.cambiar_fuente)
        sp.pack(side="left")
        sp.bind("<Return>", self.cambiar_fuente)
        sp.bind("<FocusOut>", self.cambiar_fuente)

        # ---------- Estadísticas ----------
        est = ttk.LabelFrame(cont, padding=(10, 4))
        self.reg(est, "stats", "Estadísticas")
        est.grid(row=1, column=0, columnspan=2, sticky="ew", pady=(0, 8))
        est.columnconfigure(0, weight=1)
        self.lbl_stats = ttk.Label(est, text="")
        self.lbl_stats.grid(row=0, column=0, sticky="w")
        self.barra = ttk.Progressbar(est, maximum=100, length=220)
        self.barra.grid(row=0, column=1, padx=8)
        self.lbl_pct = ttk.Label(est, text="0% completado", width=16)
        self.lbl_pct.grid(row=0, column=2, sticky="e")
        self.lbl_idiomas = ttk.Label(est, text="", style="Tenue.TLabel")
        self.lbl_idiomas.grid(row=1, column=0, columnspan=3, sticky="w", pady=(2, 0))

        # ---------- Panel izquierdo ----------
        izq = ttk.Frame(cont)
        izq.grid(row=2, column=0, sticky="nsew", padx=(0, 10))
        izq.rowconfigure(3, weight=1)
        izq.columnconfigure(0, weight=1)

        # Buscador
        barra = ttk.Frame(izq)
        barra.grid(row=0, column=0, sticky="ew")
        barra.columnconfigure(1, weight=1)
        self.reg(ttk.Label(barra), "buscar", "Buscar").grid(row=0, column=0, padx=(0, 6))
        self.var_buscar = tk.StringVar()
        self.var_buscar.trace_add("write", lambda *a: self.refrescar_lista())
        self.entry_buscar = ttk.Entry(barra, textvariable=self.var_buscar)
        self.entry_buscar.grid(row=0, column=1, sticky="ew")

        # Filtros + orden
        filtros = ttk.Frame(izq)
        filtros.grid(row=1, column=0, sticky="ew", pady=6)
        self.var_f_estado = tk.StringVar(value="Todos")
        self.var_f_idioma = tk.StringVar(value="Todos")
        self.var_f_tag = tk.StringVar(value="Todos")
        self.var_orden = tk.StringVar(value=ORDENES[0])

        ttk.Label(filtros, text="Estado:").pack(side="left")
        self._combo_filtro(filtros, self.var_f_estado, ["Todos"] + ESTADOS, 11).pack(side="left", padx=(4, 10))
        ttk.Label(filtros, text="Idioma:").pack(side="left")
        self._combo_filtro(filtros, self.var_f_idioma, ["Todos"] + IDIOMAS, 9).pack(side="left", padx=(4, 10))
        ttk.Label(filtros, text="Tag:").pack(side="left")
        self.cb_f_tag = self._combo_filtro(filtros, self.var_f_tag, ["Todos"], 13)
        self.cb_f_tag.pack(side="left", padx=(4, 10))
        ttk.Label(filtros, text="Orden:").pack(side="left")
        self._combo_filtro(filtros, self.var_orden, ORDENES, 13).pack(side="left", padx=(4, 10))
        self.reg(ttk.Button(filtros, command=self.limpiar_filtros), "limpiar", "Limpiar").pack(side="left")

        # Selector de vista + contador
        fila_vista = ttk.Frame(izq)
        fila_vista.grid(row=2, column=0, sticky="ew", pady=(0, 6))
        ttk.Label(fila_vista, text="Vista:").pack(side="left")
        self.vista = tk.StringVar(value="lista")
        self.reg(ttk.Radiobutton(fila_vista, value="lista", variable=self.vista,
                                 style="Toolbutton", command=self.cambiar_vista),
                 "lista", "Lista").pack(side="left", padx=(6, 2))
        self.reg(ttk.Radiobutton(fila_vista, value="galeria", variable=self.vista,
                                 style="Toolbutton", command=self.cambiar_vista),
                 "galeria", "Galería").pack(side="left", padx=2)
        self.lbl_total = ttk.Label(fila_vista, text="", style="Tenue.TLabel")
        self.lbl_total.pack(side="right")

        # Contenedor de vistas
        self.vistas = ttk.Frame(izq)
        self.vistas.grid(row=3, column=0, sticky="nsew")
        self.vistas.rowconfigure(0, weight=1)
        self.vistas.columnconfigure(0, weight=1)

        # -- Vista lista
        self.marco_lista = ttk.Frame(self.vistas)
        self.marco_lista.rowconfigure(0, weight=1)
        self.marco_lista.columnconfigure(0, weight=1)
        self.tabla = ttk.Treeview(self.marco_lista,
                                  columns=("codigo", "titulo", "pagina", "tipo", "estado",
                                           "idioma", "puntuacion"),
                                  show="headings", selectmode="browse")
        for col, titulo_col, ancho, ancla in (
            ("codigo", "Código", 100, "w"), ("titulo", "Título", 210, "w"),
            ("pagina", "Página", 115, "center"), ("tipo", "Tipo", 105, "center"),
            ("estado", "Estado", 95, "center"), ("idioma", "Idioma", 80, "center"),
            ("puntuacion", "★", 55, "center")):
            self.tabla.heading(col, text=titulo_col)
            self.tabla.column(col, width=ancho, anchor=ancla)
        self.tabla.grid(row=0, column=0, sticky="nsew")
        sb = ttk.Scrollbar(self.marco_lista, orient="vertical", command=self.tabla.yview)
        sb.grid(row=0, column=1, sticky="ns")
        self.tabla.configure(yscrollcommand=sb.set)
        self.tabla.bind("<<TreeviewSelect>>", self.al_seleccionar_tabla)
        self.tabla.bind("<Double-1>", self.doble_click_tabla)

        # -- Vista galería
        self.galeria = Galeria(self.vistas, self.p, self.foto_miniatura,
                               self.click_galeria, self.abrir_carpeta_de)

        self.marco_lista.grid(row=0, column=0, sticky="nsew")

        # ---------- Panel derecho: formulario ----------
        der = ttk.LabelFrame(cont, padding=10)
        self.reg(der, "juego", "Juego")
        der.grid(row=2, column=1, sticky="nsew")
        der.columnconfigure(0, weight=1)
        der.rowconfigure(3, weight=1)

        fila_t = ttk.Frame(der)
        fila_t.grid(row=0, column=0, sticky="ew", pady=(0, 8))
        fila_t.columnconfigure(1, weight=1)
        self.reg(ttk.Label(fila_t), "codigo", "Código").grid(row=0, column=0, sticky="w")
        ttk.Label(fila_t, text="Título").grid(row=0, column=1, sticky="w", padx=(8, 0))
        self.var_codigo = tk.StringVar()
        ttk.Entry(fila_t, textvariable=self.var_codigo, width=14).grid(row=1, column=0, sticky="w")
        self.var_titulo = tk.StringVar()
        ttk.Entry(fila_t, textvariable=self.var_titulo).grid(row=1, column=1, sticky="ew", padx=(8, 0))

        ttk.Label(der, text="Descripción").grid(row=2, column=0, sticky="w")
        marco_desc = ttk.Frame(der)
        marco_desc.grid(row=3, column=0, sticky="nsew", pady=(0, 8))
        marco_desc.rowconfigure(0, weight=1)
        marco_desc.columnconfigure(0, weight=1)
        self.txt_desc = tk.Text(marco_desc, height=5, wrap="word", font="TkTextFont",
                                relief="flat", highlightthickness=1, padx=6, pady=4)
        self.txt_desc.grid(row=0, column=0, sticky="nsew")
        sb2 = ttk.Scrollbar(marco_desc, orient="vertical", command=self.txt_desc.yview)
        sb2.grid(row=0, column=1, sticky="ns")
        self.txt_desc.configure(yscrollcommand=sb2.set)

        fila = ttk.Frame(der)
        fila.grid(row=4, column=0, sticky="ew", pady=(0, 8))
        fila.columnconfigure(0, weight=1)
        fila.columnconfigure(1, weight=1)
        ttk.Label(fila, text="Estado").grid(row=0, column=0, sticky="w")
        ttk.Label(fila, text="Idioma").grid(row=0, column=1, sticky="w", padx=(8, 0))
        self.var_estado = tk.StringVar(value=ESTADOS[0])
        self._nuevo_combo(fila, self.var_estado, ESTADOS, 10).grid(row=1, column=0, sticky="ew")
        self.var_idioma = tk.StringVar(value=IDIOMAS[0])
        self._nuevo_combo(fila, self.var_idioma, IDIOMAS, 10).grid(row=1, column=1, sticky="ew", padx=(8, 0))

        # Página y tipo
        fila_meta = ttk.Frame(der)
        fila_meta.grid(row=5, column=0, sticky="ew", pady=(0, 6))
        fila_meta.columnconfigure(0, weight=1)
        fila_meta.columnconfigure(1, weight=1)
        ttk.Label(fila_meta, text="Página").grid(row=0, column=0, sticky="w")
        ttk.Label(fila_meta, text="Tipo").grid(row=0, column=1, sticky="w", padx=(8, 0))
        self.var_pagina = tk.StringVar()
        self._nuevo_combo(fila_meta, self.var_pagina, PAGINAS, 18).grid(
            row=1, column=0, sticky="ew")
        self.var_tipo = tk.StringVar(value=TIPOS[-1])
        self._nuevo_combo(fila_meta, self.var_tipo, TIPOS, 14).grid(
            row=1, column=1, sticky="ew", padx=(8, 0))

        fila_punt = ttk.Frame(der)
        fila_punt.grid(row=6, column=0, sticky="ew", pady=(0, 6))
        ttk.Label(fila_punt, text="Puntuación (0 a 5 estrellas)").pack(side="left")
        self.var_puntuacion = tk.StringVar(value="0.0")
        self._nuevo_combo(fila_punt, self.var_puntuacion, PUNTUACIONES, 7).pack(
            side="left", padx=8)

        # Tags: separados por comas; se crean al guardar
        ttk.Label(der, text="Tags (separados por comas)").grid(row=7, column=0, sticky="w")
        fila_tags = ttk.Frame(der)
        fila_tags.grid(row=8, column=0, sticky="ew", pady=(0, 6))
        fila_tags.columnconfigure(0, weight=1)
        self.var_tags = tk.StringVar()
        ttk.Entry(fila_tags, textvariable=self.var_tags).grid(row=0, column=0, sticky="ew")
        ttk.Button(fila_tags, text="+ Añadir tag", command=self.agregar_tag).grid(
            row=0, column=1, padx=(5, 0))

        # Ruta del juego
        self.reg(ttk.Label(der), "carpeta", "Carpeta del juego").grid(row=9, column=0, sticky="w")
        fila_ruta = ttk.Frame(der)
        fila_ruta.grid(row=10, column=0, sticky="ew", pady=(0, 8))
        fila_ruta.columnconfigure(0, weight=1)
        self.var_ruta = tk.StringVar()
        ttk.Entry(fila_ruta, textvariable=self.var_ruta).grid(row=0, column=0, sticky="ew")
        ttk.Button(fila_ruta, text="…", width=3, command=self.elegir_carpeta).grid(row=0, column=1, padx=(4, 0))
        self.reg(ttk.Button(fila_ruta, command=self.abrir_carpeta_form),
                 "abrir", "Abrir").grid(row=0, column=2, padx=(4, 0))

        # Imagen
        marco_img = ttk.Frame(der)
        marco_img.grid(row=11, column=0, sticky="ew", pady=(0, 8))
        self.marco_portada = tk.Frame(marco_img, width=190, height=150,
                                      highlightthickness=1)
        self.marco_portada.pack_propagate(False)
        self.marco_portada.pack(side="left")
        self.lbl_img = tk.Label(self.marco_portada, text="Sin imagen", bd=0)
        self.lbl_img.place(relx=0.5, rely=0.5, anchor="center")
        botones_img = ttk.Frame(marco_img)
        botones_img.pack(side="left", padx=10, anchor="n")
        self.reg(ttk.Button(botones_img, command=self.elegir_imagen),
                 "imagen", "Elegir imagen…").pack(fill="x", pady=(0, 4))
        self.reg(ttk.Button(botones_img, command=self.quitar_imagen),
                 "quitar", "Quitar imagen").pack(fill="x")
        self.lbl_peso = ttk.Label(botones_img, text="", style="Tenue.TLabel")
        self.lbl_peso.pack(pady=(6, 0))

        # Botones CRUD
        botones = ttk.Frame(der)
        botones.grid(row=12, column=0, sticky="ew")
        for i in range(3):
            botones.columnconfigure(i, weight=1)
        self.reg(ttk.Button(botones, command=self.nuevo), "nuevo", "Nuevo").grid(
            row=0, column=0, sticky="ew", padx=(0, 4))
        self.reg(ttk.Button(botones, command=self.guardar), "guardar", "Guardar").grid(
            row=0, column=1, sticky="ew", padx=4)
        self.reg(ttk.Button(botones, command=self.eliminar), "eliminar", "Eliminar").grid(
            row=0, column=2, sticky="ew", padx=(4, 0))

    def _combo_filtro(self, padre, variable, valores, ancho):
        cb = self._nuevo_combo(padre, variable, valores, ancho)
        cb.bind("<<ComboboxSelected>>", lambda e: self.refrescar_lista())
        return cb

    # ------------------------------------------------ Filtros, orden y vista
    def limpiar_filtros(self):
        self.var_f_estado.set("Todos")
        self.var_f_idioma.set("Todos")
        self.var_f_tag.set("Todos")
        self.var_buscar.set("")
        self.refrescar_lista()

    def cambiar_vista(self):
        if self.vista.get() == "lista":
            self.galeria.grid_remove()
            self.marco_lista.grid(row=0, column=0, sticky="nsew")
        else:
            self.marco_lista.grid_remove()
            self.galeria.grid(row=0, column=0, sticky="nsew")
        self.refrescar_lista()

    def _filtrar(self):
        texto = self.var_buscar.get().strip().casefold()
        texto_norm = normalizar_codigo(texto)
        f_estado = self.var_f_estado.get()
        f_idioma = self.var_f_idioma.get()
        f_tag = self.var_f_tag.get()

        juegos = []
        for fila in self.db.listar():
            jid, titulo, desc, estado, idioma, _, codigo = fila
            tags = self.db.tags_juego(jid)
            extra = self.db.con.execute(
                "SELECT pagina, tipo, puntuacion FROM juegos WHERE id=?", (jid,)).fetchone()
            pagina, tipo, puntuacion = extra
            campos = " ".join([pagina or "", tipo or "", " ".join(tags)]).casefold()
            en_codigo = (texto in (codigo or "").casefold()
                         or (texto_norm and texto_norm in normalizar_codigo(codigo)))
            if texto and not (en_codigo or texto in titulo.casefold()
                              or texto in (desc or "").casefold() or texto in campos):
                continue
            if f_estado != "Todos" and estado != f_estado:
                continue
            if f_idioma != "Todos" and idioma != f_idioma:
                continue
            if f_tag != "Todos" and f_tag.casefold() not in [t.casefold() for t in tags]:
                continue
            juegos.append(fila)

        orden = self.var_orden.get()
        if orden == "Código":          # los juegos sin código quedan al final
            juegos.sort(key=lambda j: (not j[6], (j[6] or "").casefold(), j[1].casefold()))
        elif orden == "Nombre (Z-A)":
            juegos.sort(key=lambda j: j[1].casefold(), reverse=True)
        elif orden == "Estado":
            juegos.sort(key=lambda j: (indice(ESTADOS, j[3]), j[1].casefold()))
        elif orden == "Idioma":
            juegos.sort(key=lambda j: (indice(IDIOMAS, j[4]), j[1].casefold()))
        elif orden == "Más recientes":
            juegos.sort(key=lambda j: j[0], reverse=True)
        else:
            juegos.sort(key=lambda j: j[1].casefold())
        return juegos

    def refrescar_lista(self):
        juegos = self._filtrar()
        total = self.db.estadisticas()[0]
        tags_disponibles = ["Todos"] + self.db.todos_tags()
        if hasattr(self, "cb_f_tag"):
            self.cb_f_tag.configure(values=tags_disponibles)
            if self.var_f_tag.get() not in tags_disponibles:
                self.var_f_tag.set("Todos")
        self.lbl_total.config(text=f"Mostrando {len(juegos)} de {total}")

        if self.vista.get() == "lista":
            self.tabla.delete(*self.tabla.get_children())
            for jid, titulo, _, estado, idioma, _, codigo in juegos:
                pagina, tipo, puntuacion = self.db.con.execute(
                    "SELECT pagina, tipo, puntuacion FROM juegos WHERE id=?", (jid,)).fetchone()
                self.tabla.insert("", "end", iid=str(jid),
                                  values=(codigo, titulo, pagina, tipo, estado, idioma,
                                          f"{float(puntuacion or 0):.1f} ★"), tags=(estado,))
            if self.juego_actual is not None and self.tabla.exists(str(self.juego_actual)):
                self.tabla.selection_set(str(self.juego_actual))
                self.tabla.see(str(self.juego_actual))
        else:
            self.galeria.seleccionar(self.juego_actual)
            self.galeria.mostrar([(j[0], j[1], j[3], j[6]) for j in juegos])

    def actualizar_estadisticas(self):
        total, por_estado, por_idioma = self.db.estadisticas()
        terminados = por_estado.get("Terminado", 0)
        en_curso = por_estado.get("Empezado", 0)
        pendientes = por_estado.get("Sin empezar", 0)
        partes = [
            self.ic("juego", f"Total: {total}"),
            self.ic("ok", f"Terminados: {terminados}"),
            self.ic("curso", f"En curso: {en_curso}"),
            self.ic("pendiente", f"Pendientes: {pendientes}"),
        ]
        self.lbl_stats.config(text="      ".join(partes))
        pct = (terminados / total * 100) if total else 0
        self.barra["value"] = pct
        self.lbl_pct.config(text=f"{pct:.0f}% completado")
        self.lbl_idiomas.config(
            text=self.ic("idioma", "Por idioma:   "
                         + "     ".join(f"{i}: {por_idioma.get(i, 0)}" for i in IDIOMAS))
        )

    # ---------------------------------------------------------- Selección
    def al_seleccionar_tabla(self, _e=None):
        sel = self.tabla.selection()
        if not sel:
            return
        jid = int(sel[0])
        if jid == self.juego_actual:      # ya cargado: no pisar lo que estés editando
            return
        self.cargar_juego(jid)

    def click_galeria(self, jid):
        self.cargar_juego(jid)

    def cargar_juego(self, jid):
        fila = self.db.obtener(jid)
        if not fila:
            return
        _, titulo, desc, estado, idioma, ruta, codigo, imagen, pagina, tipo, puntuacion = fila
        self.juego_actual = jid
        self.var_codigo.set(codigo or "")
        self.var_titulo.set(titulo)
        self.txt_desc.delete("1.0", "end")
        self.txt_desc.insert("1.0", desc or "")
        self.var_estado.set(estado if estado in ESTADOS else ESTADOS[0])
        self.var_idioma.set(idioma if idioma in IDIOMAS else IDIOMAS[0])
        self.var_ruta.set(ruta or "")
        self.var_pagina.set(pagina or "")
        self.var_tipo.set(tipo if tipo in TIPOS else TIPOS[-1])
        self.var_puntuacion.set(f"{float(puntuacion or 0):.1f}")
        self.var_tags.set(", ".join(self.db.tags_juego(jid)))
        self.imagen_bytes = imagen
        self.mostrar_imagen()
        self.galeria.seleccionar(jid)

    def agregar_tag(self):
        nombre = simpledialog.askstring("Añadir tag", "Nombre de la nueva etiqueta:", parent=self)
        if not nombre or not nombre.strip():
            return
        actuales = [t.strip() for t in self.var_tags.get().split(",") if t.strip()]
        if nombre.strip().casefold() not in [t.casefold() for t in actuales]:
            actuales.append(nombre.strip())
        self.var_tags.set(", ".join(actuales))

    # --------------------------------------------------------- Carpeta/ruta
    def elegir_carpeta(self):
        inicial = self.var_ruta.get() if os.path.isdir(self.var_ruta.get()) else os.path.expanduser("~")
        ruta = filedialog.askdirectory(title="Carpeta del juego", initialdir=inicial)
        if ruta:
            self.var_ruta.set(ruta)

    def _abrir(self, ruta):
        ruta = (ruta or "").strip()
        if not ruta:
            messagebox.showinfo("Carpeta", "Este juego no tiene carpeta guardada.")
            return
        if not os.path.exists(ruta):
            messagebox.showwarning("Carpeta", f"La ruta ya no existe:\n{ruta}")
            return
        try:
            abrir_en_explorador(ruta)
        except Exception as e:
            messagebox.showerror("Error", f"No se pudo abrir la carpeta:\n{e}")

    def abrir_carpeta_form(self):
        self._abrir(self.var_ruta.get())

    def abrir_carpeta_de(self, jid):
        fila = self.db.obtener(jid)
        if fila:
            self._abrir(fila[5])

    def doble_click_tabla(self, e):
        item = self.tabla.identify_row(e.y)
        if item:
            self.abrir_carpeta_de(int(item))

    # -------------------------------------------------------------- Imagen
    def foto_miniatura(self, jid):
        """Portada para la galería (recortada para llenar el recuadro)."""
        if jid in self.cache_fotos:
            return self.cache_fotos[jid]
        datos = self.db.obtener_imagen(jid)
        portada = None
        if datos:
            try:
                img = Image.open(io.BytesIO(datos)).convert("RGB")
                portada = ImageOps.fit(img, (PORTADA_W, PORTADA_H), Image.LANCZOS,
                                       centering=(0.5, 0.4))
            except Exception:
                portada = None
        if portada is None:
            portada = Image.new("RGB", (PORTADA_W, PORTADA_H), self.p["superficie"])
            d = ImageDraw.Draw(portada)
            texto = "Sin portada"
            try:
                x0, y0, x1, y1 = d.textbbox((0, 0), texto)
                d.text(((PORTADA_W - (x1 - x0)) // 2, (PORTADA_H - (y1 - y0)) // 2),
                       texto, fill=self.p["tenue"])
            except Exception:
                pass
        foto = ImageTk.PhotoImage(portada)
        self.cache_fotos[jid] = foto
        return foto

    def mostrar_imagen(self):
        if self.imagen_bytes:
            try:
                img = Image.open(io.BytesIO(self.imagen_bytes))
                img.thumbnail((180, 140))
                self.foto_tk = ImageTk.PhotoImage(img)
                self.lbl_img.config(image=self.foto_tk, text="")
                self.lbl_peso.config(text=f"{len(self.imagen_bytes) / 1024:.1f} KB")
                return
            except Exception:
                pass
        self.foto_tk = None
        self.lbl_img.config(image="", text="Sin imagen")
        self.lbl_peso.config(text="")

    def elegir_imagen(self):
        ruta = filedialog.askopenfilename(
            title="Elegir imagen",
            filetypes=[("Imágenes", "*.png *.jpg *.jpeg *.webp *.bmp *.gif"),
                       ("Todos los archivos", "*.*")],
        )
        if not ruta:
            return
        try:
            self.imagen_bytes = comprimir_imagen(ruta)
        except Exception as e:
            messagebox.showerror("Error", f"No se pudo procesar la imagen:\n{e}")
            return
        self.mostrar_imagen()

    def quitar_imagen(self):
        self.imagen_bytes = None
        self.mostrar_imagen()

    # ---------------------------------------------------------------- CRUD
    def nuevo(self):
        self.juego_actual = None
        self.tabla.selection_remove(self.tabla.selection())
        self.galeria.seleccionar(None)
        self.var_codigo.set("")
        self.var_titulo.set("")
        self.txt_desc.delete("1.0", "end")
        self.var_estado.set(ESTADOS[0])
        self.var_idioma.set(IDIOMAS[0])
        self.var_pagina.set("")
        self.var_tipo.set(TIPOS[-1])
        self.var_puntuacion.set("0.0")
        self.var_tags.set("")
        self.var_ruta.set("")
        self.imagen_bytes = None
        self.mostrar_imagen()

    def guardar(self):
        titulo = self.var_titulo.get().strip()
        if not titulo:
            messagebox.showwarning("Falta el título", "Escribe el título del juego.")
            return
        desc = self.txt_desc.get("1.0", "end").strip()
        estado = self.var_estado.get()
        idioma = self.var_idioma.get()
        ruta = self.var_ruta.get().strip()
        codigo = self.var_codigo.get().strip()
        pagina = self.var_pagina.get().strip()
        tipo = self.var_tipo.get().strip() or TIPOS[-1]
        puntuacion = float(self.var_puntuacion.get())
        tags = list(dict.fromkeys(t.strip() for t in self.var_tags.get().split(",") if t.strip()))

        repetido = self.db.titulo_con_codigo(codigo, self.juego_actual) if codigo else None
        if repetido is not None and not messagebox.askyesno(
                "Código repetido",
                f"El código «{codigo}» ya lo tiene «{repetido}».\n\n¿Guardar de todos modos?"):
            return

        if self.juego_actual is None:
            self.juego_actual = self.db.crear(titulo, desc, estado, idioma, ruta, codigo,
                                              self.imagen_bytes, pagina, tipo, puntuacion)
        else:
            self.db.actualizar(self.juego_actual, titulo, desc, estado, idioma, ruta, codigo,
                               self.imagen_bytes, pagina, tipo, puntuacion)
            self.cache_fotos.pop(self.juego_actual, None)
        self.db.guardar_tags(self.juego_actual, tags)
        self.refrescar_lista()
        self.actualizar_estadisticas()

    def eliminar(self):
        if self.juego_actual is None:
            messagebox.showinfo("Eliminar", "Selecciona primero un juego de la lista.")
            return
        if messagebox.askyesno("Eliminar", f"¿Eliminar «{self.var_titulo.get()}»?"):
            self.db.eliminar(self.juego_actual)
            self.cache_fotos.pop(self.juego_actual, None)
            self.nuevo()
            self.refrescar_lista()
            self.actualizar_estadisticas()


if __name__ == "__main__":
    cargar_fuentes_locales()     # antes de crear la ventana, para que Tk vea las fuentes
    App().mainloop()
