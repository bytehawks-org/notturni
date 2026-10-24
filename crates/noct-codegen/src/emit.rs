use crate::types::{Usage, column, field_type, rust_ident, usages};
use noct_manifest::{
    Auth, ColType, Field, FieldType, Manifest, Method, RateKey, Route, RouteKind, Schema, Table, parse_duration,
    type_ref,
};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Letterale stringa Rust (con escape) per un testo qualsiasi.
fn lit(s: &str) -> String {
    format!("{s:?}")
}

fn q(ident: &str) -> String {
    format!("\"{ident}\"")
}

pub fn emit(
    m: &Manifest,
    has_api: bool,
    has_db: bool,
    has_lock: bool,
    migrations: &[(u32, String, String)],
) -> Result<String, String> {
    let mut out = String::new();
    let usage_map = usages(&m.api)?;
    let name = m.name();

    writeln!(out, "// @generated da noct-codegen: non modificare (sorgenti: module.toml, api.toml, db.toml).").unwrap();
    writeln!(out, "pub const MODULE_NAME: &str = {};\n", lit(name)).unwrap();

    // --- sorgenti incorporati ---------------------------------------------------
    let inc = |file: &str| format!("include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{file}\"))");
    writeln!(out, "const __MIGRATIONS: &[noct_core::Migration<'static>] = &[").unwrap();
    for (v, n, file) in migrations {
        writeln!(
            out,
            "    noct_core::Migration {{ version: {v}, name: {}, sql: {} }},",
            lit(n),
            inc(&format!("migrations/{file}"))
        )
        .unwrap();
    }
    writeln!(out, "];\n").unwrap();
    let opt = |present: bool, file: &str| if present { format!("Some({})", inc(file)) } else { "None".to_string() };
    writeln!(
        out,
        "pub fn module() -> noct_core::ModuleDef {{\n    noct_core::ModuleDef {{\n        name: MODULE_NAME,\n        module_toml: {},\n        api_toml: {},\n        db_toml: {},\n        lock_json: {},\n        migrations: __MIGRATIONS,\n        router: __router,\n        source_dir: env!(\"CARGO_MANIFEST_DIR\"),\n    }}\n}}\n",
        inc("module.toml"),
        opt(has_api, "api.toml"),
        opt(has_db, "db.toml"),
        opt(has_lock, "schema.lock.json"),
    )
    .unwrap();

    // --- DTO -----------------------------------------------------------------------
    for (tname, fields) in &m.api.types {
        let mut usage = usage_map.get(tname).cloned().unwrap_or_default();
        if !usage.request && !usage.response {
            usage.request = true;
            usage.response = true;
        }
        emit_type(&mut out, &m.schema, tname, fields, &usage);
    }

    // --- rotte -----------------------------------------------------------------------
    let mut by_path: BTreeMap<&str, Vec<&Route>> = BTreeMap::new();
    for r in &m.api.routes {
        emit_route(&mut out, m, r)?;
        by_path.entry(&r.path).or_default().push(r);
    }
    writeln!(out, "fn __router() -> axum::Router<noct_core::AppState> {{\n    axum::Router::new()").unwrap();
    for (path, routes) in &by_path {
        let mut chain = String::new();
        for (i, r) in routes.iter().enumerate() {
            let method = match r.method {
                Method::Get => "get",
                Method::Post => "post",
                Method::Put => "put",
                Method::Patch => "patch",
                Method::Delete => "delete",
            };
            if i == 0 {
                write!(chain, "axum::routing::{method}(__h_{})", r.id).unwrap();
            } else {
                write!(chain, ".{method}(__h_{})", r.id).unwrap();
            }
        }
        writeln!(out, "        .route({}, {chain})", lit(path)).unwrap();
    }
    writeln!(out, "}}").unwrap();
    // il codice generato non deve sottostare ai lint stilistici del crate che lo include
    Ok(format!(
        "#[allow(clippy::all, clippy::pedantic, dead_code)]\npub mod generated {{\n{out}\n}}\npub use generated::*;\n"
    ))
}

// ---- tipi ---------------------------------------------------------------------------

fn emit_type(out: &mut String, schema: &Schema, tname: &str, fields: &BTreeMap<String, Field>, usage: &Usage) {
    let mut derives = vec!["Debug", "Clone"];
    if usage.request {
        derives.push("serde::Deserialize");
    }
    if usage.response {
        derives.push("serde::Serialize");
        derives.push("sqlx::FromRow");
    }
    writeln!(out, "#[derive({})]\npub struct {tname} {{", derives.join(", ")).unwrap();
    for (fname, f) in fields {
        let col = column(schema, usage, fname);
        let ty = field_type(f, col, usage);
        if ty.starts_with("Option<") || ty.starts_with("noct_core::Patch<") {
            writeln!(out, "    #[serde(default)]").unwrap();
        }
        writeln!(out, "    pub {}: {ty},", rust_ident(fname)).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    if !usage.request {
        return;
    }
    writeln!(out, "impl {tname} {{\n    /// Vincoli dichiarati in api.toml.\n    pub fn validate(&self) -> Result<(), noct_core::ApiError> {{").unwrap();
    for (fname, f) in fields {
        let checks = field_checks(schema, fname, f);
        if checks.is_empty() {
            continue;
        }
        let id = rust_ident(fname);
        let col = column(schema, usage, fname);
        let is_patch = usage.update_request && f.nullable;
        let is_opt = (usage.update_request && !f.nullable) || (!usage.update_request && (f.optional || f.nullable));
        let _ = col;
        if is_patch {
            writeln!(out, "        if let noct_core::Patch::Value(v) = &self.{id} {{\n{checks}        }}").unwrap();
        } else if is_opt {
            writeln!(out, "        if let Some(v) = &self.{id} {{\n{checks}        }}").unwrap();
        } else {
            writeln!(out, "        {{\n            let v = &self.{id};\n{checks}        }}").unwrap();
        }
    }
    writeln!(out, "        Ok(())\n    }}\n}}\n").unwrap();
}

fn field_checks(schema: &Schema, fname: &str, f: &Field) -> String {
    let mut s = String::new();
    let err = |msg: String| format!("return Err(noct_core::ApiError::bad_request({}));", lit(&msg));
    match f.field_type() {
        FieldType::String => {
            if f.non_blank {
                let msg = f.message.clone().unwrap_or_else(|| format!("Il campo `{fname}` non può essere vuoto."));
                writeln!(s, "            if v.trim().is_empty() {{ {} }}", err(msg)).unwrap();
            }
            if f.min.is_some() || f.max.is_some() {
                let msg = f.message.clone().unwrap_or_else(|| match (f.min, f.max) {
                    (Some(a), Some(b)) => format!("Il campo `{fname}` deve avere tra {a} e {b} caratteri."),
                    (Some(a), None) => format!("Il campo `{fname}` deve avere almeno {a} caratteri."),
                    (None, Some(b)) => format!("Il campo `{fname}` deve avere al massimo {b} caratteri."),
                    (None, None) => unreachable!(),
                });
                writeln!(s, "            let n = v.chars().count() as i64;").unwrap();
                if let Some(a) = f.min {
                    writeln!(s, "            if n < {a} {{ {} }}", err(msg.clone())).unwrap();
                }
                if let Some(b) = f.max {
                    writeln!(s, "            if n > {b} {{ {} }}", err(msg)).unwrap();
                }
            }
            if let Some(p) = &f.pattern {
                let msg = f.message.clone().unwrap_or_else(|| format!("Il campo `{fname}` non ha un formato valido."));
                writeln!(
                    s,
                    "            static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new({}).expect(\"pattern validato dal manifest\"));\n            if !RE.is_match(v) {{ {} }}",
                    lit(p),
                    err(msg)
                )
                .unwrap();
            }
        }
        FieldType::Int => {
            if f.min.is_some() || f.max.is_some() {
                let msg = f
                    .message
                    .clone()
                    .unwrap_or_else(|| format!("Il campo `{fname}` non è nell'intervallo consentito."));
                writeln!(s, "            let n = i64::from(*v);").unwrap();
                if let Some(a) = f.min {
                    writeln!(s, "            if n < {a} {{ {} }}", err(msg.clone())).unwrap();
                }
                if let Some(b) = f.max {
                    writeln!(s, "            if n > {b} {{ {} }}", err(msg)).unwrap();
                }
            }
        }
        FieldType::Enum(name) => {
            if let Some(e) = schema.enum_def(name) {
                let list = e.values.iter().map(|v| lit(v)).collect::<Vec<_>>().join(", ");
                let msg = f
                    .message
                    .clone()
                    .unwrap_or_else(|| format!("Il campo `{fname}` deve essere uno tra: {}.", e.values.join(", ")));
                writeln!(s, "            if ![{list}].contains(&v.as_str()) {{ {} }}", err(msg)).unwrap();
            }
        }
        _ => {}
    }
    s
}

// ---- rotte --------------------------------------------------------------------------

fn policy_expr(a: &Auth) -> String {
    match a {
        Auth::Public => "noct_core::Policy::Public".into(),
        Auth::User => "noct_core::Policy::User".into(),
        Auth::BlogWrite => "noct_core::Policy::BlogWrite".into(),
        Auth::PlatformRole(roles) => {
            format!(
                "noct_core::Policy::PlatformRole(&[{}])",
                roles.iter().map(|r| lit(r)).collect::<Vec<_>>().join(", ")
            )
        }
    }
}

fn select_list(table: &Table, fields: &BTreeMap<String, Field>) -> String {
    fields
        .keys()
        .map(|f| match table.column(f).map(|c| &c.ty) {
            Some(ColType::Enum(_)) => format!("{0}::text AS {0}", q(f)),
            _ => q(f),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn enum_cast(table: &Table, col: &str) -> Option<String> {
    match table.column(col).map(|c| &c.ty) {
        Some(ColType::Enum(e)) => Some(format!("::{}", q(e))),
        _ => None,
    }
}

fn emit_route(out: &mut String, m: &Manifest, r: &Route) -> Result<(), String> {
    let id = &r.id;
    let up = id.to_uppercase();
    let rate = match &r.rate_limit {
        None => "None".to_string(),
        Some(rl) => format!(
            "Some(noct_core::RateSpec {{ key: noct_core::RateKey::{}, max: {}, window_secs: {} }})",
            if rl.key == RateKey::Ip { "Ip" } else { "User" },
            rl.max,
            parse_duration(&rl.window).expect("validato")
        ),
    };
    writeln!(
        out,
        "const __R_{up}: noct_core::RouteSpec = noct_core::RouteSpec {{ module: MODULE_NAME, id: {}, policy: {}, rate: {rate} }};",
        lit(id),
        policy_expr(&r.auth)
    )
    .unwrap();

    let has_body = r.request.is_some();
    writeln!(out, "async fn __h_{id}(\n    axum::extract::State(st): axum::extract::State<noct_core::AppState>,\n    raw: axum::extract::RawPathParams,\n    headers: axum::http::HeaderMap,").unwrap();
    if has_body {
        writeln!(out, "    body: axum::body::Bytes,").unwrap();
    }
    writeln!(
        out,
        ") -> Result<axum::response::Response, noct_core::ApiError> {{\n    let path = noct_core::path_map(&raw);"
    )
    .unwrap();
    let slug = match &r.scope {
        Some(s) => format!("Some(path.get({}).expect(\"parametro di path\").as_str())", lit(&s.blog)),
        None => "None".to_string(),
    };
    writeln!(out, "    let access = noct_core::enforce(&st, &headers, &__R_{up}, {slug}).await?;").unwrap();

    if r.kind == RouteKind::Custom {
        let handler = r.handler.as_deref().expect("validato");
        writeln!(
            out,
            "    let ctx = noct_core::RouteCtx {{ state: st, actor: access.actor, blog: access.blog, path, module: MODULE_NAME }};"
        )
        .unwrap();
        let mut args = "ctx".to_string();
        if let Some(req) = &r.request {
            let ty = type_ref(req).0;
            writeln!(out, "    let body: {ty} = noct_core::parse_json(&body)?;\n    body.validate()?;").unwrap();
            args.push_str(", body");
        }
        match &r.response {
            Some(_) => writeln!(
                out,
                "    let out = crate::handlers::{handler}({args}).await?;\n    Ok(noct_core::json_response({}, &out))",
                r.success_status()
            )
            .unwrap(),
            None => writeln!(out, "    crate::handlers::{handler}({args}).await?;\n    Ok(noct_core::no_content())")
                .unwrap(),
        }
        writeln!(out, "}}\n").unwrap();
        return Ok(());
    }

    let table_name = r.table.as_deref().expect("validato");
    let table = m.schema.table(table_name).expect("validato");
    let scoped = r.scope.is_some();
    if scoped {
        writeln!(out, "    let blog = access.blog.as_ref().expect(\"scope risolto da enforce\");").unwrap();
    }
    let tq = q(table_name);
    let not_found = lit(r.not_found.as_deref().unwrap_or("Risorsa non trovata."));
    let conflicts = r
        .conflict
        .iter()
        .map(|c| format!("({}, {}, {})", lit(&c.constraint), c.status, lit(&c.message)))
        .collect::<Vec<_>>()
        .join(", ");
    let resp_type = r.response.as_deref().map(|t| type_ref(t).0.to_string());
    let resp_fields = resp_type.as_deref().and_then(|t| m.api.types.get(t));
    let cols = match resp_fields {
        Some(f) => select_list(table, f),
        None => String::new(),
    };
    let key_expr = |out: &mut String| {
        if let Some(k) = &r.key {
            writeln!(
                out,
                "    let key = noct_core::parse_uuid(path.get({}).expect(\"parametro di path\"))?;",
                lit(&k.param)
            )
            .unwrap();
        }
    };

    match r.kind {
        RouteKind::List => {
            let ty = resp_type.unwrap();
            let mut sql = format!("SELECT {cols} FROM {tq}");
            if scoped {
                sql.push_str(" WHERE \"blog_id\" = $1");
            }
            if !r.order_by.is_empty() {
                sql.push_str(&format!(" ORDER BY {}", r.order_by.iter().map(|c| q(c)).collect::<Vec<_>>().join(", ")));
            }
            let bind = if scoped { ".bind(blog.id)" } else { "" };
            writeln!(
                out,
                "    let rows: Vec<{ty}> = sqlx::query_as({}){bind}.fetch_all(&st.db).await?;\n    Ok(noct_core::json_response({}, &rows))",
                lit(&sql),
                r.success_status()
            )
            .unwrap();
        }
        RouteKind::Get => {
            key_expr(out);
            let ty = resp_type.unwrap();
            let k = r.key.as_ref().unwrap();
            let mut sql = format!("SELECT {cols} FROM {tq} WHERE {} = $1", q(&k.column));
            if scoped {
                sql.push_str(" AND \"blog_id\" = $2");
            }
            let bind = if scoped { ".bind(blog.id)" } else { "" };
            writeln!(
                out,
                "    let row: Option<{ty}> = sqlx::query_as({}).bind(key){bind}.fetch_optional(&st.db).await?;\n    match row {{\n        Some(r) => Ok(noct_core::json_response({}, &r)),\n        None => Err(noct_core::ApiError::not_found({not_found})),\n    }}",
                lit(&sql),
                r.success_status()
            )
            .unwrap();
        }
        RouteKind::Delete => {
            key_expr(out);
            let k = r.key.as_ref().unwrap();
            let mut sql = format!("DELETE FROM {tq} WHERE {} = $1", q(&k.column));
            if scoped {
                sql.push_str(" AND \"blog_id\" = $2");
            }
            let bind = if scoped { ".bind(blog.id)" } else { "" };
            writeln!(
                out,
                "    let done = sqlx::query({}).bind(key){bind}.execute(&st.db).await?;\n    if done.rows_affected() == 0 {{\n        return Err(noct_core::ApiError::not_found({not_found}));\n    }}\n    Ok(noct_core::no_content())",
                lit(&sql)
            )
            .unwrap();
        }
        RouteKind::Create => {
            let req_ty = type_ref(r.request.as_deref().unwrap()).0;
            let req_fields = &m.api.types[req_ty];
            let ty = resp_type.unwrap();
            writeln!(out, "    let body: {req_ty} = noct_core::parse_json(&body)?;\n    body.validate()?;").unwrap();
            writeln!(
                out,
                "    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new({});",
                lit(&format!("INSERT INTO {tq} ("))
            )
            .unwrap();
            writeln!(out, "    {{\n        let mut cols = qb.separated(\", \");").unwrap();
            if scoped {
                writeln!(out, "        cols.push({});", lit("\"blog_id\"")).unwrap();
            }
            for (fname, f) in req_fields {
                let id = rust_ident(fname);
                if f.optional || f.nullable {
                    writeln!(out, "        if body.{id}.is_some() {{ cols.push({}); }}", lit(&q(fname))).unwrap();
                } else {
                    writeln!(out, "        cols.push({});", lit(&q(fname))).unwrap();
                }
            }
            writeln!(out, "    }}\n    qb.push(\") VALUES (\");\n    {{\n        let mut vals = qb.separated(\", \");")
                .unwrap();
            if scoped {
                writeln!(out, "        vals.push_bind(blog.id);").unwrap();
            }
            for (fname, f) in req_fields {
                let id = rust_ident(fname);
                let cast = enum_cast(table, fname)
                    .map(|c| format!(" vals.push_unseparated({});", lit(&c)))
                    .unwrap_or_default();
                if f.optional || f.nullable {
                    writeln!(out, "        if let Some(v) = &body.{id} {{ vals.push_bind(v.clone());{cast} }}")
                        .unwrap();
                } else {
                    writeln!(out, "        vals.push_bind(body.{id}.clone());{cast}").unwrap();
                }
            }
            writeln!(out, "    }}").unwrap();
            writeln!(
                out,
                "    qb.push({});\n    let row: {ty} = qb.build_query_as::<{ty}>().fetch_one(&st.db).await.map_err(|e| noct_core::map_db_error(e, &[{conflicts}]))?;\n    Ok(noct_core::json_response({}, &row))",
                lit(&format!(") RETURNING {cols}")),
                r.success_status()
            )
            .unwrap();
        }
        RouteKind::Update => {
            key_expr(out);
            let req_ty = type_ref(r.request.as_deref().unwrap()).0;
            let req_fields = &m.api.types[req_ty];
            let ty = resp_type.unwrap();
            let k = r.key.as_ref().unwrap();
            writeln!(out, "    let body: {req_ty} = noct_core::parse_json(&body)?;\n    body.validate()?;").unwrap();
            writeln!(
                out,
                "    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new({});",
                lit(&format!("UPDATE {tq} SET "))
            )
            .unwrap();
            writeln!(out, "    let mut changed = 0usize;\n    {{\n        let mut set = qb.separated(\", \");")
                .unwrap();
            for (fname, f) in req_fields {
                let id = rust_ident(fname);
                let cast =
                    enum_cast(table, fname).map(|c| format!(" set.push_unseparated({});", lit(&c))).unwrap_or_default();
                let assign = lit(&format!("{} = ", q(fname)));
                if f.nullable {
                    writeln!(
                        out,
                        "        match &body.{id} {{\n            noct_core::Patch::Absent => {{}}\n            noct_core::Patch::Null => {{ set.push({}); changed += 1; }}\n            noct_core::Patch::Value(v) => {{ set.push({assign}); set.push_bind_unseparated(v.clone());{cast} changed += 1; }}\n        }}",
                        lit(&format!("{} = NULL", q(fname)))
                    )
                    .unwrap();
                } else {
                    writeln!(
                        out,
                        "        if let Some(v) = &body.{id} {{ set.push({assign}); set.push_bind_unseparated(v.clone());{cast} changed += 1; }}"
                    )
                    .unwrap();
                }
            }
            let has_updated_at = table.column("updated_at").is_some_and(|c| c.ty == ColType::Timestamptz);
            if has_updated_at {
                writeln!(out, "        if changed > 0 {{ set.push({}); }}", lit("\"updated_at\" = now()")).unwrap();
            }
            writeln!(out, "    }}").unwrap();
            // nessun campo inviato: risposta con lo stato corrente, senza scrivere
            let mut sel = format!("SELECT {cols} FROM {tq} WHERE {} = $1", q(&k.column));
            if scoped {
                sel.push_str(" AND \"blog_id\" = $2");
            }
            let bind = if scoped { ".bind(blog.id)" } else { "" };
            writeln!(
                out,
                "    if changed == 0 {{\n        let row: Option<{ty}> = sqlx::query_as({}).bind(key){bind}.fetch_optional(&st.db).await?;\n        return match row {{\n            Some(r) => Ok(noct_core::json_response({}, &r)),\n            None => Err(noct_core::ApiError::not_found({not_found})),\n        }};\n    }}",
                lit(&sel),
                r.success_status()
            )
            .unwrap();
            writeln!(out, "    qb.push({});\n    qb.push_bind(key);", lit(&format!(" WHERE {} = ", q(&k.column))))
                .unwrap();
            if scoped {
                writeln!(out, "    qb.push({});\n    qb.push_bind(blog.id);", lit(" AND \"blog_id\" = ")).unwrap();
            }
            writeln!(
                out,
                "    qb.push({});\n    let row: Option<{ty}> = qb.build_query_as::<{ty}>().fetch_optional(&st.db).await.map_err(|e| noct_core::map_db_error(e, &[{conflicts}]))?;\n    match row {{\n        Some(r) => Ok(noct_core::json_response({}, &r)),\n        None => Err(noct_core::ApiError::not_found({not_found})),\n    }}",
                lit(&format!(" RETURNING {cols}")),
                r.success_status()
            )
            .unwrap();
        }
        RouteKind::Custom => unreachable!(),
    }
    writeln!(out, "}}\n").unwrap();
    Ok(())
}
