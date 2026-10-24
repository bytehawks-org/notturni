use crate::{Auth, Field, FieldType, Manifest, Route, RouteKind, type_ref};
use serde_json::{Map, Value, json};

/// Documento OpenAPI 3.1 generato dai soli manifest (nessun codice da annotare).
pub fn openapi(manifests: &[&Manifest], title: &str, version: &str) -> Value {
    let mut paths: Map<String, Value> = Map::new();
    let mut schemas = Map::new();

    for m in manifests {
        for (tname, fields) in &m.api.types {
            let mut props = Map::new();
            let mut required = Vec::new();
            for (fname, f) in fields {
                props.insert(fname.clone(), field_schema(m, f));
                if !f.optional {
                    required.push(Value::String(fname.clone()));
                }
            }
            let mut obj = json!({ "type": "object", "properties": props });
            if !required.is_empty() {
                obj["required"] = Value::Array(required);
            }
            schemas.insert(format!("{}.{tname}", m.name()), obj);
        }
        for r in &m.api.routes {
            let entry = paths.entry(r.path.clone()).or_insert_with(|| json!({}));
            entry[r.method.as_str().to_ascii_lowercase()] = operation(m, r);
        }
    }

    json!({
        "openapi": "3.1.0",
        "info": { "title": title, "version": version },
        "servers": [{ "url": "/api/v1" }],
        "paths": paths,
        "components": {
            "schemas": schemas,
            "securitySchemes": { "bearerAuth": { "type": "http", "scheme": "bearer" } }
        }
    })
}

fn schema_ref(m: &Manifest, name: &str) -> Value {
    json!({ "$ref": format!("#/components/schemas/{}.{name}", m.name()) })
}

fn field_schema(m: &Manifest, f: &Field) -> Value {
    let mut s = match f.field_type() {
        FieldType::String => {
            let mut s = json!({ "type": "string" });
            if let Some(n) = f.min {
                s["minLength"] = n.into();
            }
            if let Some(n) = f.max {
                s["maxLength"] = n.into();
            }
            if let Some(p) = &f.pattern {
                s["pattern"] = p.clone().into();
            }
            s
        }
        FieldType::Uuid => json!({ "type": "string", "format": "uuid" }),
        FieldType::Int => {
            let mut s = json!({ "type": "integer" });
            if let Some(n) = f.min {
                s["minimum"] = n.into();
            }
            if let Some(n) = f.max {
                s["maximum"] = n.into();
            }
            s
        }
        FieldType::Bool => json!({ "type": "boolean" }),
        FieldType::Timestamp => json!({ "type": "string", "format": "date-time" }),
        FieldType::Json => json!({}),
        FieldType::Enum(name) => {
            let values = m.schema.enum_def(name).map(|e| e.values.clone()).unwrap_or_default();
            json!({ "type": "string", "enum": values })
        }
    };
    if f.nullable
        && let Some(t) = s.get("type").cloned()
    {
        s["type"] = json!([t, "null"]);
    }
    s
}

fn operation(m: &Manifest, r: &Route) -> Value {
    let mut op = Map::new();
    op.insert("operationId".into(), format!("{}_{}", m.name(), r.id).into());
    op.insert("tags".into(), json!([m.name()]));
    if let Some(s) = &r.summary {
        op.insert("summary".into(), s.clone().into());
    }
    let params: Vec<Value> = r
        .path_params()
        .into_iter()
        .map(|p| {
            let is_uuid = r.key.as_ref().is_some_and(|k| k.param == p);
            let schema =
                if is_uuid { json!({ "type": "string", "format": "uuid" }) } else { json!({ "type": "string" }) };
            json!({ "name": p, "in": "path", "required": true, "schema": schema })
        })
        .collect();
    if !params.is_empty() {
        op.insert("parameters".into(), Value::Array(params));
    }
    if let Some(req) = &r.request {
        op.insert(
            "requestBody".into(),
            json!({ "required": true, "content": { "application/json": { "schema": schema_ref(m, type_ref(req).0) } } }),
        );
    }
    let mut responses = Map::new();
    let status = r.success_status().to_string();
    match &r.response {
        Some(resp) => {
            let (base, array) = type_ref(resp);
            let schema =
                if array { json!({ "type": "array", "items": schema_ref(m, base) }) } else { schema_ref(m, base) };
            responses.insert(
                status,
                json!({ "description": "OK", "content": { "application/json": { "schema": schema } } }),
            );
        }
        None => {
            responses.insert(status, json!({ "description": "Nessun contenuto" }));
        }
    }
    let err = |d: &str| json!({ "description": d });
    if r.auth != Auth::Public {
        responses.insert("401".into(), err("Non autenticato"));
        responses.insert("403".into(), err("Non autorizzato"));
    }
    if r.scope.is_some() || r.key.is_some() {
        responses.insert("404".into(), err("Risorsa non trovata"));
    }
    if matches!(r.kind, RouteKind::Create | RouteKind::Update) || r.request.is_some() {
        responses.insert("400".into(), err("Dati non validi"));
        responses.insert("422".into(), err("Corpo della richiesta non interpretabile"));
    }
    for c in &r.conflict {
        responses.insert(c.status.to_string(), err(&c.message));
    }
    if r.rate_limit.is_some() {
        responses.insert("429".into(), err("Troppe richieste"));
    }
    op.insert("responses".into(), Value::Object(responses));
    match &r.auth {
        Auth::Public => {
            op.insert("security".into(), json!([{}, { "bearerAuth": [] }]));
        }
        _ => {
            op.insert("security".into(), json!([{ "bearerAuth": [] }]));
        }
    }
    Value::Object(op)
}
