import type { GameDetail, GameInput, Vocabulary } from "../api/types";

export interface FormValues {
  titulo: string;
  codigo: string;
  estado: string;
  idioma: string;
  tipo: string;
  pagina: string;
  puntuacion: number;
  ruta: string;
  descripcion: string;
  tags: string[];
  /** Text typed in the tag editor but not yet turned into a chip. */
  tagDraft: string;
}

export type FieldKey = "titulo" | "codigo" | "estado" | "idioma" | "tipo" | "pagina" | "puntuacion" | "ruta" | "descripcion" | "tags";
export type FormErrors = Partial<Record<FieldKey, string>>;

export const MAX_TAGS = 50;
const len = (s: string) => Array.from(s).length;

export function valuesFromGame(g: GameDetail): FormValues {
  return {
    titulo: g.titulo,
    codigo: g.codigo,
    estado: g.estado,
    idioma: g.idioma,
    tipo: g.tipo,
    pagina: g.pagina,
    puntuacion: g.puntuacion,
    ruta: g.ruta,
    descripcion: g.descripcion,
    tags: [...g.tags],
    tagDraft: "",
  };
}

export function defaultValues(v: Vocabulary | null): FormValues {
  const estados = v?.estados ?? [];
  const idiomas = v?.idiomas ?? [];
  const tipos = v?.tipos ?? [];
  return {
    titulo: "",
    codigo: "",
    estado: estados.includes("Sin empezar") ? "Sin empezar" : (estados[0] ?? "Sin empezar"),
    idioma: idiomas.includes("Japonés") ? "Japonés" : (idiomas[0] ?? ""),
    tipo: tipos.includes("RPG") ? "RPG" : "Otro",
    pagina: "",
    puntuacion: 0,
    ruta: "",
    descripcion: "",
    tags: [],
    tagDraft: "",
  };
}

/** Adds the comma-separated names in `raw` to `tags`, trimming and de-duplicating case-insensitively. */
export function addTags(tags: string[], raw: string): string[] {
  const out = [...tags];
  const seen = new Set(out.map((t) => t.toLowerCase()));
  for (const part of raw.split(",")) {
    const t = part.trim();
    if (!t || seen.has(t.toLowerCase())) continue;
    seen.add(t.toLowerCase());
    out.push(t);
  }
  return out;
}

/** Normalised payload for create_game / update_game (pending tag draft included). */
export function toInput(v: FormValues): GameInput {
  return {
    titulo: v.titulo.trim(),
    descripcion: v.descripcion,
    estado: v.estado.trim(),
    idioma: v.idioma.trim(),
    ruta: v.ruta.trim(),
    codigo: v.codigo.trim(),
    pagina: v.pagina.trim(),
    tipo: v.tipo.trim(),
    puntuacion: v.puntuacion,
    tags: addTags(v.tags, v.tagDraft),
  };
}

/** Mirrors the backend limits (docs/CONTRACTS_PHASE4.md). */
export function validate(v: FormValues): FormErrors {
  const i = toInput(v);
  const e: FormErrors = {};
  if (!i.titulo) e.titulo = "El título es obligatorio.";
  else if (len(i.titulo) > 300) e.titulo = "El título admite como máximo 300 caracteres.";
  if (len(i.descripcion) > 20000) e.descripcion = "La descripción admite como máximo 20000 caracteres.";
  if (!i.estado) e.estado = "El estado es obligatorio.";
  else if (len(i.estado) > 50) e.estado = "El estado admite como máximo 50 caracteres.";
  if (!i.idioma) e.idioma = "El idioma es obligatorio.";
  else if (len(i.idioma) > 50) e.idioma = "El idioma admite como máximo 50 caracteres.";
  if (len(i.ruta) > 1000) e.ruta = "La ruta admite como máximo 1000 caracteres.";
  if (len(i.codigo) > 100) e.codigo = "El código admite como máximo 100 caracteres.";
  if (len(i.pagina) > 100) e.pagina = "La página admite como máximo 100 caracteres.";
  if (len(i.tipo) > 50) e.tipo = "El tipo admite como máximo 50 caracteres.";
  if (!(i.puntuacion >= 0 && i.puntuacion <= 5 && Number.isInteger(i.puntuacion * 2)))
    e.puntuacion = "La puntuación debe estar entre 0 y 5 en pasos de 0,5.";
  if (i.tags.length > MAX_TAGS) e.tags = `Máximo ${MAX_TAGS} etiquetas.`;
  else if (i.tags.some((t) => len(t) > 100)) e.tags = "Cada etiqueta admite como máximo 100 caracteres.";
  return e;
}

/** Vocabulary list plus the record's current value when missing: unknown values are never dropped. */
export function withCurrent(list: string[], current: string): string[] {
  return current && !list.includes(current) ? [...list, current] : list;
}

export const SCORE_STEPS = Array.from({ length: 11 }, (_, i) => i / 2);
