import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getLibraryInfo, normalizeError } from "./api/client";
import type { AppError, LibraryInfo, SortKey } from "./api/types";
import CommandPalette from "./components/CommandPalette";
import GameDetail from "./components/GameDetail";
import GameGallery from "./components/GameGallery";
import GameList from "./components/GameList";
import SearchBar from "./components/SearchBar";
import Sidebar from "./components/Sidebar";
import SuggestionDialog from "./components/SuggestionDialog";
import { useExternalRefresh } from "./hooks/useExternalRefresh";
import { useGameSearch } from "./hooks/useGameSearch";
import { useTheme } from "./hooks/useTheme";
import { clearCoverCache } from "./lib/coverCache";
import type { PaletteAction } from "./lib/palette";
import { PREF_SIDEBAR, PREF_VIEW, writePref } from "./lib/storage";
import { THEMES, type Theme } from "./lib/theme";
import { loadSidebarOpen, loadView, type ViewMode } from "./lib/view";

export default function App() {
  const [info, setInfo] = useState<LibraryInfo | null>(null);
  const [infoError, setInfoError] = useState<AppError | null>(null);
  const [query, setQuery] = useState("");
  const [estado, setEstado] = useState("");
  const [idioma, setIdioma] = useState("");
  const [tag, setTag] = useState("");
  const [sort, setSort] = useState<SortKey>("title_asc");
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [suggestOpen, setSuggestOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [focusTick, setFocusTick] = useState(0);
  const [view, setViewState] = useState<ViewMode>(() => loadView());
  const [sidebarOpen, setSidebarOpen] = useState<boolean>(() => loadSidebarOpen());
  const [theme, setTheme] = useTheme();
  const [toast, setToast] = useState<string | null>(null);
  const [refreshTick, setRefreshTick] = useState(0);
  const detailRef = useRef<HTMLElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const loadInfo = useCallback(() => {
    getLibraryInfo().then(
      (i) => {
        setInfo(i);
        setInfoError(null);
      },
      (e) => setInfoError(normalizeError(e)),
    );
  }, []);
  useEffect(loadInfo, [loadInfo]);

  const search = useGameSearch({ query, estado, idioma, tag, sort });
  const searchError = search.error;
  const reloadSearch = search.reload;

  const setView = useCallback((v: ViewMode) => {
    setViewState(v);
    writePref(PREF_VIEW, v);
  }, []);
  const toggleSidebar = useCallback(() => {
    setSidebarOpen((o) => {
      writePref(PREF_SIDEBAR, o ? "collapsed" : "open");
      return !o;
    });
  }, []);

  // ADR-010: refresh when another process modified the DB.
  useExternalRefresh(
    useCallback(() => {
      clearCoverCache();
      loadInfo();
      reloadSearch();
      setRefreshTick((n) => n + 1);
      setToast("Biblioteca actualizada");
    }, [loadInfo, reloadSearch]),
  );
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 3500);
    return () => clearTimeout(t);
  }, [toast]);

  const clearFilters = useCallback(() => {
    setQuery("");
    setEstado("");
    setIdioma("");
    setTag("");
  }, []);

  /** Shows a game in the list: clears filters, searches by code (or title) and selects it. */
  const locateGame = useCallback((game: { id: number; codigo: string; titulo: string }) => {
    setEstado("");
    setIdioma("");
    setTag("");
    setQuery(game.codigo.trim() ? game.codigo : game.titulo);
    setSelectedId(game.id);
    setSuggestOpen(false);
    setFocusTick((n) => n + 1);
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (!mod || e.altKey) return;
      const k = e.key.toLowerCase();
      if (k === "k" || (k === "p" && e.shiftKey)) {
        e.preventDefault();
        setPaletteOpen(true);
      } else if (k === "1" || k === "2") {
        e.preventDefault();
        setView(k === "1" ? "list" : "gallery");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setView]);

  const actions = useMemo<PaletteAction[]>(() => {
    const list: PaletteAction[] = [
      { id: "search", label: "Buscar…", run: () => setTimeout(() => searchRef.current?.focus(), 0) },
      { id: "suggest", label: "Juego sugerido del día", run: () => setSuggestOpen(true) },
      { id: "view-list", label: "Vista lista", run: () => setView("list") },
      { id: "view-gallery", label: "Vista galería", run: () => setView("gallery") },
      ...THEMES.map((t) => ({ id: `theme-${t.value}`, label: `Tema: ${t.label}`, run: () => setTheme(t.value) })),
      { id: "clear", label: "Limpiar filtros", run: clearFilters },
    ];
    for (const s of info?.estados ?? []) {
      if (s.value) list.push({ id: `estado-${s.value}`, label: `Filtrar por estado ${s.value}`, run: () => setEstado(s.value) });
    }
    return list;
  }, [info, setView, setTheme, clearFilters]);

  const listProps = {
    items: search.items,
    total: search.total,
    loading: search.loading,
    elapsedMs: search.elapsedMs,
    strategy: search.strategy,
    selectedId,
    onSelect: setSelectedId,
    onOpen: (id: number) => {
      setSelectedId(id);
      detailRef.current?.focus();
    },
    loadMore: search.loadMore,
    focusTick,
  };

  return (
    <div className="app">
      <header className="app-header">
        <button
          type="button"
          className="icon-btn"
          aria-label="Filtros"
          aria-expanded={sidebarOpen}
          aria-controls="sidebar-nav"
          title="Mostrar u ocultar los filtros"
          onClick={toggleSidebar}
        >
          ☰ Filtros
        </button>
        <h1>RPG Maker Library</h1>
        {info && (
          <>
            <span className="db-name" title="Base de datos">
              {info.dbFileName}
            </span>
            {info.readOnly && <span className="badge badge-ro">solo lectura</span>}
            <span className="cell-muted">{info.total} juegos</span>
          </>
        )}
        {info && !info.ftsInSync && (
          <span className="warn" role="alert">
            ⚠ El índice de búsqueda no está sincronizado; los resultados podrían ser incompletos.
          </span>
        )}
        <span className="spacer" />
        <div className="view-toggle" role="group" aria-label="Vista">
          <button type="button" aria-pressed={view === "list"} onClick={() => setView("list")} title="Vista lista (Ctrl+1)">
            Lista
          </button>
          <button type="button" aria-pressed={view === "gallery"} onClick={() => setView("gallery")} title="Vista galería (Ctrl+2)">
            Galería
          </button>
        </div>
        <label className="theme-select">
          <span className="sr-only">Tema</span>
          <select aria-label="Tema" value={theme} onChange={(e) => setTheme(e.target.value as Theme)}>
            {THEMES.map((t) => (
              <option key={t.value} value={t.value}>
                Tema: {t.label}
              </option>
            ))}
          </select>
        </label>
        <button type="button" onClick={() => setPaletteOpen(true)} title="Paleta de comandos (Ctrl+K)">
          Comandos
        </button>
      </header>
      {infoError && (
        <div className="banner error" role="alert">
          {infoError.kind === "Config" ? "Error de configuración: " : "Error: "}
          {infoError.message}
        </div>
      )}
      <SearchBar
        ref={searchRef}
        query={query}
        estado={estado}
        idioma={idioma}
        tag={tag}
        sort={sort}
        onQuery={setQuery}
        onClear={clearFilters}
        onSort={setSort}
        onSuggest={() => setSuggestOpen(true)}
      />
      {searchError && !infoError && (
        <div className="banner error" role="alert">
          Error de búsqueda: {searchError.message}{" "}
          <button type="button" onClick={reloadSearch}>
            Reintentar
          </button>
        </div>
      )}
      <div className="body">
        <div id="sidebar-nav" className="sidebar-wrap">
          <Sidebar
            info={info}
            estado={estado}
            idioma={idioma}
            tag={tag}
            open={sidebarOpen}
            onEstado={setEstado}
            onIdioma={setIdioma}
            onTag={setTag}
            onAll={() => {
              setEstado("");
              setIdioma("");
              setTag("");
            }}
            onClose={toggleSidebar}
          />
        </div>
        <main className="main">
          {view === "list" ? <GameList {...listProps} /> : <GameGallery {...listProps} />}
          <GameDetail ref={detailRef} id={selectedId} reloadToken={refreshTick} onMissing={() => setSelectedId(null)} />
        </main>
      </div>
      {suggestOpen && <SuggestionDialog onClose={() => setSuggestOpen(false)} onShowInList={locateGame} />}
      {paletteOpen && <CommandPalette actions={actions} onClose={() => setPaletteOpen(false)} />}
      {toast && (
        <div className="toast" role="status" aria-live="polite">
          {toast}
        </div>
      )}
    </div>
  );
}
