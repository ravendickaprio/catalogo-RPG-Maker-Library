import { useSyncExternalStore } from "react";

/** Max cover upload accepted by the client (the backend enforces the same 20 MiB limit). */
export const MAX_COVER_BYTES = 20 * 1024 * 1024;

/** `http://cover.localhost/<id>` on Windows (WebView2), `cover://localhost/<id>` elsewhere. */
export function coverUrl(id: number, tag: string | number = 0, userAgent: string = navigator.userAgent): string {
  const base = userAgent.includes("Windows") ? "http://cover.localhost" : "cover://localhost";
  return `${base}/${id}?r=${encodeURIComponent(String(tag))}`;
}

/** Client-side pre-check; the backend revalidates by magic bytes. An empty MIME type is let through. */
export function validateCoverFile(f: { type: string; size: number }): string | null {
  if (f.type && !f.type.startsWith("image/")) return "El archivo no es una imagen.";
  if (f.size > MAX_COVER_BYTES) return "La imagen supera el límite de 20 MB.";
  if (f.size === 0) return "El archivo está vacío.";
  return null;
}

export async function readFileBytes(f: Blob): Promise<Uint8Array> {
  if (typeof f.arrayBuffer === "function") return new Uint8Array(await f.arrayBuffer());
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(new Uint8Array(r.result as ArrayBuffer));
    r.onerror = () => reject(r.error);
    r.readAsArrayBuffer(f);
  });
}

// ---- cache-busting tags: the webview caches covers as immutable, so the URL must change after a write.
let epoch = 0;
const bumps = new Map<number, number>();
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((l) => l());

/** Forgets one id after its cover was changed/removed (new URL tag). */
export const invalidateCover = (id: number) => {
  bumps.set(id, (bumps.get(id) ?? 0) + 1);
  emit();
};
/** New tag for every id (external refresh, DB swap, migration). */
export const clearCoverCache = () => {
  epoch++;
  bumps.clear();
  emit();
};
export const coverTag = (id: number) => `${epoch}.${bumps.get(id) ?? 0}`;

const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => listeners.delete(l);
};
/** Re-renders when the cover of `id` (or everything) is invalidated; returns the current tag. */
export function useCoverTag(id: number): string {
  return useSyncExternalStore(subscribe, () => coverTag(id));
}
