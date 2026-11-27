#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Editor independiente por tandas, con la apariencia y campos del catálogo v4."""
import io, os, random, sqlite3, tkinter as tk
from tkinter import ttk, filedialog, messagebox
from PIL import Image, ImageTk

BASE=os.path.dirname(os.path.abspath(__file__)); DB=os.path.join(BASE,"juegos.db")
BACKUP=DB+".backup-before-description-edit"
ESTADOS=["Sin empezar","Empezado","Terminado"]; IDIOMAS=["Español","Inglés","Japonés"]
PAGINAS=["Elonichan","Estahi0","lewdzone","M0RETranslations","Otome","ryuugames","Shorts","Youtube"]
TIPOS=["Visual Novel","RPG","Renpy","3d","Otro"]; PUNTOS=[f"{x/2:.1f}" for x in range(11)]
TEMAS={
"Claro":dict(fondo="#eceef1",superficie="#e1e4e9",campo="#ffffff",borde="#c3c9d2",texto="#2b303a",tenue="#6b7280",acento="#3b6ea5",seleccion="#c9dcf2",hover="#d3d8df",ok="#3f8f2a",warn="#b26a00",neutro="#6b7785"),
"Oscuro":dict(fondo="#1e2127",superficie="#2a2f37",campo="#262a31",borde="#3b414c",texto="#c8cfda",tenue="#8791a1",acento="#6f9bc7",seleccion="#34445a",hover="#343a44",ok="#7aa86a",warn="#cf9f5a",neutro="#76808f"),
"Noche":dict(fondo="#1c1a18",superficie="#2a2623",campo="#242120",borde="#3d3630",texto="#d4c8b4",tenue="#8f8372",acento="#c49a6c",seleccion="#4a3d30",hover="#342f2a",ok="#8aa66b",warn="#c9954f",neutro="#7d7366")}
class App:
 def __init__(self,root):
  self.root=root;root.title("RPG Maker Library · Editor de registros");root.geometry("1240x850");root.minsize(1000,700)
  self.db=sqlite3.connect(DB);self.db.execute("PRAGMA foreign_keys=ON")
  self.cols={r[1] for r in self.db.execute("PRAGMA table_info(juegos)")}
  required={"id","titulo","descripcion","estado","idioma","ruta","codigo","imagen","pagina","tipo","puntuacion"}
  if not required<=self.cols: raise RuntimeError("La base de datos no tiene todos los campos compatibles con el catálogo v4.")
  self.theme=tk.StringVar(value="Oscuro");self.p=TEMAS["Oscuro"];self.rows=[];self.current=None;self.dirty=False;self.imgbytes=None;self.imgtk=None
  self.build();self.apply_theme();self.batch();root.protocol("WM_DELETE_WINDOW",self.close)
 def build(self):
  self.main=ttk.Frame(self.root,padding=10);self.main.pack(fill="both",expand=True);self.main.columnconfigure(0,weight=1);self.main.columnconfigure(1,weight=3);self.main.rowconfigure(1,weight=1)
  top=ttk.Frame(self.main);top.grid(row=0,column=0,columnspan=2,sticky="ew",pady=(0,8))
  ttk.Label(top,text="Editor de biblioteca",font=("TkDefaultFont",16,"bold")).pack(side="left")
  ttk.Label(top,text="Tema").pack(side="right",padx=4);cb=ttk.Combobox(top,textvariable=self.theme,values=list(TEMAS),state="readonly",width=10);cb.pack(side="right");cb.bind("<<ComboboxSelected>>",lambda e:self.apply_theme())
  left=ttk.LabelFrame(self.main,text="Registros por revisar · máximo 20",padding=8);left.grid(row=1,column=0,sticky="nsew",padx=(0,10));left.rowconfigure(0,weight=1);left.columnconfigure(0,weight=1)
  self.lb=tk.Listbox(left,exportselection=False,width=32);self.lb.grid(row=0,column=0,sticky="nsew");sc=ttk.Scrollbar(left,command=self.lb.yview);sc.grid(row=0,column=1,sticky="ns");self.lb.configure(yscrollcommand=sc.set);self.lb.bind("<<ListboxSelect>>",self.select)
  ttk.Button(left,text="Nueva tanda",command=self.batch).grid(row=1,column=0,columnspan=2,sticky="ew",pady=(8,0))
  right=ttk.LabelFrame(self.main,text="Ficha del juego",padding=10);right.grid(row=1,column=1,sticky="nsew");right.columnconfigure(0,weight=1);right.rowconfigure(2,weight=1)
  self.code=tk.StringVar();self.title=tk.StringVar();row=ttk.Frame(right);row.grid(row=0,column=0,sticky="ew",pady=(0,6));row.columnconfigure(1,weight=1)
  ttk.Label(row,text="Código").grid(row=0,column=0,sticky="w");ttk.Label(row,text="Título").grid(row=0,column=1,sticky="w",padx=(8,0))
  ttk.Entry(row,textvariable=self.code,width=16).grid(row=1,column=0,sticky="w");ttk.Entry(row,textvariable=self.title).grid(row=1,column=1,sticky="ew",padx=(8,0))
  ttk.Label(right,text="Descripción").grid(row=1,column=0,sticky="w")
  descbox=ttk.Frame(right);descbox.grid(row=2,column=0,sticky="nsew",pady=(0,8));descbox.rowconfigure(0,weight=1);descbox.columnconfigure(0,weight=1)
  self.desc=tk.Text(descbox,wrap="word",undo=True);self.desc.grid(row=0,column=0,sticky="nsew");ds=ttk.Scrollbar(descbox,command=self.desc.yview);ds.grid(row=0,column=1,sticky="ns");self.desc.configure(yscrollcommand=ds.set);self.desc.bind("<<Modified>>",self.mark_dirty)
  self.state=tk.StringVar(value=ESTADOS[0]);self.lang=tk.StringVar(value=IDIOMAS[0]);self.page=tk.StringVar();self.typ=tk.StringVar(value=TIPOS[-1]);self.rating=tk.StringVar(value="0.0");self.tags=tk.StringVar();self.path=tk.StringVar()
  fields=ttk.Frame(right);fields.grid(row=3,column=0,sticky="ew",pady=5);fields.columnconfigure((0,1),weight=1)
  self.combo(fields,"Estado",self.state,ESTADOS,0,0);self.combo(fields,"Idioma",self.lang,IDIOMAS,0,1);self.combo(fields,"Página",self.page,PAGINAS,1,0);self.combo(fields,"Tipo",self.typ,TIPOS,1,1);self.combo(fields,"Puntuación ★",self.rating,PUNTOS,2,0)
  ttk.Label(fields,text="Tags (separados por coma)").grid(row=2,column=1,sticky="w");ttk.Entry(fields,textvariable=self.tags).grid(row=3,column=1,sticky="ew")
  pathf=ttk.Frame(right);pathf.grid(row=4,column=0,sticky="ew",pady=5);pathf.columnconfigure(0,weight=1);ttk.Label(pathf,text="Ruta").grid(row=0,column=0,sticky="w")
  ttk.Entry(pathf,textvariable=self.path).grid(row=1,column=0,sticky="ew");ttk.Button(pathf,text="Elegir",command=self.pick_path).grid(row=1,column=1,padx=4);ttk.Button(pathf,text="Abrir",command=self.open_path).grid(row=1,column=2)
  imgf=ttk.Frame(right);imgf.grid(row=5,column=0,sticky="ew",pady=6);self.imglabel=tk.Label(imgf,text="Sin portada",width=22,height=7);self.imglabel.pack(side="left");btns=ttk.Frame(imgf);btns.pack(side="left",padx=8);ttk.Button(btns,text="Elegir imagen",command=self.pick_image).pack(fill="x");ttk.Button(btns,text="Quitar imagen",command=self.remove_image).pack(fill="x",pady=3)
  actions=ttk.Frame(right);actions.grid(row=6,column=0,sticky="ew",pady=(8,0))
  for label,fn in [("Omitir",self.skip),("Anterior",self.previous),("Guardar",self.save),("Guardar y siguiente",self.save_next),("Nueva tanda",self.batch)]:
   ttk.Button(actions,text=label,command=fn).pack(side="left",padx=3)
  self.status=ttk.Label(self.main,text="");self.status.grid(row=2,column=0,columnspan=2,sticky="ew",pady=(8,0))
 def combo(self,parent,label,var,vals,r,c):
  ttk.Label(parent,text=label).grid(row=r*2,column=c,sticky="w",padx=(4,4));ttk.Combobox(parent,textvariable=var,values=vals,state="readonly").grid(row=r*2+1,column=c,sticky="ew",padx=4,pady=(0,5))
 def apply_theme(self):
  self.p=TEMAS[self.theme.get()];p=self.p;s=ttk.Style(self.root);s.theme_use("clam")
  self.root.configure(bg=p["fondo"]);s.configure(".",background=p["fondo"],foreground=p["texto"],fieldbackground=p["campo"],bordercolor=p["borde"],troughcolor=p["superficie"],focuscolor=p["acento"]);s.configure("TFrame",background=p["fondo"]);s.configure("TLabel",background=p["fondo"],foreground=p["texto"]);s.configure("TLabelframe",background=p["fondo"],foreground=p["texto"],bordercolor=p["borde"]);s.configure("TLabelframe.Label",background=p["fondo"],foreground=p["tenue"]);s.configure("TButton",background=p["superficie"],foreground=p["texto"],bordercolor=p["borde"],padding=(8,4));s.map("TButton",background=[("active",p["hover"]),("pressed",p["seleccion"])]);s.configure("TEntry",fieldbackground=p["campo"],foreground=p["texto"]);s.configure("TCombobox",fieldbackground=p["campo"],background=p["superficie"],foreground=p["texto"]);s.configure("TScrollbar",background=p["borde"],troughcolor=p["fondo"]);self.lb.configure(bg=p["campo"],fg=p["texto"],selectbackground=p["seleccion"],selectforeground=p["texto"]);self.desc.configure(bg=p["campo"],fg=p["texto"],insertbackground=p["texto"],selectbackground=p["seleccion"],highlightbackground=p["borde"]);self.imglabel.configure(bg=p["campo"],fg=p["tenue"])
 def pending(self):return self.db.execute("SELECT id,titulo,COALESCE(codigo,''),COALESCE(descripcion,'') FROM juegos WHERE descripcion IS NULL OR TRIM(descripcion)=''").fetchall()
 def batch(self):
  if not self.confirm_leave():return
  allr=self.pending();random.shuffle(allr);self.rows=allr[:20];self.current=None;self.lb.delete(0,"end")
  for r in self.rows:self.lb.insert("end",(r[2]+" · " if r[2] else "")+r[1])
  self.clear();self.status.config(text=f"{len(self.rows)} en tanda · {len(allr)} pendientes total")
  if not self.rows:messagebox.showinfo("Biblioteca","No quedan registros sin descripción.")
 def clear(self):
  self.current=None;self.code.set("");self.title.set("");self.desc.delete("1.0","end");self.desc.edit_modified(False);self.state.set(ESTADOS[0]);self.lang.set(IDIOMAS[0]);self.page.set("");self.typ.set(TIPOS[-1]);self.rating.set("0.0");self.tags.set("");self.path.set("");self.imgbytes=None;self.show_image();self.dirty=False
 def mark_dirty(self,e=None):
  if self.desc.edit_modified():self.dirty=True;self.desc.edit_modified(False)
 def confirm_leave(self):
  if self.dirty and not messagebox.askyesno("Cambios sin guardar","¿Descartar los cambios actuales?"):return False
  self.dirty=False;return True
 def select(self,e=None):
  sel=self.lb.curselection()
  if not sel:return
  r=self.rows[sel[0]]
  if self.current!=r[0] and not self.confirm_leave():
   if self.current is not None:self.lb.selection_set(next(i for i,x in enumerate(self.rows) if x[0]==self.current))
   return
  self.load(r[0])
 def load(self,jid):
  r=self.db.execute("SELECT id,titulo,descripcion,estado,idioma,ruta,codigo,imagen,pagina,tipo,puntuacion FROM juegos WHERE id=?",(jid,)).fetchone()
  if not r:return
  jid,t,d,st,lang,path,code,img,page,typ,score=r;self.current=jid;self.title.set(t or "");self.code.set(code or "");self.desc.delete("1.0","end");self.desc.insert("1.0",d or "");self.desc.edit_modified(False);self.state.set(st if st in ESTADOS else ESTADOS[0]);self.lang.set(lang if lang in IDIOMAS else IDIOMAS[0]);self.path.set(path or "");self.page.set(page or "");self.typ.set(typ if typ in TIPOS else TIPOS[-1]);self.rating.set(str(float(score or 0)));self.tags.set(", ".join(x[0] for x in self.db.execute("SELECT t.nombre FROM tags t JOIN juego_tags jt ON jt.tag_id=t.id WHERE jt.juego_id=? ORDER BY t.nombre",(jid,))));self.imgbytes=img;self.show_image();self.dirty=False
 def backup(self):
  if not os.path.exists(BACKUP):
   with sqlite3.connect(BACKUP) as target:self.db.backup(target)
 def save(self):
  if self.current is None:messagebox.showinfo("Guardar","Selecciona un registro.");return False
  title=self.title.get().strip();desc=self.desc.get("1.0","end").strip()
  if not title:messagebox.showwarning("Título","El título no puede quedar vacío.");return False
  if not desc:messagebox.showwarning("Descripción","La descripción es necesaria para completar esta tanda.");return False
  try:
   self.backup();jid=self.current;vals=(title,desc,self.state.get(),self.lang.get(),self.path.get().strip(),self.code.get().strip(),self.imgbytes,self.page.get(),self.typ.get(),float(self.rating.get()))
   self.db.execute("UPDATE juegos SET titulo=?,descripcion=?,estado=?,idioma=?,ruta=?,codigo=?,imagen=?,pagina=?,tipo=?,puntuacion=? WHERE id=?",(*vals,jid))
   self.db.execute("DELETE FROM juego_tags WHERE juego_id=?",(jid,))
   for tag in dict.fromkeys(x.strip() for x in self.tags.get().split(",") if x.strip()):
    self.db.execute("INSERT OR IGNORE INTO tags(nombre) VALUES(?)",(tag,));tid=self.db.execute("SELECT id FROM tags WHERE nombre=? COLLATE NOCASE",(tag,)).fetchone()[0];self.db.execute("INSERT OR IGNORE INTO juego_tags(juego_id,tag_id) VALUES(?,?)",(jid,tid))
   self.db.commit()
  except Exception as ex:self.db.rollback();messagebox.showerror("Guardar",f"No se pudo guardar: {ex}");return False
  i=self.lb.curselection()[0] if self.lb.curselection() else 0;self.rows=[r for r in self.rows if r[0]!=jid];self.lb.delete(i);self.clear();self.status.config(text=f"Registro guardado · {len(self.rows)} restantes · {len(self.pending())} pendientes total")
  return True
 def save_next(self):
  if self.save():
   if self.rows:self.lb.selection_set(0);self.lb.event_generate("<<ListboxSelect>>")
   else:self.batch()
 def skip(self):
  if not self.rows or not self.confirm_leave():return
  i=next((i for i,r in enumerate(self.rows) if r[0]==self.current),0);n=(i+1)%len(self.rows);self.current=None;self.lb.selection_set(n);self.lb.event_generate("<<ListboxSelect>>")
 def previous(self):
  if not self.rows or not self.confirm_leave():return
  i=next((i for i,r in enumerate(self.rows) if r[0]==self.current),0);self.lb.selection_set((i-1)%len(self.rows));self.lb.event_generate("<<ListboxSelect>>")
 def pick_path(self):
  path=filedialog.askdirectory(title="Carpeta del juego")
  if path:self.path.set(path)
 def open_path(self):
  path=self.path.get().strip()
  if not path or not os.path.exists(path):messagebox.showwarning("Ruta","La carpeta no existe o no está definida.");return
  import subprocess,sys
  try:
   if sys.platform.startswith("win"):os.startfile(path)
   elif sys.platform=="darwin":subprocess.Popen(["open",path])
   else:subprocess.Popen(["xdg-open",path])
  except Exception as ex:messagebox.showerror("Ruta",str(ex))
 def pick_image(self):
  path=filedialog.askopenfilename(filetypes=[("Imágenes","*.png *.jpg *.jpeg *.webp *.bmp *.gif")])
  if not path:return
  try:
   im=Image.open(path);im.thumbnail((300,300));buf=io.BytesIO()
   if im.mode not in ("RGB","RGBA"):im=im.convert("RGB")
   im.save(buf,"WEBP",quality=65);self.imgbytes=buf.getvalue();self.show_image()
  except Exception as ex:messagebox.showerror("Imagen",f"No se pudo procesar: {ex}")
 def remove_image(self):self.imgbytes=None;self.show_image()
 def show_image(self):
  self.imgtk=None
  if self.imgbytes:
   try:im=Image.open(io.BytesIO(self.imgbytes));im.thumbnail((180,140));self.imgtk=ImageTk.PhotoImage(im);self.imglabel.configure(image=self.imgtk,text="");return
   except Exception:pass
  self.imglabel.configure(image="",text="Sin portada")
 def close(self):
  if not self.confirm_leave():return
  self.db.close();self.root.destroy()
if __name__=="__main__":
 root=tk.Tk()
 try:app=App(root)
 except Exception as e:messagebox.showerror("Inicio",str(e));root.destroy()
 else:root.mainloop()
