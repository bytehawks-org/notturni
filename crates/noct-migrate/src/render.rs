use noct_manifest::{Column, Table};

/// Identificatore quotato (gli identificatori dei manifest sono già `[a-z0-9_]`).
pub fn quote(ident: &str) -> String {
    format!("\"{ident}\"")
}

pub(crate) fn quote_list(cols: &[String]) -> String {
    cols.iter().map(|c| quote(c)).collect::<Vec<_>>().join(", ")
}

pub(crate) fn column_def(c: &Column, include_not_null: bool) -> String {
    let mut s = format!("{} {}", quote(&c.name), c.ty.sql());
    if !c.null && include_not_null {
        s.push_str(" NOT NULL");
    }
    if let Some(d) = &c.default {
        s.push_str(&format!(" DEFAULT {d}"));
    }
    if let Some(r) = &c.references {
        s.push_str(&format!(" REFERENCES {} ({}) ON DELETE {}", quote(&r.table), quote(&r.column), r.on_delete.sql()));
    }
    s
}

/// `CREATE TABLE` + indici, come un'unica stringa di istruzioni.
pub fn create_table_sql(t: &Table) -> String {
    let mut lines: Vec<String> = t.columns.iter().map(|c| format!("    {}", column_def(c, true))).collect();
    lines.push(format!("    PRIMARY KEY ({})", quote_list(&t.primary_key)));
    for u in &t.uniques {
        lines.push(format!("    CONSTRAINT {} UNIQUE ({})", quote(&u.name), quote_list(&u.columns)));
    }
    let mut sql = format!("CREATE TABLE {} (\n{}\n);", quote(&t.name), lines.join(",\n"));
    for i in &t.indexes {
        sql.push('\n');
        sql.push_str(&create_index_sql(&t.name, i));
    }
    sql
}

pub(crate) fn create_index_sql(table: &str, i: &noct_manifest::Index) -> String {
    format!(
        "CREATE {}INDEX {} ON {} ({});",
        if i.unique { "UNIQUE " } else { "" },
        quote(&i.name),
        quote(table),
        quote_list(&i.columns)
    )
}
