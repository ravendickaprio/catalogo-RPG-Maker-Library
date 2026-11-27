use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SortKey {
    TitleAsc,
    TitleDesc,
    Code,
    Status,
    Language,
    Recent,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub estado: Option<String>,
    #[serde(default)]
    pub idioma: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
    pub sort: SortKey,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSummary {
    pub id: i64,
    pub titulo: String,
    pub codigo: String,
    pub estado: String,
    pub idioma: String,
    pub tipo: String,
    pub pagina: String,
    pub puntuacion: f64,
    pub has_cover: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub items: Vec<GameSummary>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
    pub elapsed_ms: f64,
    pub strategy: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetail {
    #[serde(flatten)]
    pub summary: GameSummary,
    pub descripcion: String,
    pub ruta: String,
    pub tags: Vec<String>,
    /// Huella FNV-1a 64 (hex) del registro; detecta modificaciones externas (Fase 4).
    pub revision: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cover {
    pub mime: String,
    pub data_base64: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CountItem {
    pub value: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInfo {
    pub db_file_name: String,
    pub read_only: bool,
    pub total: i64,
    pub fts_available: bool,
    pub fts_in_sync: bool,
    pub estados: Vec<CountItem>,
    pub idiomas: Vec<CountItem>,
    pub tags: Vec<CountItem>,
    pub sqlite_version: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SuggestRequest {
    pub exclude_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub game: Option<GameDetail>,
    pub eligible_count: i64,
    pub total_count: i64,
}

/// Datos editables de un juego (Fase 4). `imagen` no forma parte (portadas: Fase 5).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInput {
    pub titulo: String,
    pub descripcion: String,
    pub estado: String,
    pub idioma: String,
    pub ruta: String,
    pub codigo: String,
    pub pagina: String,
    pub tipo: String,
    pub puntuacion: f64,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WriteOptions {
    pub allow_duplicate_code: bool,
    pub expected_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vocabulary {
    pub estados: Vec<String>,
    pub idiomas: Vec<String>,
    pub paginas: Vec<String>,
    pub tipos: Vec<String>,
    pub puntuaciones: Vec<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub db_file_name: String,
    pub db_dir: String,
    pub read_only: bool,
    pub last_backup: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupResult {
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingMigration {
    pub version: i64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrationStatus {
    pub current: i64,
    pub latest: i64,
    pub pending: Vec<PendingMigration>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationRun {
    pub applied: Vec<i64>,
    /// Vacío si no había migraciones pendientes (no se hace backup).
    pub backup_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_request_defaults_and_sort() {
        let r: SearchRequest =
            serde_json::from_str(r#"{"query":"a","sort":"title_desc","offset":0,"limit":10}"#)
                .unwrap();
        assert_eq!(r.sort, SortKey::TitleDesc);
        assert!(r.estado.is_none() && r.idioma.is_none() && r.tag.is_none());
    }

    #[test]
    fn suggest_request_empty_object() {
        let r: SuggestRequest = serde_json::from_str("{}").unwrap();
        assert!(r.exclude_id.is_none());
        let r: SuggestRequest = serde_json::from_str(r#"{"excludeId":5}"#).unwrap();
        assert_eq!(r.exclude_id, Some(5));
    }

    #[test]
    fn detail_flattens_summary_camel_case() {
        let d = GameDetail {
            summary: GameSummary {
                id: 1,
                titulo: "t".into(),
                codigo: "".into(),
                estado: "".into(),
                idioma: "".into(),
                tipo: "".into(),
                pagina: "".into(),
                puntuacion: 0.0,
                has_cover: true,
            },
            descripcion: "".into(),
            ruta: "".into(),
            tags: vec![],
            revision: "0".into(),
        };
        let v = serde_json::to_value(d).unwrap();
        assert_eq!(v["hasCover"], true);
        assert_eq!(v["id"], 1);
        assert!(v.get("tags").is_some());
    }

    #[test]
    fn write_options_defaults_and_input_shape() {
        let o: WriteOptions = serde_json::from_str("{}").unwrap();
        assert!(!o.allow_duplicate_code && o.expected_revision.is_none());
        let o: WriteOptions =
            serde_json::from_str(r#"{"allowDuplicateCode":true,"expectedRevision":"ab"}"#).unwrap();
        assert!(o.allow_duplicate_code);
        assert_eq!(o.expected_revision.as_deref(), Some("ab"));
        let g: GameInput = serde_json::from_str(
            r#"{"titulo":"t","descripcion":"","estado":"e","idioma":"i","ruta":"","codigo":"","pagina":"","tipo":"","puntuacion":1.5}"#,
        )
        .unwrap();
        assert!(g.tags.is_empty());
    }
}
