import { useEffect, useState } from "react";
import { coverCache } from "../lib/coverCache";

interface State {
  id: number | null;
  src: string | null;
  loading: boolean;
}

/** Lazily loads the cover for a single id through the shared LRU cache; returns a data URL or null. */
export function useCover(id: number | null, enabled: boolean): { src: string | null; loading: boolean } {
  const [state, setState] = useState<State>({ id: null, src: null, loading: false });
  useEffect(() => {
    if (id == null || !enabled) {
      setState({ id, src: null, loading: false });
      return;
    }
    const hit = coverCache.peek(id);
    if (hit !== undefined) {
      setState({ id, src: hit, loading: false });
      return;
    }
    let cancelled = false; // unmounted or scrolled away: ignore the result (cache still keeps it)
    setState({ id, src: null, loading: true });
    coverCache.load(id).then(
      (src) => {
        if (!cancelled) setState({ id, src, loading: false });
      },
      () => {
        if (!cancelled) setState({ id, src: null, loading: false });
      },
    );
    return () => {
      cancelled = true;
    };
  }, [id, enabled]);
  if (state.id === id) return { src: state.src, loading: state.loading };
  if (id != null && enabled) {
    const hit = coverCache.peek(id);
    if (hit !== undefined) return { src: hit, loading: false };
  }
  return { src: null, loading: enabled && id != null };
}
