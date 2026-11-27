#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Editor por tandas para completar descripciones en la biblioteca RPG Maker."""
import os, random, sqlite3, tkinter as tk
from tkinter import messagebox, ttk

LIMIT = 20
BASE_DIR = os.path.dirname(os.path.abspath(__file__))
DB_PATH = os.path.join(BASE_DIR, "juegos.db")
BACKUP_PATH = DB_PATH + ".backup-before-description-edit"


def connect(path):
    con = sqlite3.connect(path)
    con.execute("PRAGMA foreign_keys=ON")
    cols = {c[1] for c in con.execute("PRAGMA table_info(juegos)")}
    if not {"id", "titulo", "descripcion"} <= cols:
        con.close()
        raise RuntimeError("No se encontró una tabla juegos compatible.")
    return con


class App:
    def __init__(self, root):
        self.root=root; root.title("RPG Maker Library · Descripciones"); root.geometry("1000x680"); root.minsize(800,520)
        self.db=connect(DB_PATH); self.rows=[]; self.current=None; self.dirty=False
        self._ui(); self.batch(); root.protocol("WM_DELETE_WINDOW", self.close)

    def _ui(self):
        main=ttk.Frame(self.root,padding=12); main.pack(fill="both",expand=True)
        ttk.Label(main,text="Completar descripciones pendientes",font=("TkDefaultFont",16,"bold")).pack(anchor="w")
        ttk.Label(main,text="Tanda de hasta 20 registros. Guardar elimina el registro de la lista pendiente.").pack(anchor="w",pady=(4,10))
        pw=ttk.Panedwindow(main,orient="horizontal"); pw.pack(fill="both",expand=True)
        left=ttk.Frame(pw); right=ttk.Frame(pw,padding=(12,0,0,0)); pw.add(left,weight=1); pw.add(right,weight=3)
        self.lb=tk.Listbox(left,exportselection=False); self.lb.pack(side="left",fill="both",expand=True)
        sc=ttk.Scrollbar(left,orient="vertical",command=self.lb.yview); sc.pack(side="right",fill="y"); self.lb.configure(yscrollcommand=sc.set)
        self.lb.bind("<<ListboxSelect>>",self.select)
        self.heading=ttk.Label(right,text="Selecciona un juego",font=("TkDefaultFont",13,"bold")); self.heading.pack(anchor="w")
        self.meta=ttk.Label(right,text="",foreground="gray"); self.meta.pack(anchor="w",pady=(3,8))
        box=ttk.Frame(right); box.pack(fill="both",expand=True)
        self.txt=tk.Text(box,wrap="word",undo=True,font=("TkDefaultFont",11)); self.txt.pack(side="left",fill="both",expand=True)
        ts=ttk.Scrollbar(box,orient="vertical",command=self.txt.yview); ts.pack(side="right",fill="y"); self.txt.configure(yscrollcommand=ts.set)
        self.txt.bind("<<Modified>>",self.changed)
        buttons=ttk.Frame(main); buttons.pack(fill="x",pady=(10,0))
        self.status=ttk.Label(buttons,text=""); self.status.pack(side="left")
        for label,cmd in [("Omitir",self.skip),("Anterior",self.previous),("Guardar",self.save),("Guardar y siguiente",self.save_next),("Nueva tanda",self.batch)]:
            ttk.Button(buttons,text=label,command=cmd).pack(side="right",padx=3)

    def pending(self):
        return self.db.execute("SELECT id,titulo,COALESCE(codigo,''),COALESCE(descripcion,'') FROM juegos WHERE descripcion IS NULL OR TRIM(descripcion)=''").fetchall()

    def batch(self):
        if not self._confirm_leave(): return
        allrows=self.pending(); random.shuffle(allrows); self.rows=allrows[:LIMIT]; self.current=None; self.dirty=False
        self.lb.delete(0,"end")
        for r in self.rows: self.lb.insert("end",(r[2]+" · " if r[2] else "")+r[1])
        self.clear_editor()
        self.status.config(text=f"{len(self.rows)} en esta tanda · {len(allrows)} pendientes total")
        if not self.rows: messagebox.showinfo("Biblioteca", "No quedan descripciones pendientes.",parent=self.root)

    def clear_editor(self):
        self.txt.delete("1.0","end"); self.txt.edit_modified(False); self.heading.config(text="Selecciona un juego"); self.meta.config(text="")

    def changed(self,_=None):
        if self.txt.edit_modified():
            self.dirty=True
            self.txt.edit_modified(False)

    def _confirm_leave(self):
        if self.dirty and not messagebox.askyesno("Cambios sin guardar","Descartar los cambios actuales?",parent=self.root): return False
        self.dirty=False; return True

    def select(self,_=None):
        sel=self.lb.curselection()
        if not sel:return
        row=self.rows[sel[0]]
        if self.current is not None and self.current!=row[0] and not self._confirm_leave():
            self.lb.selection_set(next(i for i,r in enumerate(self.rows) if r[0]==self.current)); return
        self.current=row[0]; self.heading.config(text=row[1]); self.meta.config(text=f"Registro {row[0]} · Código {row[2] or '—'}")
        self.txt.delete("1.0","end"); self.txt.insert("1.0",row[3]); self.txt.edit_modified(False); self.dirty=False

    def backup(self):
        if not os.path.exists(BACKUP_PATH):
            with sqlite3.connect(BACKUP_PATH) as target: self.db.backup(target)

    def save(self):
        if self.current is None: messagebox.showinfo("Guardar","Selecciona un juego.",parent=self.root); return False
        val=self.txt.get("1.0","end").strip()
        if not val: messagebox.showwarning("Descripción","La descripción no puede quedar vacía.",parent=self.root); return False
        try:
            self.backup(); self.db.execute("UPDATE juegos SET descripcion=? WHERE id=?",(val,self.current)); self.db.commit()
        except Exception as e:
            self.db.rollback(); messagebox.showerror("Error",f"No se pudo guardar: {e}",parent=self.root); return False
        idx=self.lb.curselection()[0]; saved=self.current
        self.rows=[r for r in self.rows if r[0]!=saved]; self.lb.delete(idx); self.current=None; self.dirty=False; self.clear_editor()
        count=len(self.pending()); self.status.config(text=f"Guardado · {len(self.rows)} restantes en tanda · {count} pendientes total")
        if not self.rows: messagebox.showinfo("Tanda completada","Tanda terminada. Puedes cargar otra.",parent=self.root)
        return True

    def save_next(self):
        if self.save():
            if self.rows: self.lb.selection_set(0); self.lb.event_generate("<<ListboxSelect>>")
            else:self.batch()

    def skip(self):
        if not self.rows:return
        if not self._confirm_leave():return
        if self.current is not None:
            i=next((i for i,r in enumerate(self.rows) if r[0]==self.current),0)
        else:i=self.lb.curselection()[0] if self.lb.curselection() else 0
        self.current=None; self.dirty=False; self.lb.selection_clear(0,"end")
        if len(self.rows)>1:
            nxt=(i+1)%len(self.rows); self.lb.selection_set(nxt); self.lb.event_generate("<<ListboxSelect>>")
        else:self.clear_editor()

    def previous(self):
        if not self.rows:return
        if not self._confirm_leave():return
        i=next((i for i,r in enumerate(self.rows) if r[0]==self.current),0)
        nxt=(i-1)%len(self.rows); self.lb.selection_set(nxt); self.lb.event_generate("<<ListboxSelect>>")

    def close(self):
        if not self._confirm_leave():return
        self.db.close(); self.root.destroy()


if __name__=="__main__":
    root=tk.Tk()
    try: app=App(root)
    except Exception as e: messagebox.showerror("Inicio",str(e)); root.destroy()
    else: root.mainloop()
