use crate::diff::Plan;
use sha2::{Digest, Sha256};

/// SHA-256 esadecimale del contenuto del file di migrazione.
pub fn checksum(sql: &str) -> String {
    hex::encode(Sha256::digest(sql.as_bytes()))
}

/// `0003_add_x.sql` → `(3, "add_x")`.
pub fn parse_file_name(file: &str) -> Option<(u32, String)> {
    let stem = file.strip_suffix(".sql")?;
    let (num, name) = stem.split_once('_')?;
    if num.len() != 4
        || name.is_empty()
        || !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return None;
    }
    Some((num.parse().ok().filter(|n| *n > 0)?, name.to_string()))
}

/// Contenuto di un file di migrazione generato da un [`Plan`].
pub fn render_file(module: &str, version: u32, plan: &Plan) -> String {
    let mut out = format!("-- noct: generato da `noct-ctl migrate generate` — modulo={module} versione={version}\n");
    out.push_str("-- Rivedere prima del commit; non modificare dopo l'applicazione (il checksum è registrato).\n");
    for w in &plan.warnings {
        out.push_str(&format!("-- ATTENZIONE: {w}\n"));
    }
    for s in &plan.steps {
        out.push_str(&format!("\n-- {}\n{}\n", s.description, s.sql));
    }
    out
}
