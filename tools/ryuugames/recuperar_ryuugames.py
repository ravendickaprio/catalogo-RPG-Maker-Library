"""Interfaz Tkinter para recuperar metadatos desde Ryuugames (ver README.md).

Uso:  py -3.14 tools/ryuugames/recuperar_ryuugames.py
"""
import queue
import threading
import tkinter as tk
from pathlib import Path
from tkinter import filedialog, messagebox, ttk

import ryuugames as rg

COLUMNAS = [("sel", "Aplicar", 60), ("codigo", "Código", 120), ("codigo_busqueda", "Búsqueda", 100),
            ("titulo_actual", "Título actual", 180), ("titulo_encontrado", "Título encontrado", 220),
            ("titulo", "Reemplazar título", 110), ("validacion", "Validación", 100),
            ("extraccion", "Extracción", 120), ("motivo", "Motivo", 240)]


class App(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title("Recuperar metadatos desde Ryuugames")
        self.geometry("1300x760")
        self.db = None
        self.total, self.regs, self.sesion = 0, [], {}
        self.marcados, self.reemplazar = set(), set()
        self.cancel = threading.Event()
        self.cola = queue.Queue()
        self.trabajando = self.aplicando = False
        self._foto = None
        self._construir()
        self.protocol("WM_DELETE_WINDOW", self._cerrar)

    # ------------------------------------------------------------ layout
    def _construir(self):
        top = ttk.Frame(self, padding=8)
        top.pack(fill="x")
        ttk.Button(top, text="Seleccionar DB…", command=self.elegir_db).pack(side="left")
        self.lbl_db = ttk.Label(top, text="Ninguna base de datos seleccionada")
        self.lbl_db.pack(side="left", padx=8)

        self.lbl_cuentas = ttk.Label(self, padding=(8, 0), text="")
        self.lbl_cuentas.pack(fill="x")

        opc = ttk.Frame(self, padding=8)
        opc.pack(fill="x")
        ttk.Label(opc, text="Intervalo entre solicitudes (s):").pack(side="left")
        self.intervalo = tk.DoubleVar(value=3.0)
        ttk.Spinbox(opc, from_=1, to=30, increment=0.5, width=5, textvariable=self.intervalo).pack(side="left", padx=4)
        self.reintentar = tk.BooleanVar(value=False)
        ttk.Checkbutton(opc, text="Volver a buscar no encontrados y dudosos", variable=self.reintentar).pack(side="left", padx=12)
        self.btn_buscar = ttk.Button(opc, text="Buscar (no modifica la DB)", command=self.buscar, state="disabled")
        self.btn_buscar.pack(side="left", padx=4)
        self.btn_cancelar = ttk.Button(opc, text="Cancelar", command=self.cancel.set, state="disabled")
        self.btn_cancelar.pack(side="left")
        self.barra = ttk.Progressbar(opc, length=220)
        self.barra.pack(side="left", padx=12)
        self.lbl_estado = ttk.Label(opc, text="")
        self.lbl_estado.pack(side="left")

        medio = ttk.PanedWindow(self, orient="vertical")
        medio.pack(fill="both", expand=True, padx=8)
        marco = ttk.Frame(medio)
        self.tabla = ttk.Treeview(marco, columns=[c for c, _, _ in COLUMNAS], show="headings", selectmode="browse")
        for c, t, w in COLUMNAS:
            self.tabla.heading(c, text=t)
            self.tabla.column(c, width=w, stretch=c in ("titulo_encontrado", "motivo"))
        sb = ttk.Scrollbar(marco, orient="vertical", command=self.tabla.yview)
        self.tabla.configure(yscrollcommand=sb.set)
        self.tabla.pack(side="left", fill="both", expand=True)
        sb.pack(side="right", fill="y")
        self.tabla.tag_configure("verificada", foreground="#0a7d32")
        self.tabla.tag_configure("dudosa", foreground="#b26a00")
        self.tabla.tag_configure("aplicado", foreground="#777777")
        self.tabla.bind("<<TreeviewSelect>>", self._detalle)
        self.tabla.bind("<Button-1>", self._clic)
        self.tabla.bind("<space>", lambda e: self._alternar(self.tabla.focus(), "sel"))
        medio.add(marco, weight=3)

        det = ttk.Frame(medio)
        self.img = ttk.Label(det)
        self.img.pack(side="left", padx=4)
        self.txt = tk.Text(det, height=9, wrap="word", state="disabled")
        self.txt.pack(side="left", fill="both", expand=True)
        medio.add(det, weight=1)

        pie = ttk.Frame(self, padding=8)
        pie.pack(fill="x")
        ttk.Button(pie, text="Marcar todas las verificadas", command=self.marcar_todas).pack(side="left")
        ttk.Button(pie, text="Desmarcar todo", command=self.desmarcar).pack(side="left", padx=4)
        ttk.Label(pie, text="Clic o Espacio en «Aplicar» marca la fila; clic en «Reemplazar título» autoriza cambiar un título significativo.").pack(side="left", padx=8)
        self.btn_aplicar = ttk.Button(pie, text="Aplicar seleccionados…", command=self.aplicar, state="disabled")
        self.btn_aplicar.pack(side="right")

    # ------------------------------------------------------------ DB
    def elegir_db(self):
        if self.trabajando:
            return
        ruta = filedialog.askopenfilename(title="Base de datos SQLite",
                                          filetypes=[("SQLite", "*.db *.sqlite *.sqlite3"), ("Todos", "*.*")])
        if not ruta:
            return
        try:
            rg.configurar_log(ruta)
            self.total, self.regs = rg.leer_candidatos(ruta)
            self.sesion = rg.cargar_sesion(ruta)
        except Exception as e:
            messagebox.showerror("Base de datos", f"No se pudo leer la base de datos:\n{e}")
            return
        self.db = ruta
        self.lbl_db.config(text=ruta)
        self.marcados.clear()
        self.reemplazar.clear()
        self._refrescar()
        self.btn_buscar.config(state="normal")

    def _recargar(self):
        self.total, self.regs = rg.leer_candidatos(self.db)
        self.sesion = rg.cargar_sesion(self.db)
        self._refrescar()

    def _refrescar(self):
        r = rg.resumen(self.total, self.regs, self.sesion)
        pend = len(rg.pendientes(self.regs, self.sesion, self.reintentar.get()))
        self.lbl_cuentas.config(text=(
            f"Total: {r['Total de registros']}   Sin descripción: {r['Registros sin descripción']}   "
            f"Elegibles: {r['Registros elegibles']}   Excluidos (sin R): {r['Excluidos por no contener R']}   "
            f"Código inválido: {r['Código inválido']}   Pendientes de búsqueda: {pend}   |   "
            f"Verificadas: {r['Coincidencias verificadas']}   Dudosas: {r['Coincidencias dudosas']}   "
            f"No encontrados: {r['No encontrados']}   Errores HTTP: {r['Errores HTTP']}"))
        self.tabla.delete(*self.tabla.get_children())
        for reg in self.regs:
            s = self.sesion.get(str(reg["id"]))
            if not s:
                continue
            self.tabla.insert("", "end", iid=str(reg["id"]), values=self._valores(s),
                              tags=("aplicado",) if s.get("aplicado") else (s.get("validacion") or "",))
        self.btn_aplicar.config(state="normal" if self.marcados else "disabled")

    def _valores(self, s):
        i = s["id"]
        sel = "☑" if i in self.marcados else ("☐" if rg.seleccionable(s) else "—")
        if s.get("aplicado"):
            sel = "aplicado"
        tit = "auto" if s.get("titulo_provisional") else ("sí" if i in self.reemplazar else "no")
        return [sel, s["codigo"], s["codigo_busqueda"], s["titulo_actual"], s["titulo_encontrado"], tit,
                s["validacion"], s["extraccion"], s["motivo"]]

    # ------------------------------------------------------------ selección
    def _clic(self, ev):
        if self.tabla.identify("region", ev.x, ev.y) != "cell":
            return
        fila, col = self.tabla.identify_row(ev.y), self.tabla.identify_column(ev.x)
        nombre = COLUMNAS[int(col[1:]) - 1][0]
        if nombre in ("sel", "titulo"):
            self._alternar(fila, nombre)

    def _alternar(self, fila, nombre):
        s = self.sesion.get(fila)
        if not s or not rg.seleccionable(s) or self.trabajando:
            return
        i = s["id"]
        if nombre == "sel":
            self.marcados ^= {i}
        elif not s.get("titulo_provisional") and s.get("titulo_encontrado"):
            self.reemplazar ^= {i}
        self.tabla.item(fila, values=self._valores(s))
        self.btn_aplicar.config(state="normal" if self.marcados else "disabled")

    def marcar_todas(self):
        self.marcados = {s["id"] for s in self.sesion.values() if rg.seleccionable(s)}
        self._refrescar()

    def desmarcar(self):
        self.marcados.clear()
        self.reemplazar.clear()
        self._refrescar()

    def _detalle(self, _=None):
        s = self.sesion.get(self.tabla.focus())
        if not s:
            return
        texto = (f"URL: {s['url'] or '—'}\nImagen: {s['imagen_url'] or '—'}"
                 f"{'' if s['sin_imagen'] else '  (el registro ya tiene imagen: se conserva)'}\n"
                 f"Archivo: {s['imagen_archivo'] or '—'}\n\nDescripción ({len(s['descripcion'])} car.):\n{s['descripcion'] or '—'}")
        self.txt.config(state="normal")
        self.txt.delete("1.0", "end")
        self.txt.insert("1.0", texto)
        self.txt.config(state="disabled")
        self._foto = None
        if s.get("imagen_archivo") and Path(s["imagen_archivo"]).exists():
            try:
                from PIL import Image, ImageTk
                im = Image.open(s["imagen_archivo"])
                im.thumbnail((180, 140))
                self._foto = ImageTk.PhotoImage(im)
            except Exception:
                pass
        self.img.config(image=self._foto or "", text="" if self._foto else "sin vista previa")

    # ------------------------------------------------------------ trabajo en segundo plano
    def _ocupado(self, si):
        self.trabajando = si
        self.btn_buscar.config(state="disabled" if si else "normal")
        self.btn_cancelar.config(state="normal" if si else "disabled")
        self.btn_aplicar.config(state="disabled" if si or not self.marcados else "normal")

    def buscar(self):
        try:
            intervalo = max(1.0, float(self.intervalo.get()))
        except (tk.TclError, ValueError):
            intervalo = 3.0
        self.cancel.clear()
        self._ocupado(True)
        cli = rg.Cliente(intervalo=intervalo, cancel=self.cancel)
        db, reint = self.db, self.reintentar.get()

        def trabajo():
            try:
                sesion, parada = rg.buscar(db, cli, reint, lambda i, n, c: self.cola.put(("prog", i, n, c)))
                self.cola.put(("fin_busqueda", sesion, parada))
            except Exception as e:
                rg.log.exception("búsqueda")
                self.cola.put(("error", e))
        threading.Thread(target=trabajo, daemon=True).start()
        self.after(100, self._sondear)

    def aplicar(self):
        ids = sorted(self.marcados)
        if not messagebox.askyesno("Aplicar cambios", (
                f"Se actualizarán {len(ids)} registros (descripción; título si es provisional o autorizado; "
                "imagen solo si está vacía).\n\nAntes se creará un backup de la DB. ¿Continuar?")):
            return
        self._ocupado(True)
        self.aplicando = True
        db, sesion, reemplazar = self.db, self.sesion, set(self.reemplazar)

        def trabajo():
            try:
                res = rg.aplicar(db, sesion, ids, reemplazar)
                self.cola.put(("fin_aplicar", res))
            except Exception as e:
                rg.log.exception("aplicación")
                self.cola.put(("error", e))
        threading.Thread(target=trabajo, daemon=True).start()
        self.after(100, self._sondear)

    def _sondear(self):
        try:
            while True:
                msg = self.cola.get_nowait()
                if msg[0] == "prog":
                    _, i, n, c = msg
                    self.barra.config(maximum=max(n, 1), value=i)
                    self.lbl_estado.config(text=f"{i}/{n}  {c}")
                    continue
                self._ocupado(False)
                self.aplicando = False
                if msg[0] == "fin_busqueda":
                    self._recargar()
                    csv_p, txt_p = rg.escribir_informes(self.db, self.total, self.regs, self.sesion)
                    aviso = msg[2] or "búsqueda terminada"
                    self.lbl_estado.config(text=aviso)
                    (messagebox.showwarning if msg[2] else messagebox.showinfo)(
                        "Búsqueda", f"{aviso}.\n\nLa DB no se ha modificado.\nInforme: {txt_p}\nCSV: {csv_p}")
                elif msg[0] == "fin_aplicar":
                    res = msg[1]
                    # resumen con los candidatos de antes de aplicar (los aplicados ya no lo son)
                    csv_p, txt_p = rg.escribir_informes(self.db, self.total, self.regs, self.sesion, res)
                    self.marcados -= set(res["actualizados"])
                    self._recargar()
                    omit = "".join(f"\n  id {i}: {m}" for i, m in list(res["omitidos"].items())[:10])
                    messagebox.showinfo("Aplicado", (
                        f"Actualizados: {len(res['actualizados'])}\nImágenes guardadas: {res['imagenes']}\n"
                        f"Imágenes fallidas: {res['imagenes_fallidas']}\nOmitidos: {len(res['omitidos'])}{omit}\n\n"
                        f"Backup: {res['backup']}\nInforme: {txt_p}"))
                elif msg[0] == "error":
                    self.lbl_estado.config(text="error")
                    messagebox.showerror("Error", f"{msg[1]}\n\nLa transacción se revirtió: la DB no se ha modificado.")
                return
        except queue.Empty:
            self.after(100, self._sondear)

    def _cerrar(self):
        if self.aplicando:  # el backup o la transacción quedarían a medias
            messagebox.showinfo("Salir", "Espera a que termine la aplicación de cambios.")
            return
        if self.trabajando and not messagebox.askyesno("Salir", "Hay un proceso en curso. ¿Cancelarlo y salir?"):
            return
        self.cancel.set()
        self.destroy()


if __name__ == "__main__":
    try:
        import PIL  # noqa: F401  — sin Pillow todas las portadas fallarían
    except ImportError:
        tk.Tk().withdraw()
        messagebox.showerror("Falta Pillow", "Instala Pillow para este Python:\n  python -m pip install -r requirements.txt")
        raise SystemExit(1)
    App().mainloop()
