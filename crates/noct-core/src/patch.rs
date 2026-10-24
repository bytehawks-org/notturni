use serde::{Deserialize, Deserializer};

/// Campo di un PATCH con tre stati: assente, `null` esplicito, valore.
/// Da usare con `#[serde(default)]` (il codice generato lo fa).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Patch<T> {
    #[default]
    Absent,
    Null,
    Value(T),
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Patch<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match Option::<T>::deserialize(d)? {
            None => Patch::Null,
            Some(v) => Patch::Value(v),
        })
    }
}

impl<T> Patch<T> {
    pub fn is_absent(&self) -> bool {
        matches!(self, Patch::Absent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Body {
        #[serde(default)]
        note: Patch<String>,
    }

    #[test]
    fn distinguishes_absent_null_and_value() {
        assert_eq!(serde_json::from_str::<Body>("{}").unwrap().note, Patch::Absent);
        assert_eq!(serde_json::from_str::<Body>(r#"{"note":null}"#).unwrap().note, Patch::Null);
        assert_eq!(serde_json::from_str::<Body>(r#"{"note":"x"}"#).unwrap().note, Patch::Value("x".into()));
    }
}
