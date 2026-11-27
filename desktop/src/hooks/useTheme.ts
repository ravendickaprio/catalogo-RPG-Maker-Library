import { useCallback, useState } from "react";
import { applyTheme, loadTheme, saveTheme, type Theme } from "../lib/theme";

export function useTheme(): [Theme, (t: Theme) => void] {
  const [theme, setThemeState] = useState<Theme>(() => loadTheme());
  const setTheme = useCallback((t: Theme) => {
    setThemeState(t);
    saveTheme(t);
    applyTheme(t);
  }, []);
  return [theme, setTheme];
}
