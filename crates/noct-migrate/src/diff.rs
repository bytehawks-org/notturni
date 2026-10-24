use crate::render::{column_def, create_index_sql, create_table_sql, quote, quote_list};
use noct_manifest::{Column, Schema, Table};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub description: String,
    pub sql: String,
}

/// Esito del confronto fra due schemi.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Istruzioni generabili in automatico, nell'ordine di esecuzione.
    pub steps: Vec<Step>,
    /// Cambiamenti che richiedono una migrazione scritta a mano.
    pub blocked: Vec<String>,
    /// Passi generati ma da rivedere (possono fallire su dati esistenti).
    pub warnings: Vec<String>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty() && self.blocked.is_empty()
    }

    fn step(&mut self, description: impl Into<String>, sql: impl Into<String>) {
        self.steps.push(Step { description: description.into(), sql: sql.into() });
    }
}

/// Differenze tra l'istantanea `old` e lo schema desiderato `new`.
pub fn diff(old: &Schema, new: &Schema) -> Plan {
    let mut plan = Plan::default();

    // --- enum -----------------------------------------------------------------
    for e in &new.enums {
        match old.enum_def(&e.name) {
            None => {
                let values = e.values.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(", ");
                plan.step(
                    format!("crea enum {}", e.name),
                    format!("CREATE TYPE {} AS ENUM ({values});", quote(&e.name)),
                );
            }
            Some(o) => {
                for gone in o.values.iter().filter(|v| !e.values.contains(v)) {
                    plan.blocked.push(format!("enum `{}`: rimozione del valore `{gone}` (migrazione manuale)", e.name));
                }
                for added in e.values.iter().filter(|v| !o.values.contains(v)) {
                    plan.step(
                        format!("enum {}: aggiunge il valore {added}", e.name),
                        format!("ALTER TYPE {} ADD VALUE IF NOT EXISTS '{added}';", quote(&e.name)),
                    );
                }
            }
        }
    }
    for o in &old.enums {
        if new.enum_def(&o.name).is_none() {
            plan.blocked.push(format!("enum `{}` rimosso (DROP TYPE: migrazione manuale)", o.name));
        }
    }

    // --- tabelle nuove, in ordine di dipendenza delle FK interne -----------------
    let new_tables: Vec<&Table> = new.tables.iter().filter(|t| old.table(&t.name).is_none()).collect();
    match order_new_tables(&new_tables) {
        Ok(ordered) => {
            for t in ordered {
                plan.step(format!("crea tabella {}", t.name), create_table_sql(t));
            }
        }
        Err(cycle) => plan.blocked.push(format!("dipendenza circolare tra le nuove tabelle: {}", cycle.join(", "))),
    }
    for o in &old.tables {
        if new.table(&o.name).is_none() {
            plan.blocked.push(format!("tabella `{}` rimossa (DROP TABLE: migrazione manuale)", o.name));
        }
    }

    // --- tabelle esistenti -------------------------------------------------------
    for t in &new.tables {
        if let Some(o) = old.table(&t.name) {
            diff_table(o, t, &mut plan);
        }
    }
    plan
}

fn order_new_tables<'a>(tables: &[&'a Table]) -> Result<Vec<&'a Table>, Vec<String>> {
    let names: BTreeSet<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    let mut remaining: BTreeMap<&str, (&Table, BTreeSet<&str>)> = tables
        .iter()
        .map(|t| {
            let deps = t
                .columns
                .iter()
                .filter_map(|c| c.references.as_ref())
                .map(|r| r.table.as_str())
                .filter(|n| names.contains(n) && *n != t.name)
                .collect();
            (t.name.as_str(), (*t, deps))
        })
        .collect();
    let mut out = Vec::new();
    while !remaining.is_empty() {
        let ready: Vec<&str> = remaining.iter().filter(|(_, (_, d))| d.is_empty()).map(|(n, _)| *n).collect();
        if ready.is_empty() {
            return Err(remaining.keys().map(|s| s.to_string()).collect());
        }
        for n in ready {
            let (t, _) = remaining.remove(n).expect("presente");
            out.push(t);
            for (_, d) in remaining.values_mut() {
                d.remove(n);
            }
        }
    }
    Ok(out)
}

fn diff_table(old: &Table, new: &Table, plan: &mut Plan) {
    let tq = quote(&new.name);
    if old.primary_key != new.primary_key {
        plan.blocked.push(format!("tabella `{}`: cambio di chiave primaria (migrazione manuale)", new.name));
    }

    // colonne: rename dichiarati, nuove, modificate, rimosse
    let mut matched_old: BTreeSet<&str> = BTreeSet::new();
    for c in &new.columns {
        let loc = format!("`{}.{}`", new.name, c.name);
        if let Some(prev) = old.column(&c.name) {
            matched_old.insert(&prev.name);
            diff_column(&tq, &new.name, prev, c, plan);
            continue;
        }
        if let Some(from) = c.renamed_from.as_deref()
            && let Some(prev) = old.column(from)
        {
            matched_old.insert(&prev.name);
            plan.step(
                format!("rinomina {}.{from} → {}", new.name, c.name),
                format!("ALTER TABLE {tq} RENAME COLUMN {} TO {};", quote(from), quote(&c.name)),
            );
            diff_column(&tq, &new.name, prev, c, plan);
            continue;
        }
        // colonna nuova
        if new.primary_key.contains(&c.name) {
            plan.blocked.push(format!("colonna {loc}: nuova colonna di chiave primaria (migrazione manuale)"));
        } else if !c.null && c.default.is_none() {
            plan.blocked.push(format!(
                "colonna {loc}: NOT NULL senza default su una tabella esistente (aggiungere `default`, rendere la colonna `null = true` o scrivere una migrazione manuale)"
            ));
        } else {
            plan.step(
                format!("aggiunge colonna {}.{}", new.name, c.name),
                format!("ALTER TABLE {tq} ADD COLUMN {};", column_def(c, true)),
            );
        }
    }
    for prev in &old.columns {
        if !matched_old.contains(prev.name.as_str()) && new.column(&prev.name).is_none() {
            plan.blocked
                .push(format!("colonna `{}.{}` rimossa (DROP COLUMN: migrazione manuale)", new.name, prev.name));
        }
    }

    // indici
    for i in &new.indexes {
        match old.indexes.iter().find(|o| o.name == i.name) {
            None => {
                if i.unique {
                    plan.warnings.push(format!(
                        "indice unico `{}` su tabella esistente: fallisce se i dati hanno duplicati",
                        i.name
                    ));
                }
                plan.step(format!("crea indice {}", i.name), create_index_sql(&new.name, i));
            }
            Some(o) if o != i => plan.blocked.push(format!("indice `{}` modificato (migrazione manuale)", i.name)),
            Some(_) => {}
        }
    }
    for o in &old.indexes {
        if !new.indexes.iter().any(|i| i.name == o.name) {
            plan.step(format!("elimina indice {}", o.name), format!("DROP INDEX IF EXISTS {};", quote(&o.name)));
        }
    }
    // unique constraint
    for u in &new.uniques {
        match old.uniques.iter().find(|o| o.name == u.name) {
            None => {
                plan.warnings
                    .push(format!("vincolo `{}` su tabella esistente: fallisce se i dati hanno duplicati", u.name));
                plan.step(
                    format!("aggiunge vincolo {}", u.name),
                    format!("ALTER TABLE {tq} ADD CONSTRAINT {} UNIQUE ({});", quote(&u.name), quote_list(&u.columns)),
                );
            }
            Some(o) if o != u => plan.blocked.push(format!("vincolo `{}` modificato (migrazione manuale)", u.name)),
            Some(_) => {}
        }
    }
    for o in &old.uniques {
        if !new.uniques.iter().any(|u| u.name == o.name) {
            plan.warnings.push(format!("vincolo `{}` rimosso: la garanzia di unicità viene meno", o.name));
            plan.step(
                format!("elimina vincolo {}", o.name),
                format!("ALTER TABLE {tq} DROP CONSTRAINT {};", quote(&o.name)),
            );
        }
    }
}

fn diff_column(tq: &str, table: &str, old: &Column, new: &Column, plan: &mut Plan) {
    let loc = format!("`{table}.{}`", new.name);
    let cq = quote(&new.name);
    if old.ty != new.ty {
        plan.blocked.push(format!("colonna {loc}: cambio di tipo {} → {} (migrazione manuale)", old.ty, new.ty));
    }
    if old.references != new.references {
        plan.blocked.push(format!("colonna {loc}: cambio di foreign key (migrazione manuale)"));
    }
    match (old.null, new.null) {
        (false, true) => plan.step(
            format!("{table}.{}: ammette NULL", new.name),
            format!("ALTER TABLE {tq} ALTER COLUMN {cq} DROP NOT NULL;"),
        ),
        (true, false) => {
            plan.blocked.push(format!("colonna {loc}: NULL → NOT NULL (migrazione manuale, con backfill)"))
        }
        _ => {}
    }
    if old.default != new.default {
        let sql = match &new.default {
            Some(d) => format!("ALTER TABLE {tq} ALTER COLUMN {cq} SET DEFAULT {d};"),
            None => format!("ALTER TABLE {tq} ALTER COLUMN {cq} DROP DEFAULT;"),
        };
        plan.step(format!("{table}.{}: cambia il default", new.name), sql);
    }
}
