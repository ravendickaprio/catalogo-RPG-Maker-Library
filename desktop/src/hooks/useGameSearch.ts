import { useCallback, useEffect, useRef, useState } from "react";
import { searchGames } from "../api/client";
import type { AppError, GameSummary, SearchPage, SortKey } from "../api/types";
import { normalizeError } from "../api/client";
import { RequestSequencer } from "../lib/requestSequencer";
import { useDebouncedValue } from "./useDebouncedValue";

export const PAGE_SIZE = 100;

export interface SearchParams {
  query: string;
  estado: string; // "" = todos
  idioma: string; // "" = todos
  sort: SortKey;
}

export interface SearchState {
  items: GameSummary[];
  total: number;
  loading: boolean;
  error: AppError | null;
  elapsedMs: number | null;
  strategy: SearchPage["strategy"] | null;
  loadMore: () => void;
}

export function useGameSearch({ query, estado, idioma, sort }: SearchParams): SearchState {
  const debouncedQuery = useDebouncedValue(query.trim(), 150);
  const [items, setItems] = useState<GameSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<AppError | null>(null);
  const [elapsedMs, setElapsedMs] = useState<number | null>(null);
  const [strategy, setStrategy] = useState<SearchPage["strategy"] | null>(null);

  const sequencer = useRef(new RequestSequencer());
  const itemsLen = useRef(0);
  const totalRef = useRef(0);
  const loadingMore = useRef(false);
  const paramsRef = useRef({ debouncedQuery, estado, idioma, sort });
  paramsRef.current = { debouncedQuery, estado, idioma, sort };

  useEffect(() => {
    const token = sequencer.current.next(); // new filter set: invalidates everything older
    loadingMore.current = false;
    setLoading(true);
    setError(null);
    searchGames({
      query: debouncedQuery,
      estado: estado || null,
      idioma: idioma || null,
      sort,
      offset: 0,
      limit: PAGE_SIZE,
    }).then(
      (page) => {
        if (!sequencer.current.isLatest(token)) return;
        itemsLen.current = page.items.length;
        totalRef.current = page.total;
        setItems(page.items);
        setTotal(page.total);
        setElapsedMs(page.elapsedMs);
        setStrategy(page.strategy);
        setLoading(false);
      },
      (e) => {
        if (!sequencer.current.isLatest(token)) return;
        setError(normalizeError(e));
        setLoading(false);
      },
    );
  }, [debouncedQuery, estado, idioma, sort]);

  const loadMore = useCallback(() => {
    if (loadingMore.current || itemsLen.current >= totalRef.current) return;
    const token = sequencer.current.current(); // same filter set; dropped if filters change
    const p = paramsRef.current;
    loadingMore.current = true;
    searchGames({
      query: p.debouncedQuery,
      estado: p.estado || null,
      idioma: p.idioma || null,
      sort: p.sort,
      offset: itemsLen.current,
      limit: PAGE_SIZE,
    }).then(
      (page) => {
        if (!sequencer.current.isLatest(token)) return;
        loadingMore.current = false;
        itemsLen.current += page.items.length;
        totalRef.current = page.total;
        setItems((prev) => prev.concat(page.items));
        setTotal(page.total);
        setElapsedMs(page.elapsedMs);
        setStrategy(page.strategy);
      },
      (e) => {
        if (!sequencer.current.isLatest(token)) return;
        loadingMore.current = false;
        setError(normalizeError(e));
      },
    );
  }, []);

  return { items, total, loading, error, elapsedMs, strategy, loadMore };
}
