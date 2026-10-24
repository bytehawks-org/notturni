use crate::{ColType, Issues, Schema, is_ident};
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::BTreeMap, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }
}

/// Policy di accesso di una rotta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Auth {
    /// Anche anonimo; se la rotta è nel perimetro di un blog, vale la sua visibilità.
    Public,
    /// Qualsiasi utente autenticato.
    User,
    /// Proprietario del blog o membro con ruolo autore/co-autore (richiede `scope`).
    BlogWrite,
    /// Uno dei ruoli di piattaforma indicati.
    PlatformRole(Vec<String>),
}

impl FromStr for Auth {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "public" => Ok(Auth::Public),
            "user" => Ok(Auth::User),
            "blog_write" => Ok(Auth::BlogWrite),
            other => match other.strip_prefix("platform_role:") {
                Some(roles) if !roles.is_empty() => {
                    let roles: Vec<String> = roles.split(',').map(|r| r.trim().to_string()).collect();
                    if roles.iter().all(|r| is_ident(r)) {
                        Ok(Auth::PlatformRole(roles))
                    } else {
                        Err(format!("ruoli di piattaforma non validi in `{other}`"))
                    }
                }
                _ => Err(format!(
                    "auth sconosciuta `{other}` (ammesse: public, user, blog_write, platform_role:<ruolo>[,<ruolo>])"
                )),
            },
        }
    }
}

impl<'de> Deserialize<'de> for Auth {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RouteKind {
    List,
    Get,
    Create,
    Update,
    Delete,
    Custom,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    /// Nome del parametro di path che contiene lo slug del blog.
    pub blog: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Key {
    pub param: String,
    pub column: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conflict {
    pub constraint: String,
    pub status: u16,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateLimit {
    pub key: RateKey,
    pub max: u32,
    pub window: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RateKey {
    Ip,
    User,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub id: String,
    pub method: Method,
    pub path: String,
    pub auth: Auth,
    pub kind: RouteKind,
    #[serde(default)]
    pub scope: Option<Scope>,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub key: Option<Key>,
    #[serde(default)]
    pub order_by: Vec<String>,
    #[serde(default)]
    pub request: Option<String>,
    #[serde(default)]
    pub response: Option<String>,
    #[serde(default)]
    pub status: Option<u16>,
    #[serde(default)]
    pub not_found: Option<String>,
    #[serde(default)]
    pub conflict: Vec<Conflict>,
    #[serde(default)]
    pub rate_limit: Option<RateLimit>,
    #[serde(default)]
    pub handler: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
}

impl Route {
    /// Nomi dei parametri `{x}` del path.
    pub fn path_params(&self) -> Vec<&str> {
        self.path.split('/').filter_map(|seg| seg.strip_prefix('{').and_then(|s| s.strip_suffix('}'))).collect()
    }

    /// Status HTTP di successo (esplicito o di default per il tipo di rotta).
    pub fn success_status(&self) -> u16 {
        self.status.unwrap_or(match self.kind {
            RouteKind::Create => 201,
            RouteKind::Delete => 204,
            _ => 200,
        })
    }

    /// Path con i parametri normalizzati, per rilevare collisioni tra rotte.
    pub fn shape(&self) -> String {
        self.path.split('/').map(|seg| if seg.starts_with('{') { "{}" } else { seg }).collect::<Vec<_>>().join("/")
    }
}

/// `Nome` oppure `Nome[]`.
pub fn type_ref(name: &str) -> (&str, bool) {
    match name.strip_suffix("[]") {
        Some(base) => (base, true),
        None => (name, false),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    String,
    Uuid,
    Int,
    Bool,
    Timestamp,
    Json,
    Enum(String),
}

impl FromStr for FieldType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        Ok(match s {
            "string" => FieldType::String,
            "uuid" => FieldType::Uuid,
            "int" => FieldType::Int,
            "bool" => FieldType::Bool,
            "timestamp" => FieldType::Timestamp,
            "json" => FieldType::Json,
            other => match other.strip_prefix("enum:") {
                Some(name) if is_ident(name) => FieldType::Enum(name.to_string()),
                _ => {
                    return Err(format!(
                        "tipo campo sconosciuto `{other}` (ammessi: string, uuid, int, bool, timestamp, json, enum:<nome>)"
                    ));
                }
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    pub ty: FieldTypeName,
    pub optional: bool,
    pub nullable: bool,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub pattern: Option<String>,
    pub non_blank: bool,
    pub message: Option<String>,
}

/// Wrapper serializzabile (per OpenAPI/debug) attorno a [`FieldType`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldTypeName(pub FieldType);

impl Serialize for FieldTypeName {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&match &self.0 {
            FieldType::String => "string".to_string(),
            FieldType::Uuid => "uuid".to_string(),
            FieldType::Int => "int".to_string(),
            FieldType::Bool => "bool".to_string(),
            FieldType::Timestamp => "timestamp".to_string(),
            FieldType::Json => "json".to_string(),
            FieldType::Enum(n) => format!("enum:{n}"),
        })
    }
}

impl Field {
    pub fn field_type(&self) -> &FieldType {
        &self.ty.0
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawField {
    Short(String),
    Full(FullField),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FullField {
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    optional: bool,
    #[serde(default)]
    nullable: bool,
    #[serde(default)]
    min: Option<i64>,
    #[serde(default)]
    max: Option<i64>,
    #[serde(default)]
    pattern: Option<String>,
    #[serde(default)]
    non_blank: bool,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawApi {
    #[serde(default, rename = "type")]
    types: BTreeMap<String, BTreeMap<String, RawField>>,
    #[serde(default, rename = "route")]
    routes: Vec<Route>,
}

#[derive(Debug, Clone, Default)]
pub struct ApiFile {
    pub types: BTreeMap<String, BTreeMap<String, Field>>,
    pub routes: Vec<Route>,
}

impl ApiFile {
    pub(crate) fn parse(module: &str, text: &str, issues: &mut Issues) -> ApiFile {
        let raw: RawApi = match toml::from_str(text) {
            Ok(raw) => raw,
            Err(err) => {
                issues.push(module, "api.toml", err.to_string());
                return ApiFile::default();
            }
        };
        let mut types = BTreeMap::new();
        for (tname, fields) in raw.types {
            let mut out = BTreeMap::new();
            for (fname, raw_field) in fields {
                let loc = format!("api.toml type {tname}.{fname}");
                if !is_ident(&fname) {
                    issues.push(module, &loc, "nome campo non valido (minuscole, cifre, `_`)");
                }
                let full = match raw_field {
                    RawField::Short(s) => {
                        let (ty, optional) = match s.strip_suffix('?') {
                            Some(base) => (base.to_string(), true),
                            None => (s, false),
                        };
                        FullField {
                            ty,
                            optional,
                            nullable: false,
                            min: None,
                            max: None,
                            pattern: None,
                            non_blank: false,
                            message: None,
                        }
                    }
                    RawField::Full(f) => f,
                };
                let ty = match full.ty.parse::<FieldType>() {
                    Ok(ty) => ty,
                    Err(msg) => {
                        issues.push(module, &loc, msg);
                        FieldType::String
                    }
                };
                out.insert(
                    fname,
                    Field {
                        ty: FieldTypeName(ty),
                        optional: full.optional,
                        nullable: full.nullable,
                        min: full.min,
                        max: full.max,
                        pattern: full.pattern,
                        non_blank: full.non_blank,
                        message: full.message,
                    },
                );
            }
            if !tname.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                || !tname.chars().all(|c| c.is_ascii_alphanumeric())
            {
                issues.push(
                    module,
                    format!("api.toml type {tname}"),
                    "il nome del tipo deve essere CamelCase alfanumerico",
                );
            }
            types.insert(tname, out);
        }
        ApiFile { types, routes: raw.routes }
    }

    /// Controlli interni al modulo (riferimenti a tipi, tabelle, colonne).
    pub(crate) fn validate(&self, module: &str, schema: &Schema, issues: &mut Issues) {
        let mut ids = std::collections::BTreeSet::new();
        let mut shapes = std::collections::BTreeSet::new();

        for (tname, fields) in &self.types {
            for (fname, f) in fields {
                let loc = format!("api.toml type {tname}.{fname}");
                if let Some(p) = &f.pattern {
                    if f.field_type() != &FieldType::String {
                        issues.push(module, &loc, "`pattern` è ammesso solo sui campi string");
                    } else if let Err(e) = Regex::new(p) {
                        issues.push(module, &loc, format!("pattern non valido: {e}"));
                    }
                }
                if (f.min.is_some() || f.max.is_some()) && !matches!(f.field_type(), FieldType::String | FieldType::Int)
                {
                    issues.push(module, &loc, "`min`/`max` sono ammessi solo su string (lunghezza) e int (valore)");
                }
                if f.non_blank && f.field_type() != &FieldType::String {
                    issues.push(module, &loc, "`non_blank` è ammesso solo sui campi string");
                }
                if let FieldType::Enum(name) = f.field_type()
                    && schema.enum_def(name).is_none()
                {
                    issues.push(
                        module,
                        &loc,
                        format!("l'enum `{name}` deve essere definito in db.toml dello stesso modulo"),
                    );
                }
            }
        }

        for (i, r) in self.routes.iter().enumerate() {
            let loc = format!("api.toml route {}", if r.id.is_empty() { i.to_string() } else { r.id.clone() });
            if !is_ident(&r.id) {
                issues.push(module, &loc, "`id` non valido (minuscole, cifre, `_`)");
            }
            if !ids.insert(r.id.clone()) {
                issues.push(module, &loc, "id di rotta duplicato");
            }
            if !r.path.starts_with('/') || (r.path.len() > 1 && r.path.ends_with('/')) || r.path.contains("//") {
                issues.push(module, &loc, "il path deve iniziare con `/`, senza `/` finale né segmenti vuoti");
            }
            let params = r.path_params();
            for p in &params {
                if !is_ident(p) {
                    issues.push(module, &loc, format!("parametro di path non valido `{{{p}}}`"));
                }
            }
            if !shapes.insert((r.method, r.shape())) {
                issues.push(module, &loc, "stesso metodo e path di un'altra rotta del modulo");
            }
            if let Some(s) = &r.scope
                && !params.contains(&s.blog.as_str())
            {
                issues.push(module, &loc, format!("`scope.blog = \"{}\"` non è un parametro del path", s.blog));
            }
            if r.auth == Auth::BlogWrite && r.scope.is_none() {
                issues.push(module, &loc, "`auth = \"blog_write\"` richiede `scope = { blog = \"<param>\" }`");
            }
            if let Some(rl) = &r.rate_limit
                && (rl.max == 0 || parse_duration(&rl.window).is_none())
            {
                issues.push(module, &loc, "`rate_limit` richiede `max > 0` e `window` come `30s`, `5m`, `1h` o `1d`");
            }
            if let Some(s) = r.status
                && !(200..=299).contains(&s)
            {
                issues.push(module, &loc, "`status` deve essere un 2xx");
            }
            for c in &r.conflict {
                if !(400..=599).contains(&c.status) {
                    issues.push(module, &loc, "`conflict.status` deve essere 4xx/5xx");
                }
            }
            for (label, tref) in [("request", &r.request), ("response", &r.response)] {
                if let Some(t) = tref {
                    let (base, array) = type_ref(t);
                    if !self.types.contains_key(base) {
                        issues.push(module, &loc, format!("`{label}` usa il tipo inesistente `{base}`"));
                    }
                    if array && label == "request" {
                        issues.push(module, &loc, "`request` non può essere un array");
                    }
                }
            }
            self.validate_kind(module, &loc, r, schema, &params, issues);
        }
    }

    fn validate_kind(&self, module: &str, loc: &str, r: &Route, schema: &Schema, params: &[&str], issues: &mut Issues) {
        if r.kind == RouteKind::Custom {
            match &r.handler {
                Some(h) if is_ident(h) => {}
                _ => issues.push(module, loc, "una rotta `custom` richiede `handler` (nome di funzione Rust)"),
            }
            if r.table.is_some() || r.key.is_some() || !r.order_by.is_empty() || !r.conflict.is_empty() {
                issues.push(module, loc, "`table`/`key`/`order_by`/`conflict` sono ammessi solo sulle rotte CRUD");
            }
            if r.response.is_none() && r.success_status() != 204 {
                issues.push(module, loc, "una rotta `custom` senza `response` deve dichiarare `status = 204`");
            }
            return;
        }

        if r.handler.is_some() {
            issues.push(module, loc, "`handler` è ammesso solo sulle rotte `custom`");
        }
        let expected = match r.kind {
            RouteKind::List | RouteKind::Get => &[Method::Get][..],
            RouteKind::Create => &[Method::Post][..],
            RouteKind::Update => &[Method::Patch, Method::Put][..],
            RouteKind::Delete => &[Method::Delete][..],
            RouteKind::Custom => unreachable!(),
        };
        if !expected.contains(&r.method) {
            issues.push(
                module,
                loc,
                format!("il metodo {} non è coerente con il kind {:?}", r.method.as_str(), r.kind),
            );
        }
        let Some(table_name) = &r.table else {
            issues.push(module, loc, "le rotte CRUD richiedono `table`");
            return;
        };
        let Some(table) = schema.table(table_name) else {
            issues.push(module, loc, format!("la tabella `{table_name}` non è definita in db.toml di questo modulo"));
            return;
        };
        if r.scope.is_some() {
            match table.column("blog_id") {
                Some(c) if c.ty == ColType::Uuid => {}
                _ => issues.push(module, loc, format!("`scope` richiede la colonna uuid `blog_id` in `{table_name}`")),
            }
        }
        for col in &r.order_by {
            if table.column(col).is_none() {
                issues.push(module, loc, format!("`order_by`: colonna inesistente `{col}`"));
            }
        }
        for c in &r.conflict {
            if !table.has_unique_named(&c.constraint) {
                issues.push(
                    module,
                    loc,
                    format!("`conflict.constraint` `{}` non è un vincolo di unicità di `{table_name}`", c.constraint),
                );
            }
        }

        let needs_key = matches!(r.kind, RouteKind::Get | RouteKind::Update | RouteKind::Delete);
        match (&r.key, needs_key) {
            (Some(k), true) => {
                if !params.contains(&k.param.as_str()) {
                    issues.push(module, loc, format!("`key.param` `{}` non è un parametro del path", k.param));
                }
                match table.column(&k.column) {
                    Some(c) if c.ty == ColType::Uuid => {}
                    Some(_) => issues.push(module, loc, format!("`key.column` `{}` deve essere uuid", k.column)),
                    None => {
                        issues.push(module, loc, format!("`key.column` `{}` non esiste in `{table_name}`", k.column))
                    }
                }
            }
            (None, true) => issues.push(module, loc, "questo kind richiede `key = { param, column }`"),
            (Some(_), false) => issues.push(module, loc, "`key` non è ammesso su questo kind"),
            (None, false) => {}
        }

        let has_request = matches!(r.kind, RouteKind::Create | RouteKind::Update);
        match (&r.request, has_request) {
            (None, true) => issues.push(module, loc, "questo kind richiede `request`"),
            (Some(_), false) => issues.push(module, loc, "`request` non è ammesso su questo kind"),
            _ => {}
        }
        let has_response = r.kind != RouteKind::Delete;
        match (&r.response, has_response) {
            (None, true) => issues.push(module, loc, "questo kind richiede `response`"),
            (Some(_), false) => issues.push(module, loc, "`response` non è ammesso su `delete` (risposta 204)"),
            _ => {}
        }
        if r.kind != RouteKind::List
            && let Some(resp) = &r.response
            && type_ref(resp).1
        {
            issues.push(module, loc, "solo `list` può rispondere con un array");
        }
        if r.kind == RouteKind::Delete && r.status.is_some_and(|s| s != 204) {
            issues.push(module, loc, "`delete` risponde sempre 204");
        }

        // campi della risposta: colonne esistenti, tipo e nullabilità coerenti
        if let Some(resp) = r.response.as_deref().map(|t| type_ref(t).0)
            && let Some(fields) = self.types.get(resp)
        {
            for (fname, f) in fields {
                match table.column(fname) {
                    None => issues.push(
                        module,
                        loc,
                        format!("risposta `{resp}.{fname}`: colonna inesistente in `{table_name}`"),
                    ),
                    Some(col) => {
                        check_compat(module, loc, &format!("{resp}.{fname}"), f, &col.ty, false, issues);
                        if f.nullable != col.null {
                            issues.push(
                                module,
                                loc,
                                format!("risposta `{resp}.{fname}`: `nullable` non coincide con la colonna"),
                            );
                        }
                    }
                }
            }
        }
        // campi della richiesta
        if let Some(req) = r.request.as_deref().map(|t| type_ref(t).0)
            && let Some(fields) = self.types.get(req)
        {
            let scope_col = r.scope.as_ref().map(|_| "blog_id");
            for (fname, f) in fields {
                match table.column(fname) {
                    None => issues.push(
                        module,
                        loc,
                        format!("richiesta `{req}.{fname}`: colonna inesistente in `{table_name}`"),
                    ),
                    Some(col) => {
                        check_compat(module, loc, &format!("{req}.{fname}"), f, &col.ty, true, issues);
                        if Some(fname.as_str()) == scope_col || table.primary_key.contains(fname) {
                            issues.push(
                                module,
                                loc,
                                format!(
                                    "richiesta `{req}.{fname}`: colonna gestita dal sistema (scope/chiave primaria)"
                                ),
                            );
                        }
                        if f.nullable && !col.null {
                            issues.push(
                                module,
                                loc,
                                format!("richiesta `{req}.{fname}`: `nullable` ma la colonna è NOT NULL"),
                            );
                        }
                        if r.kind == RouteKind::Update && !f.optional {
                            issues.push(
                                module,
                                loc,
                                format!(
                                    "richiesta `{req}.{fname}`: in un update tutti i campi devono essere `optional`"
                                ),
                            );
                        }
                        if r.kind == RouteKind::Create && f.optional && !col.null && col.default.is_none() {
                            issues.push(
                                module,
                                loc,
                                format!("richiesta `{req}.{fname}`: `optional` ma la colonna è NOT NULL senza default"),
                            );
                        }
                    }
                }
            }
            if r.kind == RouteKind::Create {
                for col in &table.columns {
                    let managed = Some(col.name.as_str()) == scope_col;
                    if !managed && !col.null && col.default.is_none() && !fields.contains_key(&col.name) {
                        issues.push(
                            module,
                            loc,
                            format!("richiesta `{req}`: manca il campo per la colonna obbligatoria `{}`", col.name),
                        );
                    }
                }
            }
        }
    }
}

fn check_compat(module: &str, loc: &str, what: &str, f: &Field, col: &ColType, is_request: bool, issues: &mut Issues) {
    let ok = matches!(
        (f.field_type(), col),
        (FieldType::String, ColType::Text | ColType::Varchar(_))
            | (FieldType::Uuid, ColType::Uuid)
            | (FieldType::Int, ColType::Integer | ColType::BigInt)
            | (FieldType::Bool, ColType::Boolean)
            | (FieldType::Timestamp, ColType::Timestamptz)
            | (FieldType::Json, ColType::Jsonb)
    ) || matches!((f.field_type(), col), (FieldType::Enum(a), ColType::Enum(b)) if a == b);
    if !ok {
        issues.push(module, loc, format!("`{what}`: tipo incompatibile con la colonna ({col})"));
    }
    if let (true, FieldType::String, ColType::Varchar(n)) = (is_request, f.field_type(), col)
        && f.max.is_none_or(|m| m > i64::from(*n))
    {
        issues.push(module, loc, format!("`{what}`: la colonna è varchar({n}), dichiarare `max <= {n}`"));
    }
}

/// `30s`, `5m`, `1h`, `1d` → secondi.
pub fn parse_duration(s: &str) -> Option<u64> {
    let (num, unit) = s.split_at(s.len().checked_sub(1)?);
    let n: u64 = num.parse().ok()?;
    let mult = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86400,
        _ => return None,
    };
    (n > 0).then_some(n * mult)
}
