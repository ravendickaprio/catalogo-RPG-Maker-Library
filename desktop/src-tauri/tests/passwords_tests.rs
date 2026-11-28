//! Real OS credential store (Windows Credential Manager / Secret Service). Ignored by default
//! because CI runners have no unlocked keyring: `cargo test --test passwords_tests -- --ignored`.

use rpg_library_lib::passwords::{delete, get, set, with_password};

const SERVICE: &str = "local.rpgmakerlibrary.desktop.pagina.test";

#[test]
#[ignore = "usa el almacén de credenciales real del sistema"]
fn create_edit_query_delete_against_the_real_store() {
    let page = format!("Prueba-{}", std::process::id());
    let other = format!("otra-{}", std::process::id());
    delete(SERVICE, &page).unwrap();

    assert_eq!(get(SERVICE, &page).unwrap(), None);
    set(SERVICE, &page, "primera").unwrap();
    // Belongs to the page, case/space-insensitive.
    assert_eq!(
        get(SERVICE, &format!("  {}  ", page.to_uppercase()))
            .unwrap()
            .as_deref(),
        Some("primera")
    );
    set(SERVICE, &page, "segunda ñ 日本").unwrap();
    assert_eq!(
        get(SERVICE, &page).unwrap().as_deref(),
        Some("segunda ñ 日本")
    );

    let status = with_password(
        SERVICE,
        &[page.clone(), other.clone(), " ".into(), "x".repeat(101)],
    )
    .unwrap();
    assert_eq!(status, vec![page.clone()]);

    delete(SERVICE, &page).unwrap();
    assert_eq!(get(SERVICE, &page).unwrap(), None);
    delete(SERVICE, &page).unwrap(); // idempotent
}
