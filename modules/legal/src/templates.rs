//! Modelli forniti con la piattaforma. Il testo segue la versione installata finché l'amministratore non lo
//! modifica; le lingue senza modello (le traduzioni aggiunte) partono dalla copia di un'altra lingua.

pub const DOCS: [&str; 2] = ["privacy", "cookie"];
pub const TEMPLATE_LOCALES: [&str; 2] = ["it", "en"];

/// Indirizzo della pagina pubblicata (`/p/{slug}`): fisso, perché gli altri testi del sito vi rimandano.
pub fn slug(doc: &str) -> Option<&'static str> {
    match doc {
        "privacy" => Some("privacy"),
        "cookie" => Some("cookie-policy"),
        _ => None,
    }
}

/// `(titolo, testo)` del modello per documento e lingua.
pub fn template(doc: &str, locale: &str) -> Option<(&'static str, &'static str)> {
    match (doc, locale) {
        ("privacy", "it") => Some(("Informativa sulla privacy", include_str!("../templates/privacy.it.md"))),
        ("privacy", "en") => Some(("Privacy policy", include_str!("../templates/privacy.en.md"))),
        ("cookie", "it") => Some(("Cookie policy", include_str!("../templates/cookie.it.md"))),
        ("cookie", "en") => Some(("Cookie policy", include_str!("../templates/cookie.en.md"))),
        _ => None,
    }
}
