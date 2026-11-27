import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getLibraryInfo, getSessionInfo, getVocabulary, normalizeError } from "./api/client";
import type { AppError, GameDetail as Detail, LibraryInfo, SessionInfo, SortKey, Vocabulary } from "./api/types";
import CommandPalette from "./components/CommandPalette";
import ConfirmDialog from "./components/ConfirmDialog";
import SettingsDialog from "./components/SettingsDialog";
import ImportDialog from "./components/ImportDialog";
import MaintenanceDialog from "./components/MaintenanceDialog";
import DashboardDialog, { type DashboardFilter } from "./components/DashboardDialog";
import ReviewDialog, { type ReviewCriterion } from "./components/ReviewDialog";
import ShortcutsDialog from "./components/ShortcutsDialog";
import ExportDialog from "./components/ExportDialog";
import FirstRunScreen from "./components/FirstRunScreen";
import ToolsMenu from "./components/ToolsMenu";
import WelcomeScreen, { welcomeOnStartup } from "./components/WelcomeScreen";
import logo from "./assets/logo.webp";
import GameDetail, { type DetailCommand } from "./components/GameDetail";
import GameGallery from "./components/GameGallery";
import GameList from "./components/GameList";
import SearchBar from "./components/SearchBar";
import Sidebar from "./components/Sidebar";
import SuggestionDialog from "./components/SuggestionDialog";
import { useExternalRefresh } from "./hooks/useExternalRefresh";
import { useGameSearch } from "./hooks/useGameSearch";
import { useTheme } from "./hooks/useTheme";
import { clearCoverCache, invalidateCover } from "./lib/cover";
import { findFavoriteTag } from "./lib/favorites";
import type { PaletteAction } from "./lib/palette";
import { isModalOpen, matchShortcut } from "./lib/shortcuts";
import { PREF_SIDEBAR, PREF_VIEW, writePref } from "./lib/storage";
import { THEMES, type Theme } from "./lib/theme";
import { loadSidebarOpen, loadView, type ViewMode } from "./lib/view";

export default function App() {
  const [info, setInfo] = useState<LibraryInfo | null>(null);
  const [infoError, setInfoError] = useState<AppError | null>(null);
  const [sessionConfigError, setSessionConfigError] = useState<string | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
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
  const [session, setSession] = useState<SessionInfo | null>(null);
  const [vocab, setVocab] = useState<Vocabulary | null>(null);
  const [creating, setCreating] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [maintOpen, setMaintOpen] = useState(false);
  const [dashOpen, setDashOpen] = useState(false);
  const [reviewOpen, setReviewOpen] = useState(false);
  const [reviewCriterion, setReviewCriterion] = useState<ReviewCriterion>("sin_descripcion");
  const [helpOpen, setHelpOpen] = useState(false);
  const [welcomeOpen, setWelcomeOpen] = useState(welcomeOnStartup);
  const [command, setCommand] = useState<DetailCommand | null>(null);
  const [pendingAction, setPendingAction] = useState<(() => void) | null>(null);
  const dirtyRef = useRef(false);
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
  const loadSession = useCallback(() => {
    getSessionInfo().then(
      (s) => {
        setSession(s);
        setSessionConfigError(null);
      },
      (e) => {
        const err = normalizeError(e);
        setSessionConfigError(err.kind === "Config" ? err.message : null);
      },
    );
    getVocabulary().then(setVocab, () => undefined);
  }, []);
  useEffect(loadInfo, [loadInfo]);
  useEffect(loadSession, [loadSession]);
  const readOnly = session?.readOnly ?? info?.readOnly ?? true;

  /** Runs `action` now, or after the user confirms discarding unsaved edits. */
  const guard = useCallback((action: () => void) => {
    if (dirtyRef.current) setPendingAction(() => action);
    else action();
  }, []);
  const onDirtyChange = useCallback((d: boolean) => {
    dirtyRef.current = d;
  }, []);

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
      loadSession();
      reloadSearch();
      setRefreshTick((n) => n + 1);
      setToast("Biblioteca actualizada por otra aplicación");
    }, [loadInfo, loadSession, reloadSearch]),
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
    if (dirtyRef.current) {
      setSuggestOpen(false);
      setPendingAction(() => () => {
        dirtyRef.current = false;
        setCreating(false);
        setEstado("");
        setIdioma("");
        setTag("");
        setQuery(game.codigo.trim() ? game.codigo : game.titulo);
        setSelectedId(game.id);
        setFocusTick((n) => n + 1);
      });
      return;
    }
    setCreating(false);
    setEstado("");
    setIdioma("");
    setTag("");
    setQuery(game.codigo.trim() ? game.codigo : game.titulo);
    setSelectedId(game.id);
    setSuggestOpen(false);
    setFocusTick((n) => n + 1);
  }, []);

  const selectGame = useCallback(
    (id: number | null) => {
      if (id === selectedId && !creating) return;
      guard(() => {
        dirtyRef.current = false;
        setCreating(false);
        setSelectedId(id);
      });
    },
    [guard, selectedId, creating],
  );

  const startCreate = useCallback(() => {
    if (readOnly) {
      setToast("Modo solo lectura: no se puede crear juegos");
      return;
    }
    if (creating) return;
    guard(() => {
      dirtyRef.current = false;
      setSelectedId(null);
      setCreating(true);
    });
  }, [readOnly, creating, guard]);

  /** Everything that depends on stored data is refreshed after a write. */
  const afterWrite = useCallback(
    (id?: number) => {
      if (id != null) invalidateCover(id);
      loadInfo();
      getVocabulary().then(setVocab, () => undefined);
      reloadSearch();
    },
    [loadInfo, reloadSearch],
  );
  const onSaved = useCallback(
    (g: Detail, created: boolean) => {
      afterWrite(g.id);
      if (created) {
        setCreating(false);
        setSelectedId(g.id);
      }
      setToast(created ? "Juego creado" : "Cambios guardados");
    },
    [afterWrite],
  );
  const onDeleted = useCallback(
    (id: number) => {
      afterWrite(id);
      setSelectedId(null);
      setToast("Juego eliminado");
    },
    [afterWrite],
  );
  const onRestored = useCallback(() => {
    clearCoverCache();
    dirtyRef.current = false;
    setCreating(false);
    setSelectedId(null);
    loadInfo();
    loadSession();
    reloadSearch();
    setRefreshTick((n) => n + 1);
    setToast("Copia restaurada");
  }, [loadInfo, loadSession, reloadSearch]);
  const onDbChanged = useCallback(
    (i: LibraryInfo) => {
      clearCoverCache();
      dirtyRef.current = false;
      setInfo(i);
      setInfoError(null);
      setSessionConfigError(null);
      setCreating(false);
      setSelectedId(null);
      loadSession();
      reloadSearch();
      setRefreshTick((n) => n + 1);
    },
    [loadSession, reloadSearch],
  );
  const sendCommand = (kind: "edit" | "delete" | "favorite") => {
    if (selectedId == null) {
      setToast("Selecciona un juego primero");
      return;
    }
    setCommand((c) => ({ kind, n: (c?.n ?? 0) + 1 }));
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (isModalOpen()) return; // dialogs own their keys (form Ctrl+S/Esc, review Ctrl+Enter)
      switch (matchShortcut(e)?.id) {
        case "palette":
          e.preventDefault();
          setPaletteOpen(true);
          break;
        case "new-game":
          e.preventDefault();
          startCreate();
          break;
        case "view-list":
          e.preventDefault();
          setView("list");
          break;
        case "view-gallery":
          e.preventDefault();
          setView("gallery");
          break;
        case "help":
          e.preventDefault();
          setHelpOpen(true);
          break;
        // "focus-search" is handled by SearchBar; form/review/list shortcuts by their components.
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setView, startCreate]);

  const showFavorites = () => {
    const fav = findFavoriteTag(info?.tags);
    if (!fav) setToast("Aún no hay favoritos");
    else setTag(fav);
  };
  const applyDashboardFilter = (f: DashboardFilter) => {
    if (f.kind === "estado") setEstado(f.value);
    else if (f.kind === "idioma") setIdioma(f.value);
    else setTag(f.value);
    setDashOpen(false);
  };

  const actions = useMemo<PaletteAction[]>(() => {
    const list: PaletteAction[] = [
      { id: "search", label: "Buscar…", run: () => setTimeout(() => searchRef.current?.focus(), 0) },
      { id: "suggest", label: "Juego sugerido del día", run: () => setSuggestOpen(true) },
      { id: "new", label: "Nuevo juego", run: startCreate },
      { id: "edit", label: "Editar juego seleccionado", run: () => sendCommand("edit") },
      { id: "delete", label: "Eliminar juego seleccionado", run: () => sendCommand("delete") },
      { id: "import", label: "Importar juegos…", run: () => setImportOpen(true) },
      { id: "export", label: "Exportar biblioteca…", run: () => setExportOpen(true) },
      { id: "maintenance", label: "Centro de mantenimiento", run: () => setMaintOpen(true) },
      { id: "dashboard", label: "Estadísticas", run: () => setDashOpen(true) },
      { id: "review", label: "Revisión por tandas", run: () => {
          setReviewCriterion("sin_descripcion");
          setReviewOpen(true);
        } },
      { id: "favorite", label: "Alternar favorito del juego seleccionado", run: () => sendCommand("favorite") },
      { id: "show-favorites", label: "Mostrar favoritos", run: showFavorites },
      { id: "shortcuts", label: "Atajos de teclado", run: () => setHelpOpen(true) },
      { id: "settings", label: "Ajustes", run: () => setSettingsOpen(true) },
      { id: "welcome", label: "Pantalla de bienvenida", run: () => setWelcomeOpen(true) },
      { id: "view-list", label: "Vista lista", run: () => setView("list") },
      { id: "view-gallery", label: "Vista galería", run: () => setView("gallery") },
      ...THEMES.map((t) => ({ id: `theme-${t.value}`, label: `Tema: ${t.label}`, run: () => setTheme(t.value) })),
      { id: "clear", label: "Limpiar filtros", run: clearFilters },
    ];
    for (const s of info?.estados ?? []) {
      if (s.value) list.push({ id: `estado-${s.value}`, label: `Filtrar por estado ${s.value}`, run: () => setEstado(s.value) });
    }
    return list;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [info, setView, setTheme, clearFilters, startCreate, selectedId]);

  const listProps = {
    items: search.items,
    total: search.total,
    loading: search.loading,
    elapsedMs: search.elapsedMs,
    strategy: search.strategy,
    selectedId,
    onSelect: selectGame,
    onOpen: (id: number) => {
      selectGame(id);
      detailRef.current?.focus();
    },
    loadMore: search.loadMore,
    focusTick,
  };

  const firstRun = !info && (infoError?.kind === "Config" || sessionConfigError != null);
  if (firstRun) {
    return (
      <div className="app">
        <FirstRunScreen message={infoError?.kind === "Config" ? infoError.message : sessionConfigError} onOpened={onDbChanged} />
      </div>
    );
  }

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
        <h1>
          <img className="app-logo" src={logo} alt="" width={28} height={28} />
          RPG Maker Library
        </h1>
        {info && (
          <>
            <span className="db-name" title={`Base de datos: ${info.dbFileName}`}>
              {info.dbFileName}
            </span>
            <span className={"badge " + (readOnly ? "badge-ro" : "badge-edit")}>{readOnly ? "solo lectura" : "edición"}</span>
            <span className="cell-muted header-count" title={`${info.total} juegos`}>{info.total} juegos</span>
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
        <button type="button" className="primary" onClick={startCreate} title={readOnly ? "Modo solo lectura" : "Nuevo juego (Ctrl+N)"} disabled={readOnly}>
          Nuevo juego
        </button>
        <ToolsMenu
          items={[
            { id: "import", label: "Importar…", title: "Importar juegos desde una carpeta", run: () => setImportOpen(true) },
            { id: "export", label: "Exportar biblioteca…", title: "Guardar los datos en CSV o JSON", run: () => setExportOpen(true) },
            { id: "maintenance", label: "Mantenimiento", title: "Detectar y corregir problemas de la biblioteca", run: () => setMaintOpen(true) },
            { id: "dashboard", label: "Estadísticas", title: "Resumen y cobertura de datos de la biblioteca", run: () => setDashOpen(true) },
            { id: "review", label: "Revisión por tandas", title: "Revisar tandas aleatorias de juegos", run: () => {
          setReviewCriterion("sin_descripcion");
          setReviewOpen(true);
        } },
          ]}
        />
        <button type="button" onClick={() => setSettingsOpen(true)} title="Ajustes de la base de datos">
          Ajustes
        </button>
        <button type="button" className="icon-btn" aria-label="Comandos" title="Paleta de comandos (Ctrl+K)" onClick={() => setPaletteOpen(true)}>
          ⌕
        </button>
        <button type="button" className="icon-btn" aria-label="Atajos de teclado" title="Atajos de teclado (?)" onClick={() => setHelpOpen(true)}>
          ?
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
          <GameDetail
            ref={detailRef}
            id={selectedId}
            creating={creating}
            readOnly={readOnly}
            vocab={vocab}
            tagSuggestions={(info?.tags ?? []).map((t) => t.value)}
            reloadToken={refreshTick}
            command={command}
            onMissing={() => {
              setSelectedId(null);
              reloadSearch();
            }}
            onDirtyChange={onDirtyChange}
            onSaved={onSaved}
            onCoverChanged={(g) => {
              invalidateCover(g.id);
              afterWrite(g.id);
              setToast(g.hasCover ? "Portada guardada" : "Portada quitada");
            }}
            onDeleted={onDeleted}
            onFavoriteChanged={(g, fav) => {
              afterWrite(g.id);
              setToast(fav ? "Añadido a favoritos" : "Quitado de favoritos");
            }}
            onCancelCreate={() => setCreating(false)}
            onToast={setToast}
          />
        </main>
      </div>
      {suggestOpen && <SuggestionDialog onClose={() => setSuggestOpen(false)} onShowInList={locateGame} />}
      {importOpen && (
        <ImportDialog
          vocab={vocab}
          readOnly={readOnly}
          onClose={() => setImportOpen(false)}
          onChanged={() => {
            afterWrite();
            setToast("Cambios aplicados");
          }}
        />
      )}
      {maintOpen && (
        <MaintenanceDialog
          vocab={vocab}
          readOnly={readOnly}
          onClose={() => setMaintOpen(false)}
          onChanged={() => {
            afterWrite();
            setToast("Cambios aplicados");
          }}
          onOpenGame={(g) => {
            setMaintOpen(false);
            locateGame(g);
          }}
          onOpenReview={(c) => {
            setMaintOpen(false);
            setReviewCriterion(c);
            setReviewOpen(true);
          }}
        />
      )}
      {dashOpen && <DashboardDialog onClose={() => setDashOpen(false)} onFilter={applyDashboardFilter} />}
      {reviewOpen && (
        <ReviewDialog
          initialCriterion={reviewCriterion}
          vocab={vocab}
          readOnly={readOnly}
          tagSuggestions={(info?.tags ?? []).map((t) => t.value)}
          onClose={() => setReviewOpen(false)}
          onChanged={(id) => {
            afterWrite(id);
            setRefreshTick((n) => n + 1);
          }}
        />
      )}
      {helpOpen && <ShortcutsDialog onClose={() => setHelpOpen(false)} />}
      {settingsOpen && (
        <SettingsDialog
          session={session}
          onClose={() => setSettingsOpen(false)}
          onDbChanged={onDbChanged}
          onSessionChanged={(s) => {
            setSession(s);
            loadInfo();
          }}
          onRestored={onRestored}
          onMigrated={() => {
            clearCoverCache();
            afterWrite();
          }}
        />
      )}
      {exportOpen && <ExportDialog onClose={() => setExportOpen(false)} onDone={setToast} />}
      {pendingAction && (
        <ConfirmDialog
          title="¿Descartar los cambios?"
          confirmLabel="Descartar"
          danger
          onCancel={() => setPendingAction(null)}
          onConfirm={() => {
            const a = pendingAction;
            setPendingAction(null);
            a();
          }}
        >
          <p>Tienes cambios sin guardar en el formulario. Si continúas, se perderán.</p>
        </ConfirmDialog>
      )}
      {welcomeOpen && <WelcomeScreen total={info?.total ?? null} error={infoError?.message ?? null} onClose={() => setWelcomeOpen(false)} />}
      {paletteOpen && <CommandPalette actions={actions} onClose={() => setPaletteOpen(false)} />}
      {toast && (
        <div className="toast" role="status" aria-live="polite">
          {toast}
        </div>
      )}
    </div>
  );
}
