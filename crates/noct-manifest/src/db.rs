use crate::{Issues, is_ident};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::BTreeSet, fmt, str::FromStr};

// ---- forma grezza di db.toml -------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDb {
    #[serde(default, rename = "enum")]
    enums: Vec<EnumDef>,
    #[serde(default, rename = "table")]
    tables: Vec<RawTable>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTable {
    name: String,
    #[serde(default)]
    timestamps: bool,
    columns: Vec<RawColumn>,
    #[serde(default)]
    indexes: Vec<RawIndex>,
    #[serde(default)]
    uniques: Vec<RawUnique>,
    #[serde(default)]
    primary_key: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawColumn {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    pk: bool,
    #[serde(default)]
    null: bool,
    #[serde(default)]
    default: Option<String>,
    #[serde(default, rename = "ref")]
    reference: Option<String>,
    #[serde(default)]
    on_delete: Option<OnDelete>,
    #[serde(default)]
    renamed_from: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIndex {
    #[serde(default)]
    name: Option<String>,
    columns: Vec<String>,
    #[serde(default)]
    unique: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUnique {
    #[serde(default)]
    name: Option<String>,
    columns: Vec<String>,
}

// ---- schema normalizzato (anche formato di schema.lock.json) ---------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumDef {
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnDelete {
    #[default]
    Restrict,
    Cascade,
    SetNull,
}

impl OnDelete {
    pub fn sql(self) -> &'static str {
        match self {
            OnDelete::Restrict => "RESTRICT",
            OnDelete::Cascade => "CASCADE",
            OnDelete::SetNull => "SET NULL",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColType {
    Text,
    Varchar(u32),
    Uuid,
    Integer,
    BigInt,
    Boolean,
    Timestamptz,
    Jsonb,
    Enum(String),
}

impl fmt::Display for ColType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColType::Text => f.write_str("text"),
            ColType::Varchar(n) => write!(f, "varchar({n})"),
            ColType::Uuid => f.write_str("uuid"),
            ColType::Integer => f.write_str("integer"),
            ColType::BigInt => f.write_str("bigint"),
            ColType::Boolean => f.write_str("boolean"),
            ColType::Timestamptz => f.write_str("timestamptz"),
            ColType::Jsonb => f.write_str("jsonb"),
            ColType::Enum(name) => write!(f, "enum:{name}"),
        }
    }
}

impl FromStr for ColType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        Ok(match s {
            "text" => ColType::Text,
            "uuid" => ColType::Uuid,
            "integer" => ColType::Integer,
            "bigint" => ColType::BigInt,
            "boolean" => ColType::Boolean,
            "timestamptz" => ColType::Timestamptz,
            "jsonb" => ColType::Jsonb,
            other => {
                if let Some(rest) = other.strip_prefix("varchar(").and_then(|r| r.strip_suffix(')')) {
                    let n: u32 = rest.parse().map_err(|_| format!("lunghezza varchar non valida: `{other}`"))?;
                    if n == 0 || n > 10_485_760 {
                        return Err(format!("lunghezza varchar fuori intervallo: `{other}`"));
                    }
                    ColType::Varchar(n)
                } else if let Some(name) = other.strip_prefix("enum:") {
                    if !is_ident(name) {
                        return Err(format!("nome enum non valido: `{name}`"));
                    }
                    ColType::Enum(name.to_string())
                } else {
                    return Err(format!(
                        "tipo colonna sconosciuto `{other}` (ammessi: text, varchar(n), uuid, integer, bigint, boolean, timestamptz, jsonb, enum:<nome>)"
                    ));
                }
            }
        })
    }
}

impl ColType {
    /// Tipo SQL per `CREATE TABLE`/`ALTER TABLE`.
    pub fn sql(&self) -> String {
        match self {
            ColType::Enum(name) => format!("\"{name}\""),
            other => other.to_string(),
        }
    }
}

impl Serialize for ColType {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ColType {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub table: String,
    pub column: String,
    pub on_delete: OnDelete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: ColType,
    pub null: bool,
    pub default: Option<String>,
    pub references: Option<Reference>,
    pub renamed_from: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unique {
    pub name: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub primary_key: Vec<String>,
    pub indexes: Vec<Index>,
    pub uniques: Vec<Unique>,
}

impl Table {
    pub fn column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// Un vincolo di unicità (constraint o indice unico) con questo nome.
    pub fn has_unique_named(&self, name: &str) -> bool {
        self.uniques.iter().any(|u| u.name == name) || self.indexes.iter().any(|i| i.unique && i.name == name)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schema {
    pub enums: Vec<EnumDef>,
    pub tables: Vec<Table>,
}

impl Schema {
    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.name == name)
    }

    pub fn enum_def(&self, name: &str) -> Option<&EnumDef> {
        self.enums.iter().find(|e| e.name == name)
    }

    /// Ordine deterministico (tabelle ed enum per nome; le colonne mantengono
    /// l'ordine dichiarato): base del confronto con `schema.lock.json`.
    pub fn canonical(&self) -> Schema {
        let mut s = self.clone();
        s.enums.sort_by(|a, b| a.name.cmp(&b.name));
        s.tables.sort_by(|a, b| a.name.cmp(&b.name));
        s
    }

    pub(crate) fn parse(module: &str, text: &str, issues: &mut Issues) -> Schema {
        let raw: RawDb = match toml::from_str(text) {
            Ok(raw) => raw,
            Err(err) => {
                issues.push(module, "db.toml", err.to_string());
                return Schema::default();
            }
        };
        let mut schema = Schema::default();
        let mut enum_names = BTreeSet::new();
        for e in raw.enums {
            let loc = format!("db.toml enum {}", e.name);
            if !is_ident(&e.name) {
                issues.push(module, &loc, "nome enum non valido (minuscole, cifre, `_`)");
            }
            if !enum_names.insert(e.name.clone()) {
                issues.push(module, &loc, "enum duplicato");
            }
            if e.values.is_empty() {
                issues.push(module, &loc, "un enum deve avere almeno un valore");
            }
            let mut seen = BTreeSet::new();
            for v in &e.values {
                if v.is_empty() || v.contains('\'') {
                    issues.push(module, &loc, format!("valore enum non valido: `{v}`"));
                }
                if !seen.insert(v) {
                    issues.push(module, &loc, format!("valore enum duplicato: `{v}`"));
                }
            }
            schema.enums.push(e);
        }
        let mut table_names = BTreeSet::new();
        for raw_table in raw.tables {
            let loc = format!("db.toml table {}", raw_table.name);
            if !is_ident(&raw_table.name) {
                issues.push(module, &loc, "nome tabella non valido (minuscole, cifre, `_`)");
            }
            if !table_names.insert(raw_table.name.clone()) {
                issues.push(module, &loc, "tabella duplicata");
            }
            schema.tables.push(normalize_table(module, &loc, raw_table, issues));
        }
        schema
    }
}

fn normalize_table(module: &str, loc: &str, raw: RawTable, issues: &mut Issues) -> Table {
    let mut columns: Vec<Column> = Vec::new();
    let mut pk: Vec<String> = Vec::new();
    for c in raw.columns {
        let cloc = format!("{loc} column {}", c.name);
        if !is_ident(&c.name) {
            issues.push(module, &cloc, "nome colonna non valido (minuscole, cifre, `_`)");
        }
        if columns.iter().any(|x| x.name == c.name) {
            issues.push(module, &cloc, "colonna duplicata");
        }
        let ty = match c.ty.parse::<ColType>() {
            Ok(ty) => ty,
            Err(msg) => {
                issues.push(module, &cloc, msg);
                ColType::Text
            }
        };
        if let Some(d) = &c.default
            && (d.contains(';') || d.contains("--") || d.trim().is_empty())
        {
            issues.push(module, &cloc, "default non valido (vuoto, `;` o `--` non ammessi)");
        }
        let references = c.reference.as_deref().and_then(|r| match r.split_once('.') {
            Some((t, col)) if is_ident(t) && is_ident(col) => Some(Reference {
                table: t.to_string(),
                column: col.to_string(),
                on_delete: c.on_delete.unwrap_or_default(),
            }),
            _ => {
                issues.push(module, &cloc, format!("`ref` deve essere `tabella.colonna`, trovato `{r}`"));
                None
            }
        });
        if c.on_delete.is_some() && references.is_none() {
            issues.push(module, &cloc, "`on_delete` richiede `ref`");
        }
        if let Some(old) = &c.renamed_from
            && (!is_ident(old) || old == &c.name)
        {
            issues.push(module, &cloc, "`renamed_from` non valido");
        }
        if c.pk {
            pk.push(c.name.clone());
        }
        columns.push(Column {
            null: c.null && !c.pk,
            name: c.name,
            ty,
            default: c.default,
            references,
            renamed_from: c.renamed_from,
        });
    }
    if raw.timestamps {
        for name in ["created_at", "updated_at"] {
            if columns.iter().any(|c| c.name == name) {
                issues.push(module, loc, format!("`timestamps = true` ma `{name}` è già dichiarata"));
            } else {
                columns.push(Column {
                    name: name.to_string(),
                    ty: ColType::Timestamptz,
                    null: false,
                    default: Some("now()".to_string()),
                    references: None,
                    renamed_from: None,
                });
            }
        }
    }
    if let Some(explicit) = raw.primary_key {
        if !pk.is_empty() {
            issues.push(module, loc, "usare `pk = true` sulle colonne oppure `primary_key`, non entrambi");
        }
        pk = explicit;
    }
    if pk.is_empty() {
        issues.push(module, loc, "manca la chiave primaria (`pk = true` o `primary_key = [...]`)");
    }
    let names: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();
    let check_cols = |cols: &[String], what: &str, issues: &mut Issues| {
        if cols.is_empty() {
            issues.push(module, loc, format!("{what}: nessuna colonna"));
        }
        for c in cols {
            if !names.contains(c) {
                issues.push(module, loc, format!("{what}: colonna inesistente `{c}`"));
            }
        }
    };
    check_cols(&pk, "primary key", issues);
    for c in columns.iter_mut() {
        if pk.contains(&c.name) {
            c.null = false;
        }
    }
    let mut indexes = Vec::new();
    for i in raw.indexes {
        check_cols(&i.columns, "indice", issues);
        let prefix = if i.unique { "uq" } else { "ix" };
        let name = i.name.unwrap_or_else(|| format!("{prefix}_{}_{}", raw.name, i.columns.join("_")));
        indexes.push(Index { name, columns: i.columns, unique: i.unique });
    }
    let mut uniques = Vec::new();
    for u in raw.uniques {
        check_cols(&u.columns, "unique", issues);
        let name = u.name.unwrap_or_else(|| format!("uq_{}_{}", raw.name, u.columns.join("_")));
        uniques.push(Unique { name, columns: u.columns });
    }
    let mut names = BTreeSet::new();
    for n in indexes.iter().map(|i| &i.name).chain(uniques.iter().map(|u| &u.name)) {
        if !is_ident(n) || !names.insert(n.clone()) {
            issues.push(module, loc, format!("nome di indice/unique non valido o duplicato: `{n}`"));
        }
    }
    Table { name: raw.name, columns, primary_key: pk, indexes, uniques }
}
