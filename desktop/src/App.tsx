import { useCallback, useEffect, useRef, useState } from "react";
import { getLibraryInfo, normalizeError } from "./api/client";
import type { AppError, LibraryInfo, SortKey } from "./api/types";
import GameDetail from "./components/GameDetail";
import GameList from "./components/GameList";
import SearchBar from "./components/SearchBar";
import SuggestionDialog from "./components/SuggestionDialog";
import { useGameSearch } from "./hooks/useGameSearch";

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
  const [focusTick, setFocusTick] = useState(0);
  const detailRef = useRef<HTMLElement>(null);

  useEffect(() => {
    getLibraryInfo().then(setInfo, (e) => setInfoError(normalizeError(e)));
  }, []);

  const search = useGameSearch({ query, estado, idioma, tag, sort });
  const searchError = search.error;

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

  return (
    <div className="app">
      <header className="app-header">
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
      </header>
      {infoError && (
        <div className="banner error" role="alert">
          {infoError.kind === "Config" ? "Error de configuración: " : "Error: "}
          {infoError.message}
        </div>
      )}
      <SearchBar
        info={info}
        query={query}
        estado={estado}
        idioma={idioma}
        tag={tag}
        sort={sort}
        onQuery={setQuery}
        onEstado={setEstado}
        onIdioma={setIdioma}
        onTag={setTag}
        onClear={clearFilters}
        onSort={setSort}
        onSuggest={() => setSuggestOpen(true)}
      />
      {searchError && !infoError && (
        <div className="banner error" role="alert">
          Error de búsqueda: {searchError.message}
        </div>
      )}
      <main className="main">
        <GameList
          items={search.items}
          total={search.total}
          loading={search.loading}
          elapsedMs={search.elapsedMs}
          strategy={search.strategy}
          selectedId={selectedId}
          onSelect={setSelectedId}
          onOpen={(id) => {
            setSelectedId(id);
            detailRef.current?.focus();
          }}
          loadMore={search.loadMore}
          focusTick={focusTick}
        />
        <GameDetail ref={detailRef} id={selectedId} />
      </main>
      {suggestOpen && (
        <SuggestionDialog
          onClose={() => setSuggestOpen(false)}
          onShowInList={locateGame}
        />
      )}
    </div>
  );
}
