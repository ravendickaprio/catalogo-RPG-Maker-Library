//! `settings.json` en el directorio de configuración de la app (ADR-012).
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub db_path: Option<String>,
    pub read_only: bool,
}

impl Settings {
    /// Missing or unreadable/corrupt file -> defaults (a bad settings file must not block the app).
    pub fn load(path: &Path) -> Settings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Creates the parent directory (only on save) and writes atomically via a temp file.
    pub fn save(&self, path: &Path) -> Result<(), AppError> {
        let io = |e: std::io::Error| {
            AppError::Config(format!("No se pudieron guardar los ajustes: {e}"))
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| AppError::Config(format!("Ajustes inválidos: {e}")))?;
        std::fs::write(&tmp, json).map_err(io)?;
        std::fs::rename(&tmp, path).map_err(io)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_defaults_and_corrupt() {
        let d = tempfile::TempDir::new().unwrap();
        let p = d.path().join("cfg").join("settings.json");
        assert_eq!(Settings::load(&p), Settings::default());
        assert!(!p.parent().unwrap().exists(), "load must not create dirs");
        let s = Settings {
            db_path: Some("C:/x/juegos.db".into()),
            read_only: true,
        };
        s.save(&p).unwrap();
        assert_eq!(Settings::load(&p), s);
        let raw: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(raw["dbPath"], "C:/x/juegos.db");
        assert_eq!(raw["readOnly"], true);
        std::fs::write(&p, "{ not json").unwrap();
        assert_eq!(Settings::load(&p), Settings::default());
    }
}
