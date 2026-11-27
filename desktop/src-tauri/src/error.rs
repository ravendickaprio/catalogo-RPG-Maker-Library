use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

#[derive(Debug, Clone, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    InvalidInput(String),
    #[error("{0}")]
    Database(String),
    #[error("{0}")]
    Config(String),
    /// La revisión del registro cambió desde que se leyó (otra app lo modificó).
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    DuplicateCode(String),
    #[error("{0}")]
    ReadOnly(String),
    /// SQLITE_BUSY / SQLITE_LOCKED: otra aplicación mantiene la base bloqueada.
    #[error("{0}")]
    Busy(String),
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::NotFound(_) => "NotFound",
            AppError::InvalidInput(_) => "InvalidInput",
            AppError::Database(_) => "Database",
            AppError::Config(_) => "Config",
            AppError::Conflict(_) => "Conflict",
            AppError::DuplicateCode(_) => "DuplicateCode",
            AppError::ReadOnly(_) => "ReadOnly",
            AppError::Busy(_) => "Busy",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        use rusqlite::ErrorCode;
        if let rusqlite::Error::SqliteFailure(f, _) = &e {
            match f.code {
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => {
                    return AppError::Busy(
                        "La base de datos está ocupada por otra aplicación. Inténtelo de nuevo."
                            .into(),
                    )
                }
                ErrorCode::ReadOnly => {
                    return AppError::ReadOnly("La base de datos está en modo solo lectura.".into())
                }
                _ => {}
            }
        }
        AppError::Database(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_kind_and_message() {
        let v = serde_json::to_value(AppError::NotFound("x".into())).unwrap();
        assert_eq!(v, serde_json::json!({"kind":"NotFound","message":"x"}));
    }

    #[test]
    fn new_kinds() {
        for (e, k) in [
            (AppError::Conflict("c".into()), "Conflict"),
            (AppError::DuplicateCode("d".into()), "DuplicateCode"),
            (AppError::ReadOnly("r".into()), "ReadOnly"),
            (AppError::Busy("b".into()), "Busy"),
        ] {
            assert_eq!(serde_json::to_value(e).unwrap()["kind"], k);
        }
    }

    #[test]
    fn busy_and_locked_map_to_busy() {
        for code in [rusqlite::ffi::SQLITE_BUSY, rusqlite::ffi::SQLITE_LOCKED] {
            let e: AppError =
                rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), None).into();
            assert_eq!(e.kind(), "Busy");
        }
        let e: AppError = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_READONLY),
            None,
        )
        .into();
        assert_eq!(e.kind(), "ReadOnly");
    }
}
