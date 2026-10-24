//! Frammenti di testo salvati dai lettori sui post pubblicati.

include!(concat!(env!("OUT_DIR"), "/module_gen.rs"));

mod handlers_impl;

pub mod handlers {
    //! Handler delle rotte `custom` di api.toml.
    pub use crate::handlers_impl::*;
}

/// Un frammento non può superare questa quota del testo del post.
pub const MAX_FRAGMENT_RATIO: f64 = 0.15;

/// Ripulisce e valida il testo di un frammento rispetto al contenuto Markdown grezzo del post (un
/// proxy del testo reso: il backend non fa rendering).
pub fn validate_text(text: &str, post_content: &str) -> Result<String, String> {
    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return Err("Il frammento non può essere vuoto.".into());
    }
    let max = ((post_content.chars().count() as f64 * MAX_FRAGMENT_RATIO) as usize).max(1);
    if cleaned.chars().count() > max {
        return Err("Il frammento selezionato supera il 15% del testo del post.".into());
    }
    Ok(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_are_cleaned_and_capped_to_fifteen_percent() {
        let post = "a".repeat(100);
        assert_eq!(validate_text("  ciao   mondo ", &post).unwrap(), "ciao mondo");
        assert!(validate_text(&"b".repeat(15), &post).is_ok());
        assert!(validate_text(&"b".repeat(16), &post).unwrap_err().contains("15%"));
        assert!(validate_text("   ", &post).is_err());
        // un post cortissimo ammette comunque un carattere
        assert!(validate_text("x", "ab").is_ok());
    }
}
