use crate::Manifest;
use std::collections::BTreeMap;

/// Ordine di attivazione/migrazione: ogni modulo dopo quelli che richiede
/// (Kahn, a parità di livello in ordine alfabetico, quindi deterministico).
/// `Err` contiene i nomi dei moduli coinvolti in un ciclo.
pub fn topological_order(manifests: &[&Manifest]) -> Result<Vec<usize>, Vec<String>> {
    let index: BTreeMap<&str, usize> = manifests.iter().enumerate().map(|(i, m)| (m.name(), i)).collect();
    let mut remaining: BTreeMap<&str, Vec<&str>> = manifests
        .iter()
        .map(|m| {
            let deps = m.module.module.requires.iter().map(String::as_str).filter(|d| index.contains_key(d)).collect();
            (m.name(), deps)
        })
        .collect();
    let mut order = Vec::with_capacity(manifests.len());
    while !remaining.is_empty() {
        let ready: Vec<&str> = remaining.iter().filter(|(_, deps)| deps.is_empty()).map(|(n, _)| *n).collect();
        if ready.is_empty() {
            return Err(remaining.keys().map(|s| s.to_string()).collect());
        }
        for name in ready {
            remaining.remove(name);
            order.push(index[name]);
            for deps in remaining.values_mut() {
                deps.retain(|d| *d != name);
            }
        }
    }
    Ok(order)
}
