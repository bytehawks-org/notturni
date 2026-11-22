use noct_manifest::{ApiFile, ColType, Field, FieldType, RouteKind, Schema, type_ref};
use std::collections::BTreeMap;

/// Come un tipo di `api.toml` viene usato dalle rotte: determina i derive e il
/// tipo Rust dei campi opzionali.
#[derive(Debug, Default, Clone)]
pub struct Usage {
    pub table: Option<String>,
    pub request: bool,
    pub response: bool,
    pub update_request: bool,
}

pub fn usages(api: &ApiFile) -> Result<BTreeMap<String, Usage>, String> {
    let mut map: BTreeMap<String, Usage> = BTreeMap::new();
    for r in &api.routes {
        let mut touch = |name: &str, request: bool, response: bool| -> Result<(), String> {
            let u = map.entry(name.to_string()).or_default();
            u.request |= request;
            u.response |= response;
            if request && r.kind == RouteKind::Update {
                u.update_request = true;
            }
            if let Some(t) = &r.table {
                match &u.table {
                    Some(prev) if prev != t => {
                        return Err(format!(
                            "il tipo `{name}` è usato con tabelle diverse (`{prev}` e `{t}`): separare i tipi"
                        ));
                    }
                    _ => u.table = Some(t.clone()),
                }
            }
            Ok(())
        };
        if let Some(t) = &r.request {
            touch(type_ref(t).0, true, false)?;
        }
        if let Some(t) = &r.response {
            touch(type_ref(t).0, false, true)?;
        }
    }
    for (name, u) in &map {
        if u.update_request && u.response {
            return Err(format!(
                "il tipo `{name}` è usato sia come richiesta di update sia come risposta: separare i tipi"
            ));
        }
    }
    Ok(map)
}

pub const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
    "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "static", "struct", "super", "trait",
    "true", "type", "unsafe", "use", "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do",
    "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen",
];

pub fn rust_ident(name: &str) -> String {
    if RUST_KEYWORDS.contains(&name) { format!("r#{name}") } else { name.to_string() }
}

/// Tipo Rust base di un campo; per i campi legati a una colonna conta la colonna
/// (es. `integer` → `i32`), altrimenti il tipo dichiarato.
pub fn base_type(f: &Field, col: Option<&noct_manifest::Column>) -> String {
    match f.field_type() {
        FieldType::String | FieldType::Enum(_) => "String".into(),
        FieldType::Uuid => "uuid::Uuid".into(),
        FieldType::Int => match col.map(|c| &c.ty) {
            Some(ColType::Integer) => "i32".into(),
            _ => "i64".into(),
        },
        FieldType::Bool => "bool".into(),
        FieldType::Timestamp => "chrono::DateTime<chrono::Utc>".into(),
        FieldType::Json => "serde_json::Value".into(),
        FieldType::StringList => "Vec<String>".into(),
        FieldType::Float => "f64".into(),
    }
}

/// Tipo Rust completo del campo (con `Option`/`Patch`) secondo l'uso del tipo.
pub fn field_type(f: &Field, col: Option<&noct_manifest::Column>, usage: &Usage) -> String {
    let base = base_type(f, col);
    if usage.update_request {
        if f.nullable {
            return format!("noct_core::Patch<{base}>");
        }
        return format!("Option<{base}>");
    }
    if f.optional || f.nullable { format!("Option<{base}>") } else { base }
}

pub fn column<'a>(schema: &'a Schema, usage: &Usage, field: &str) -> Option<&'a noct_manifest::Column> {
    usage.table.as_deref().and_then(|t| schema.table(t)).and_then(|t| t.column(field))
}
