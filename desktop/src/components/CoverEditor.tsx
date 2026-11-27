import { useEffect, useRef, useState, type DragEvent } from "react";
import { normalizeError, removeCover, setCover } from "../api/client";
import type { AppError, GameDetail } from "../api/types";
import { readFileBytes, validateCoverFile } from "../lib/cover";
import ConfirmDialog from "./ConfirmDialog";
import CoverImage from "./CoverImage";

interface Props {
  game: GameDetail;
  onChanged: (g: GameDetail) => void;
  onError: (e: AppError) => void;
}

export function firstImageFile(files: FileList | File[] | null | undefined): File | null {
  if (!files) return null;
  return Array.from(files).find((f) => f.type.startsWith("image/")) ?? null;
}

/** Cover section of the edit form. Cover changes are saved immediately and independently of the text. */
export default function CoverEditor({ game, onChanged, onError }: Props) {
  const fileRef = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [over, setOver] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);

  // A file dropped outside the zone must not navigate the webview to it.
  useEffect(() => {
    const stop = (e: Event) => e.preventDefault();
    window.addEventListener("dragover", stop);
    window.addEventListener("drop", stop);
    return () => {
      window.removeEventListener("dragover", stop);
      window.removeEventListener("drop", stop);
    };
  }, []);

  const fail = (e: unknown) => {
    const err = normalizeError(e);
    if (err.kind === "InvalidInput") setMessage(err.message);
    else onError(err);
  };

  const upload = async (file: File) => {
    if (busy) return;
    setMessage(null);
    const bad = validateCoverFile(file);
    if (bad) {
      setMessage(bad);
      return;
    }
    setBusy(true);
    try {
      const bytes = await readFileBytes(file);
      onChanged(await setCover(game.id, bytes, game.revision));
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  // Ctrl+V with an image in the clipboard while the edit form is open (text pastes are untouched).
  const uploadRef = useRef(upload);
  uploadRef.current = upload;
  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      const f = firstImageFile(e.clipboardData?.files);
      if (!f) return;
      e.preventDefault();
      void uploadRef.current(f);
    };
    document.addEventListener("paste", onPaste);
    return () => document.removeEventListener("paste", onPaste);
  }, []);

  const remove = async () => {
    setConfirmRemove(false);
    setBusy(true);
    setMessage(null);
    try {
      onChanged(await removeCover(game.id, game.revision));
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    setOver(false);
    const f = e.dataTransfer?.files?.[0];
    if (f) void upload(f);
  };

  return (
    <fieldset className="cover-editor">
      <legend>Portada</legend>
      <div
        className={"cover-drop" + (over ? " over" : "")}
        tabIndex={0}
        role="group"
        aria-label="Zona de portada: arrastra una imagen o pega con Ctrl+V"
        aria-busy={busy}
        onDragOver={(e) => {
          e.preventDefault();
          setOver(true);
        }}
        onDragLeave={() => setOver(false)}
        onDrop={onDrop}
      >
        <CoverImage id={game.id} hasCover={game.hasCover} alt={`Portada de ${game.titulo}`} />
        <p className="note">{busy ? "Procesando imagen…" : "Arrastra una imagen aquí o pégala con Ctrl+V."}</p>
      </div>
      <div className="dialog-actions">
        <input ref={fileRef} type="file" accept="image/*" hidden data-testid="cover-file" onChange={(e) => {
          const f = e.target.files?.[0];
          e.target.value = "";
          if (f) void upload(f);
        }} />
        <button type="button" disabled={busy} onClick={() => fileRef.current?.click()}>
          Cambiar portada…
        </button>
        <button type="button" className="danger" disabled={busy || !game.hasCover} onClick={() => setConfirmRemove(true)}>
          Quitar portada
        </button>
      </div>
      <p className="note">La portada se guarda al instante (no depende del botón Guardar).</p>
      {message && (
        <p className="field-error" role="alert">
          {message}
        </p>
      )}
      {confirmRemove && (
        <ConfirmDialog title="¿Quitar la portada?" confirmLabel="Quitar" danger onCancel={() => setConfirmRemove(false)} onConfirm={() => void remove()}>
          <p>Se elimina la imagen de este juego en la base de datos (hay un backup automático de la sesión).</p>
        </ConfirmDialog>
      )}
    </fieldset>
  );
}
