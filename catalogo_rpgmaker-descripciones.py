#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Herramienta independiente para completar descripciones pendientes en juegos.db."""
import os
import random
import sqlite3
import shutil
import tkinter as tk
from tkinter import messagebox, ttk

LIMIT = 20
BASE_DIR = os.path.dirname(os.path.abspath(__file__))
DB_PATH = os.path.join(BASE_DIR, "juegos.db")


def open_db(path):
    con = sqlite3.connect(path)
    con.execute("PRAGMA foreign_keys=ON")
    cols = {r[1] for r in con.execute("PRAGMA table_info(juegos)")}
    if not {"id", "titulo", "descripcion"} <= cols:
        con.close()
        raise RuntimeError("La base de datos no contiene la tabla juegos esperada.")
    return con


class DescriptionBatchApp:
    def __init__(self, root):
        self.root = root
        root.title("RPG Maker Library - Completar descripciones")
        root.geometry("980x650")
        root.minsize(760, 480)
        self.con = open_db(DB_PATH)
        self.items = []
        self.current = None
        self.backup_done = False
        self._ui()
        self.load_batch()
        root.protocol("WM_DELETE_WINDOW", self.close)

    def _ui(self):
        outer = ttk.Frame(self.root, padding=12)
        outer.pack(fill="both", expand=True)
        ttk.Label(outer, text="Completar descripciones pendientes",
                  font=("TkDefaultFont", 16, "bold")).pack(anchor="w")
        ttk.Label(outer, text="Se muestran hasta 20 juegos sin descripción. Guarda cada cambio para retirarlo de esta tanda.").pack(anchor="w", pady=(3, 10))
        pane = ttk.Panedwindow(outer, orient="horizontal")
        pane.pack(fill="both", expand=True)
        left = ttk.Frame(pane, padding=(0, 0, 8, 0))
        right = ttk.Frame(pane, padding=(8, 0, 0, 0))
        pane.add(left, weight=1); pane.add(right, weight=3)
        self.listbox = tk.Listbox(left, exportselection=False, activestyle="dotbox")
        sb = ttk.Scrollbar(left, orient="vertical", command=self.listbox.yview)
        self.listbox.configure(yscrollcommand=sb.set)
        self.listbox.pack(side="left", fill="both", expand=True); sb.pack(side="right", fill="y")
        self.listbox.bind("<<ListboxSelect>>", self.select_item)
        self.lbl_title = ttk.Label(right, text="Selecciona un juego", font=("TkDefaultFont", 13, "bold"))
        self.lbl_title.pack(anchor="w", pady=(0, 5))
        self.lbl_meta = ttk.Label(right, text="", foreground="gray")
        self.lbl_meta.pack(anchor="w", pady=(0, 8))
        self.desc = tk.Text(right, wrap="word", undo=True, font=("TkDefaultFont", 11))
        self.desc.pack(fill="both", expand=True)
        dscroll = ttk.Scrollbar(right, orient="vertical", command=self.desc.yview)
        self.desc.configure(yscrollcommand=dscroll.set)
        # Scrollbar alongside text
        self.desc.pack(side="left", fill="both", expand=True)
        dscroll.pack(side="right", fill="y")
        controls = ttk.Frame(outer)
        controls.pack(fill="x", pady=(10, 0))
        self.progress = ttk.Label(controls, text="")
        self.progress.pack(side="left")
        ttk.Button(controls, text="Guardar descripción", command=self.save_current).pack(side="right", padx=4)
        ttk.Button(controls, text="Siguiente tanda", command=self.load_batch).pack(side="right", padx=4)
        ttk.Button(controls, text="Cerrar", command=self.close).pack(side="right", padx=4)

    def load_batch(self):
        if self.current is not None and self.has_unsaved():
            if not messagebox.askyesno("Cambios pendientes", "Hay cambios sin guardar. ¿Descartarlos y cargar otra tanda?", parent=self.root):
                return
        rows = self.con.execute("""
            SELECT id, titulo, COALESCE(codigo,''), COALESCE(descripcion,'')
            FROM juegos WHERE descripcion IS NULL OR TRIM(descripcion)=''
        """).fetchall()
        random.shuffle(rows)
        self.items = rows[:LIMIT]
        self.current = None
        self.listbox.delete(0, "end")
        for jid, title, code, _ in self.items:
            self.listbox.insert("end", (f"{code} · " if code else "") + title)
        self.desc.delete("1.0", "end")
        self.lbl_title.config(text="Selecciona un juego")
        self.lbl_meta.config(text="")
        self.progress.config(text=f"{len(self.items)} juego(s) en esta tanda · pendientes en DB: {len(rows)}")
        if not self.items:
            messagebox.showinfo("¡Listo!", "No hay juegos con descripciones vacías.", parent=self.root)

    def select_item(self, _event=None):
        selection = self.listbox.curselection()
        if not selection: return
        idx = selection[0]
        row = self.items[idx]
        if self.current is not None and self.current != row[0] and self.has_unsaved():
            if not messagebox.askyesno("Cambios pendientes", "¿Descartar los cambios no guardados?", parent=self.root):
                self.listbox.selection_set(self.items.index(next(x for x in self.items if x[0] == self.current)))
                return
        self.current = row[0]
        self.lbl_title.config(text=row[1])
        self.lbl_meta.config(text=f"ID: {row[0]} · Código: {row[2] or '—'}")
        self.desc.delete("1.0", "end")
        self.desc.insert("1.0", row[3] or "")

    def has_unsaved(self):
        if self.current is None: return False
        original = next((r[3] for r in self.items if r[0] == self.current), "")
        return self.desc.get("1.0", "end").strip() != (original or "").strip()

    def backup(self):
        if self.backup_done: return
        backup_path = DB_PATH + ".backup-before-description-edit"
        if not os.path.exists(backup_path):
            with sqlite3.connect(backup_path) as dest:
                self.con.backup(dest)
        self.backup_done = True

    def save_current(self):
        if self.current is None:
            messagebox.showinfo("Guardar", "Selecciona un juego primero.", parent=self.root)
            return
        text = self.desc.get("1.0", "end").strip()
        if not text:
            messagebox.showwarning("Descripción vacía", "Escribe una descripción antes de guardar.", parent=self.root)
            return
        try:
            self.backup()
            self.con.execute("UPDATE juegos SET descripcion=? WHERE id=?", (text, self.current))
            self.con.commit()
        except Exception as e:
            self.con.rollback()
            messagebox.showerror("Error", f"No se pudo guardar:\n{e}", parent=self.root)
            return
        saved_id = self.current
        self.items = [r for r in self.items if r[0] != saved_id]
        self.listbox.delete(self.listbox.curselection()[0])
        self.current = None
        self.desc.delete("1.0", "end")
        self.lbl_title.config(text="Descripción guardada")
        self.lbl_meta.config(text="")
        remaining = self.con.execute("SELECT COUNT(*) FROM juegos WHERE descripcion IS NULL OR TRIM(descripcion)=''").fetchone()[0]
        self.progress.config(text=f"Guardado · {len(self.items)} restante(s) en esta tanda · pendientes en DB: {remaining}")
        if not self.items:
            messagebox.showinfo("Tanda completada", "¡Tanda completada! Puedes cargar la siguiente.", parent=self.root)

    def close(self):
        self.con.close()
        self.root.destroy()


if __name__ == "__main__":
    root = tk.Tk()
    try:
        app = DescriptionBatchApp(root)
    except Exception as exc:
        messagebox.showerror("No se pudo iniciar", str(exc))
        root.destroy()
    else:
        root.mainloop()
