//! Hook di avvio: alla prima installazione l'elenco degli interessi si semina da `NOCT_DEFAULT_INTERESTS`
//! (JSON, stesso schema di `GET /interests`) o, in sua assenza, da un elenco curato. Dopo la prima
//! riga vale solo ciò che l'amministratore salva da `/admin/config`.

use crate::admin_sections::validate_list;
use noct_core::AppState;
use serde_json::{Value, json};

type Builtin = (&'static str, &'static [(&'static str, &'static str)]);

/// Elenco curato, con le traduzioni nelle lingue già usate come filtro rapido dal frontend.
const DEFAULT_INTERESTS: &[Builtin] = &[
    ("music", &[("it", "Musica"), ("en", "Music"), ("de", "Musik"), ("fr", "Musique")]),
    ("technology", &[("it", "Tecnologia"), ("en", "Technology"), ("de", "Technologie"), ("fr", "Technologie")]),
    ("photography", &[("it", "Fotografia"), ("en", "Photography"), ("de", "Fotografie"), ("fr", "Photographie")]),
    ("literature", &[("it", "Letteratura"), ("en", "Literature"), ("de", "Literatur"), ("fr", "Littérature")]),
    ("cinema", &[("it", "Cinema"), ("en", "Cinema"), ("de", "Kino"), ("fr", "Cinéma")]),
    ("travel", &[("it", "Viaggi"), ("en", "Travel"), ("de", "Reisen"), ("fr", "Voyages")]),
    ("food", &[("it", "Cucina"), ("en", "Food"), ("de", "Kochen"), ("fr", "Cuisine")]),
    ("art", &[("it", "Arte"), ("en", "Art"), ("de", "Kunst"), ("fr", "Art")]),
    ("nature", &[("it", "Natura"), ("en", "Nature"), ("de", "Natur"), ("fr", "Nature")]),
    ("science", &[("it", "Scienza"), ("en", "Science"), ("de", "Wissenschaft"), ("fr", "Science")]),
    ("sports", &[("it", "Sport"), ("en", "Sports"), ("de", "Sport"), ("fr", "Sport")]),
    ("gaming", &[("it", "Videogiochi"), ("en", "Gaming"), ("de", "Gaming"), ("fr", "Jeux vidéo")]),
    ("fashion", &[("it", "Moda"), ("en", "Fashion"), ("de", "Mode"), ("fr", "Mode")]),
    ("politics", &[("it", "Politica"), ("en", "Politics"), ("de", "Politik"), ("fr", "Politique")]),
    ("philosophy", &[("it", "Filosofia"), ("en", "Philosophy"), ("de", "Philosophie"), ("fr", "Philosophie")]),
    ("history", &[("it", "Storia"), ("en", "History"), ("de", "Geschichte"), ("fr", "Histoire")]),
];

pub fn builtin() -> Vec<Value> {
    DEFAULT_INTERESTS
        .iter()
        .map(|(key, labels)| {
            let translations: serde_json::Map<String, Value> =
                labels.iter().map(|(l, v)| ((*l).to_string(), json!(v))).collect();
            json!({"key": key, "translations": translations})
        })
        .collect()
}

pub async fn run(st: AppState) -> Result<(), String> {
    let items = match st.module_config_str("interests", "default_interests") {
        // un valore malformato non deve impedire l'avvio: si ricade sull'elenco curato, con un avviso
        Some(raw) => match serde_json::from_str::<Vec<Value>>(raw)
            .map_err(|e| e.to_string())
            .and_then(|v| validate_list(&v).map_err(|e| e.detail))
        {
            Ok(list) => list,
            Err(e) => {
                tracing::warn!(error = %e, "NOCT_DEFAULT_INTERESTS non valida: uso l'elenco predefinito");
                builtin()
            }
        },
        None => builtin(),
    };
    sqlx::query("INSERT INTO interests_settings (id, items) VALUES (1, $1) ON CONFLICT (id) DO NOTHING")
        .bind(Value::Array(items))
        .execute(&st.db)
        .await
        .map_err(|e| format!("interessi di piattaforma: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builtin_list_is_valid_and_complete() {
        let list = builtin();
        assert_eq!(list.len(), 16);
        assert_eq!(validate_list(&list).unwrap(), list, "già normalizzata");
        assert_eq!(list[0]["translations"]["it"], "Musica");
    }
}
