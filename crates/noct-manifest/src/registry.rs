use crate::{ColType, Issues, Kind, Manifest, topological_order};
use std::collections::{BTreeMap, BTreeSet};

/// Controlli che coinvolgono più moduli: dipendenze, collisioni di nomi/rotte,
/// FK tra moduli, regola "il core non dipende da moduli opzionali".
pub fn validate_registry(manifests: &[&Manifest]) -> Result<(), Issues> {
    let mut issues = Issues::default();
    let mut by_name: BTreeMap<&str, &Manifest> = BTreeMap::new();
    for m in manifests {
        if by_name.insert(m.name(), m).is_some() {
            issues.push(m.name(), "module.toml", "nome modulo duplicato");
        }
    }

    for m in manifests {
        for req in &m.module.module.requires {
            match by_name.get(req.as_str()) {
                None => {
                    issues.push(m.name(), "module.toml", format!("`requires` punta a un modulo inesistente: `{req}`"))
                }
                Some(dep) if m.kind() == Kind::Core && dep.kind() == Kind::Optional => issues.push(
                    m.name(),
                    "module.toml",
                    format!("un modulo core non può dipendere dal modulo opzionale `{req}`"),
                ),
                Some(_) => {}
            }
        }
    }
    if let Err(cycle) = topological_order(manifests) {
        issues.push("?", "module.toml", format!("ciclo di dipendenze tra moduli: {}", cycle.join(", ")));
    }

    // chi pubblica eventi dalle rotte passa dall'outbox, dichiarata dal modulo core `events`
    for m in manifests {
        let emits = m.api.routes.iter().any(|r| r.emits.is_some());
        if emits && m.name() != "events" && !m.module.module.requires.iter().any(|r| r == "events") {
            issues.push(
                m.name(),
                "module.toml",
                "le rotte con `emits` richiedono `requires = [\"events\"]` (outbox transazionale)",
            );
        }
    }

    // proprietà globali: tabelle, enum, rotte, subject dei job, variabili d'ambiente
    let mut tables: BTreeMap<&str, &str> = BTreeMap::new();
    let mut enums: BTreeMap<&str, &str> = BTreeMap::new();
    let mut routes: BTreeMap<(&'static str, String), &str> = BTreeMap::new();
    let mut job_subjects: BTreeMap<&str, &str> = BTreeMap::new();
    let mut envs: BTreeMap<&str, &str> = BTreeMap::new();
    for m in manifests {
        for t in &m.schema.tables {
            if let Some(other) = tables.insert(&t.name, m.name()) {
                issues.push(
                    m.name(),
                    format!("db.toml table {}", t.name),
                    format!("tabella già definita dal modulo `{other}`"),
                );
            }
        }
        for e in &m.schema.enums {
            if let Some(other) = enums.insert(&e.name, m.name()) {
                issues.push(
                    m.name(),
                    format!("db.toml enum {}", e.name),
                    format!("enum già definito dal modulo `{other}`"),
                );
            }
        }
        for r in &m.api.routes {
            if let Some(other) = routes.insert((r.method.as_str(), r.shape()), m.name()) {
                issues.push(
                    m.name(),
                    format!("api.toml route {}", r.id),
                    format!("{} {} collide con una rotta del modulo `{other}`", r.method.as_str(), r.path),
                );
            }
        }
        for (job, spec) in &m.module.jobs {
            if let Some(other) = job_subjects.insert(&spec.subject, m.name()) {
                issues.push(
                    m.name(),
                    format!("module.toml jobs {job}"),
                    format!("subject già usato dal modulo `{other}`"),
                );
            }
        }
        for (key, f) in &m.module.config {
            if let Some(other) = envs.insert(&f.env, m.name()) {
                issues.push(
                    m.name(),
                    format!("module.toml config {key}"),
                    format!("variabile {} già usata dal modulo `{other}`", f.env),
                );
            }
        }
    }

    // FK e tipi enum: solo verso tabelle/enum proprie o di un modulo in `requires`
    for m in manifests {
        let visible: BTreeSet<&str> =
            std::iter::once(m.name()).chain(m.module.module.requires.iter().map(String::as_str)).collect();
        for t in &m.schema.tables {
            for c in &t.columns {
                let loc = format!("db.toml table {} column {}", t.name, c.name);
                if let ColType::Enum(e) = &c.ty {
                    match enums.get(e.as_str()) {
                        None => issues.push(m.name(), &loc, format!("enum inesistente `{e}`")),
                        Some(owner) if !visible.contains(owner) => issues.push(
                            m.name(),
                            &loc,
                            format!("l'enum `{e}` appartiene a `{owner}`, che non è in `requires`"),
                        ),
                        _ => {}
                    }
                }
                let Some(r) = &c.references else { continue };
                let Some(owner) = tables.get(r.table.as_str()) else {
                    issues.push(m.name(), &loc, format!("FK verso tabella inesistente `{}`", r.table));
                    continue;
                };
                if !visible.contains(owner) {
                    issues.push(
                        m.name(),
                        &loc,
                        format!("FK verso `{}` del modulo `{owner}`, che non è in `requires`", r.table),
                    );
                    continue;
                }
                let target = by_name[owner].schema.table(&r.table).and_then(|tt| tt.column(&r.column));
                match target {
                    None => {
                        issues.push(m.name(), &loc, format!("FK verso colonna inesistente `{}.{}`", r.table, r.column))
                    }
                    Some(tc) if tc.ty != c.ty => issues.push(
                        m.name(),
                        &loc,
                        format!("tipo della FK ({}) diverso da `{}.{}` ({})", c.ty, r.table, r.column, tc.ty),
                    ),
                    Some(_) => {
                        if r.on_delete == crate::OnDelete::SetNull && !c.null {
                            issues.push(
                                m.name(),
                                &loc,
                                "`on_delete = \"set_null\"` richiede una colonna `null = true`",
                            );
                        }
                    }
                }
            }
        }
    }

    issues.into_result(())
}
