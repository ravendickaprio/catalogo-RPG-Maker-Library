import { useEffect, useRef, useState } from "react";
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
  const [sort, setSort] = useState<SortKey>("title_asc");
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [suggestOpen, setSuggestOpen] = useState(false);
  const detailRef = useRef<HTMLElement>(null);

  useEffect(() => {
    getLibraryInfo().then(setInfo, (e) => setInfoError(normalizeError(e)));
  }, []);

  const search = useGameSearch({ query, estado, idioma, sort });
  const searchError = search.error;

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
        sort={sort}
        onQuery={setQuery}
        onEstado={setEstado}
        onIdioma={setIdioma}
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
        />
        <GameDetail ref={detailRef} id={selectedId} />
      </main>
      {suggestOpen && (
        <SuggestionDialog
          onClose={() => setSuggestOpen(false)}
          onShowInList={(id) => {
            setSelectedId(id);
            setSuggestOpen(false);
          }}
        />
      )}
    </div>
  );
}
