use noct_manifest::*;

const USERS_MODULE: &str = r#"
[module]
name = "users"
version = "0.1.0"
kind = "core"
"#;
const USERS_DB: &str = r#"
[[table]]
name = "users"
timestamps = true
columns = [
  { name = "id", type = "uuid", pk = true, default = "gen_random_uuid()" },
  { name = "username", type = "varchar(32)" },
]
uniques = [{ name = "uq_users_username", columns = ["username"] }]
"#;

const BLOGS_MODULE: &str = r#"
[module]
name = "blogs"
version = "0.1.0"
kind = "core"
requires = ["users"]
"#;
const BLOGS_DB: &str = r#"
[[table]]
name = "blogs"
columns = [
  { name = "id", type = "uuid", pk = true, default = "gen_random_uuid()" },
  { name = "owner_id", type = "uuid", ref = "users.id" },
  { name = "slug", type = "varchar(63)" },
]
"#;

const TAX_MODULE: &str = r#"
[module]
name = "taxonomy"
version = "0.1.0"
kind = "optional"
requires = ["blogs"]
"#;
const TAX_DB: &str = r#"
[[table]]
name = "categories"
timestamps = true
columns = [
  { name = "id", type = "uuid", pk = true, default = "gen_random_uuid()" },
  { name = "blog_id", type = "uuid", ref = "blogs.id", on_delete = "cascade" },
  { name = "name", type = "varchar(50)" },
  { name = "slug", type = "varchar(60)" },
]
uniques = [{ name = "uq_category_blog_slug", columns = ["blog_id", "slug"] }]
"#;
const TAX_API: &str = r#"
[type.Category]
id = "uuid"
name = "string"
slug = "string"

[type.CategoryCreate]
name = { type = "string", max = 50, non_blank = true }
slug = { type = "string", max = 60, pattern = "^[a-z0-9]+(-[a-z0-9]+)*$" }

[type.CategoryUpdate]
name = { type = "string", max = 50, optional = true }
slug = { type = "string", max = 60, optional = true }

[[route]]
id = "list_categories"
method = "GET"
path = "/blogs/{slug}/categories"
auth = "public"
kind = "list"
table = "categories"
scope = { blog = "slug" }
order_by = ["name"]
response = "Category[]"

[[route]]
id = "create_category"
method = "POST"
path = "/blogs/{slug}/categories"
auth = "blog_write"
kind = "create"
table = "categories"
scope = { blog = "slug" }
request = "CategoryCreate"
response = "Category"
conflict = [{ constraint = "uq_category_blog_slug", status = 409, message = "dup" }]

[[route]]
id = "update_category"
method = "PATCH"
path = "/blogs/{slug}/categories/{category_id}"
auth = "blog_write"
kind = "update"
table = "categories"
scope = { blog = "slug" }
key = { param = "category_id", column = "id" }
request = "CategoryUpdate"
response = "Category"

[[route]]
id = "delete_category"
method = "DELETE"
path = "/blogs/{slug}/categories/{category_id}"
auth = "blog_write"
kind = "delete"
table = "categories"
scope = { blog = "slug" }
key = { param = "category_id", column = "id" }
"#;

fn users() -> Manifest {
    Manifest::parse(Sources { module: USERS_MODULE, db: Some(USERS_DB), ..Default::default() }).unwrap()
}
fn blogs() -> Manifest {
    Manifest::parse(Sources { module: BLOGS_MODULE, db: Some(BLOGS_DB), ..Default::default() }).unwrap()
}
fn taxonomy() -> Manifest {
    Manifest::parse(Sources { module: TAX_MODULE, db: Some(TAX_DB), api: Some(TAX_API), lock: None }).unwrap()
}
fn tax_with(api: &str) -> Result<Manifest, Issues> {
    Manifest::parse(Sources { module: TAX_MODULE, db: Some(TAX_DB), api: Some(api), lock: None })
}

#[test]
fn valid_modules_parse_and_register() {
    let (u, b, t) = (users(), blogs(), taxonomy());
    validate_registry(&[&u, &b, &t]).unwrap();
    assert_eq!(t.api.routes.len(), 4);
    // `timestamps = true` aggiunge le due colonne con default now()
    let cats = t.schema.table("categories").unwrap();
    assert_eq!(cats.column("created_at").unwrap().default.as_deref(), Some("now()"));
    assert!(cats.indexes.is_empty() && cats.uniques.len() == 1);
}

#[test]
fn topological_order_follows_requires() {
    let (u, b, t) = (users(), blogs(), taxonomy());
    // elenco volutamente in ordine inverso
    let order = topological_order(&[&t, &b, &u]).unwrap();
    assert_eq!(order, vec![2, 1, 0]);
}

#[test]
fn unknown_keys_are_rejected() {
    let err = Manifest::parse(Sources {
        module: "[module]\nname='x'\nversion='1.0.0'\nkind='core'\ntypo=1\n",
        ..Default::default()
    })
    .unwrap_err();
    assert!(err.contains("typo"), "{err}");
}

#[test]
fn db_validation_reports_all_problems() {
    let err = Manifest::parse(Sources {
        module: USERS_MODULE,
        db: Some(
            r#"
[[table]]
name = "Bad-Name"
columns = [{ name = "a", type = "float" }]
"#,
        ),
        ..Default::default()
    })
    .unwrap_err();
    assert!(err.contains("nome tabella non valido"));
    assert!(err.contains("tipo colonna sconosciuto"));
    assert!(err.contains("manca la chiave primaria"));
}

#[test]
fn route_references_are_checked() {
    let bad = TAX_API.replace(
        "table = \"categories\"\nscope = { blog = \"slug\" }\norder_by",
        "table = \"nope\"\nscope = { blog = \"slug\" }\norder_by",
    );
    assert!(tax_with(&bad).unwrap_err().contains("non è definita in db.toml"));

    let bad = TAX_API.replace("order_by = [\"name\"]", "order_by = [\"ghost\"]");
    assert!(tax_with(&bad).unwrap_err().contains("`order_by`: colonna inesistente"));

    let bad = TAX_API.replace("scope = { blog = \"slug\" }\nkey", "scope = { blog = \"nope\" }\nkey");
    assert!(tax_with(&bad).unwrap_err().contains("non è un parametro del path"));

    let bad = TAX_API.replace("constraint = \"uq_category_blog_slug\"", "constraint = \"uq_nothing\"");
    assert!(tax_with(&bad).unwrap_err().contains("non è un vincolo di unicità"));
}

#[test]
fn create_must_cover_required_columns() {
    // `CategoryCreate` senza `slug`: colonna NOT NULL senza default
    let bad = TAX_API.replace("slug = { type = \"string\", max = 60, pattern = \"^[a-z0-9]+(-[a-z0-9]+)*$\" }\n", "");
    assert!(tax_with(&bad).unwrap_err().contains("manca il campo per la colonna obbligatoria `slug`"));
}

#[test]
fn varchar_columns_require_a_max() {
    let bad = TAX_API.replace(
        "name = { type = \"string\", max = 50, non_blank = true }",
        "name = { type = \"string\", non_blank = true }",
    );
    assert!(tax_with(&bad).unwrap_err().contains("dichiarare `max <= 50`"));
}

#[test]
fn update_fields_must_be_optional() {
    let bad = TAX_API
        .replace("name = { type = \"string\", max = 50, optional = true }", "name = { type = \"string\", max = 50 }");
    assert!(tax_with(&bad).unwrap_err().contains("in un update tutti i campi devono essere `optional`"));
}

#[test]
fn blog_write_requires_scope() {
    let api = r#"
[type.Out]
id = "uuid"
[[route]]
id = "x"
method = "GET"
path = "/x"
auth = "blog_write"
kind = "custom"
handler = "x"
response = "Out"
"#;
    assert!(tax_with(api).unwrap_err().contains("richiede `scope"));
}

#[test]
fn custom_route_needs_handler_and_crud_forbids_it() {
    let api = r#"
[type.Out]
id = "uuid"
[[route]]
id = "x"
method = "GET"
path = "/x"
auth = "public"
kind = "custom"
response = "Out"
"#;
    assert!(tax_with(api).unwrap_err().contains("richiede `handler`"));
}

#[test]
fn registry_rejects_cycles_core_on_optional_and_foreign_fk() {
    let (u, b, t) = (users(), blogs(), taxonomy());

    // core che richiede un modulo opzionale
    let core_on_opt = Manifest::parse(Sources {
        module: "[module]\nname='blogs'\nversion='0.1.0'\nkind='core'\nrequires=['users','taxonomy']\n",
        db: Some(BLOGS_DB),
        ..Default::default()
    })
    .unwrap();
    let err = validate_registry(&[&u, &core_on_opt, &t]).unwrap_err();
    assert!(err.contains("non può dipendere dal modulo opzionale"), "{err}");
    assert!(err.contains("ciclo di dipendenze"), "{err}");

    // FK verso un modulo non dichiarato in `requires`
    let no_req = Manifest::parse(Sources {
        module: "[module]\nname='taxonomy'\nversion='0.1.0'\nkind='optional'\n",
        db: Some(TAX_DB),
        ..Default::default()
    })
    .unwrap();
    let err = validate_registry(&[&u, &b, &no_req]).unwrap_err();
    assert!(err.contains("non è in `requires`"), "{err}");
}

#[test]
fn registry_detects_route_and_table_collisions() {
    let (u, b, t) = (users(), blogs(), taxonomy());
    let clone = Manifest::parse(Sources {
        module: "[module]\nname='taxonomy2'\nversion='0.1.0'\nkind='optional'\nrequires=['blogs']\n",
        db: Some(TAX_DB),
        api: Some(TAX_API),
        lock: None,
    })
    .unwrap();
    let err = validate_registry(&[&u, &b, &t, &clone]).unwrap_err();
    assert!(err.contains("tabella già definita"), "{err}");
    assert!(err.contains("collide con una rotta"), "{err}");
}

#[test]
fn drift_between_db_toml_and_lock_is_detected() {
    let t = taxonomy();
    assert!(t.drift().is_some(), "senza lock lo schema è in drift");

    let lock = serde_json::to_string(&t.schema.canonical()).unwrap();
    let ok = Manifest::parse(Sources { module: TAX_MODULE, db: Some(TAX_DB), api: Some(TAX_API), lock: Some(&lock) })
        .unwrap();
    assert!(ok.drift().is_none());

    let changed = TAX_DB.replace("varchar(50)", "varchar(60)");
    let drifted =
        Manifest::parse(Sources { module: TAX_MODULE, db: Some(&changed), api: None, lock: Some(&lock) }).unwrap();
    assert!(drifted.drift().is_some());
}

#[test]
fn config_resolution_uses_env_then_default_and_reports_errors() {
    let m = Manifest::parse(Sources {
        module: r#"
[module]
name = "sandbox"
version = "0.1.0"
kind = "optional"
[config]
greeting = { type = "string", env = "NOCT_SANDBOX_GREETING", default = "hi" }
limit = { type = "int", env = "NOCT_SANDBOX_LIMIT" }
"#,
        ..Default::default()
    })
    .unwrap();
    let env = |k: &str| (k == "NOCT_SANDBOX_LIMIT").then(|| "7".to_string());
    let cfg = resolve_config(&m.module, &env).unwrap();
    assert_eq!(cfg["greeting"], ConfigValue::String("hi".into()));
    assert_eq!(cfg["limit"], ConfigValue::Int(7));

    let bad = |k: &str| (k == "NOCT_SANDBOX_LIMIT").then(|| "x".to_string());
    assert!(resolve_config(&m.module, &bad).unwrap_err().contains("non intero"));
    assert!(resolve_config(&m.module, &|_| None).unwrap_err().contains("nessun default"));
}

#[test]
fn openapi_describes_routes_schemas_and_security() {
    let t = taxonomy();
    let doc = openapi(&[&t], "Notturni", "0.1.0");
    assert_eq!(doc["openapi"], "3.1.0");
    let list = &doc["paths"]["/blogs/{slug}/categories"]["get"];
    assert_eq!(list["operationId"], "taxonomy_list_categories");
    assert_eq!(list["responses"]["200"]["content"]["application/json"]["schema"]["type"], "array");
    let create = &doc["paths"]["/blogs/{slug}/categories"]["post"];
    assert_eq!(create["responses"]["201"]["description"], "OK");
    assert!(create["responses"]["409"].is_object());
    assert_eq!(create["security"][0]["bearerAuth"], serde_json::json!([]));
    let del = &doc["paths"]["/blogs/{slug}/categories/{category_id}"]["delete"];
    assert!(del["responses"]["204"].is_object());
    let req = &doc["components"]["schemas"]["taxonomy.CategoryCreate"];
    assert_eq!(req["properties"]["slug"]["maxLength"], 60);
    assert_eq!(req["required"], serde_json::json!(["name", "slug"]));
}
