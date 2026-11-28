//! Contraseñas de descompresión por página (ADR-019), guardadas sólo en el almacén seguro
//! del sistema (Administrador de credenciales de Windows / Secret Service en Linux).
//! Nunca tocan SQLite ni se escriben en logs ni en los mensajes de error.

use crate::error::AppError;
use keyring::{Entry, Error as KError};

/// Servicio con el que se registran las credenciales en el almacén del sistema.
pub const SERVICE: &str = "local.rpgmakerlibrary.desktop.pagina";
const MAX_PASSWORD_CHARS: usize = 1024;

/// La contraseña es de la página, no del juego: «Ryuugames» y « ryuugames » comparten una.
pub fn page_key(pagina: &str) -> Result<String, AppError> {
    let k = pagina.trim().to_lowercase();
    if k.is_empty() {
        return Err(AppError::InvalidInput("La página está vacía.".into()));
    }
    if k.chars().count() > 100 {
        return Err(AppError::InvalidInput(
            "La página supera 100 caracteres.".into(),
        ));
    }
    Ok(k)
}

/// Fixed messages only: keyring errors may carry the secret bytes (BadEncoding).
fn map_err(e: KError) -> AppError {
    match e {
        KError::NoStorageAccess(_) | KError::PlatformFailure(_) => AppError::SecureStorage(
            "El almacén seguro de contraseñas del sistema no está disponible. \
             En Linux hace falta un servicio Secret Service activo (GNOME Keyring o KWallet). \
             No se guardó nada."
                .into(),
        ),
        KError::TooLong(..) | KError::Invalid(..) => {
            AppError::InvalidInput("El almacén del sistema rechazó la contraseña.".into())
        }
        _ => AppError::SecureStorage("Error del almacén seguro de contraseñas.".into()),
    }
}

fn entry(service: &str, pagina: &str) -> Result<Entry, AppError> {
    Entry::new(service, &page_key(pagina)?).map_err(map_err)
}

pub fn get(service: &str, pagina: &str) -> Result<Option<String>, AppError> {
    match entry(service, pagina)?.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(KError::NoEntry) => Ok(None),
        Err(e) => Err(map_err(e)),
    }
}

pub fn set(service: &str, pagina: &str, password: &str) -> Result<(), AppError> {
    if password.is_empty() {
        return Err(AppError::InvalidInput("La contraseña está vacía.".into()));
    }
    if password.chars().count() > MAX_PASSWORD_CHARS {
        return Err(AppError::InvalidInput(
            "La contraseña supera 1024 caracteres.".into(),
        ));
    }
    entry(service, pagina)?
        .set_password(password)
        .map_err(map_err)
}

/// Deleting a page without a password is not an error.
pub fn delete(service: &str, pagina: &str) -> Result<(), AppError> {
    match entry(service, pagina)?.delete_credential() {
        Ok(()) | Err(KError::NoEntry) => Ok(()),
        Err(e) => Err(map_err(e)),
    }
}

/// Pages (as given) that have a password. The secrets stay in Rust. Invalid page names (e.g. a
/// >100-char value written by another tool) are skipped instead of failing the whole list.
pub fn with_password(service: &str, paginas: &[String]) -> Result<Vec<String>, AppError> {
    let mut out = Vec::new();
    for p in paginas.iter().filter(|p| page_key(p).is_ok()) {
        if get(service, p)?.is_some() {
            out.push(p.clone());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_key_normalizes_and_validates() {
        assert_eq!(page_key("  Ryuugames ").unwrap(), "ryuugames");
        assert_eq!(
            page_key("ryuugames").unwrap(),
            page_key("RYUUGAMES").unwrap()
        );
        assert!(page_key("   ").is_err());
        assert!(page_key(&"x".repeat(101)).is_err());
    }

    #[test]
    fn errors_never_carry_the_secret() {
        let secret = "s3cr3t-pass";
        let e = map_err(KError::BadEncoding(secret.as_bytes().to_vec()));
        assert!(!format!("{e} {e:?}").contains(secret));
        let e = map_err(KError::NoStorageAccess(secret.into()));
        assert_eq!(e.kind(), "SecureStorage");
        assert!(!format!("{e} {e:?}").contains(secret));
    }

    #[test]
    fn rejects_empty_and_oversized_passwords_before_touching_the_store() {
        assert_eq!(set("unused", "p", "").unwrap_err().kind(), "InvalidInput");
        let big = "x".repeat(MAX_PASSWORD_CHARS + 1);
        assert_eq!(set("unused", "p", &big).unwrap_err().kind(), "InvalidInput");
    }
}
