import { useEffect, useState } from "react";
import { getCover } from "../api/client";

/** Lazily loads the cover for a single id; returns a data URL or null. */
export function useCover(id: number | null, enabled: boolean): { src: string | null; loading: boolean } {
  const [state, setState] = useState<{ id: number | null; src: string | null; loading: boolean }>({
    id: null,
    src: null,
    loading: false,
  });
  useEffect(() => {
    if (id == null || !enabled) {
      setState({ id, src: null, loading: false });
      return;
    }
    let cancelled = false;
    setState({ id, src: null, loading: true });
    getCover(id).then(
      (c) => {
        if (!cancelled) setState({ id, src: c ? `data:${c.mime};base64,${c.dataBase64}` : null, loading: false });
      },
      () => {
        if (!cancelled) setState({ id, src: null, loading: false });
      },
    );
    return () => {
      cancelled = true;
    };
  }, [id, enabled]);
  return state.id === id ? { src: state.src, loading: state.loading } : { src: null, loading: enabled && id != null };
}
