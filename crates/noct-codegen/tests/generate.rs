use noct_codegen::generate;
use std::{fs, path::PathBuf};

struct Dir(PathBuf);

impl Dir {
    fn new(files: &[(&str, &str)]) -> Dir {
        let dir = std::env::temp_dir().join(format!(
            "noct-codegen-test-{}-{}",
            std::process::id(),
            files.len() as u128 + rand_suffix()
        ));
        fs::create_dir_all(&dir).unwrap();
        for (name, content) in files {
            let path = dir.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        Dir(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn rand_suffix() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
}

const MODULE: &str = "[module]\nname='demo'\nversion='0.1.0'\nkind='optional'\n";
const DB: &str = r#"
[[table]]
name = "items"
columns = [
  { name = "id", type = "uuid", pk = true, default = "gen_random_uuid()" },
  { name = "label", type = "varchar(20)" },
]
[[table]]
name = "others"
columns = [
  { name = "id", type = "uuid", pk = true, default = "gen_random_uuid()" },
  { name = "label", type = "varchar(20)" },
]
"#;

#[test]
fn minimal_module_generates_module_fn_and_dto() {
    let api = r#"
[type.Item]
id = "uuid"
label = "string"
[[route]]
id = "list_items"
method = "GET"
path = "/items"
auth = "public"
kind = "list"
table = "items"
response = "Item[]"
"#;
    let d = Dir::new(&[("module.toml", MODULE), ("db.toml", DB), ("api.toml", api)]);
    let src = generate(&d.0).unwrap();
    assert!(src.contains("pub fn module() -> noct_core::ModuleDef") && src.contains("pub use generated::*;"));
    assert!(src.contains("pub struct Item"));
    assert!(src.contains("async fn __h_list_items"));
    assert!(src.contains(r#".route("/items", axum::routing::get(__h_list_items))"#));
    assert!(src.contains("api_toml: Some("), "api.toml incorporato");
    assert!(src.contains("lock_json: None"));
}

#[test]
fn module_without_api_or_db_is_valid() {
    let d = Dir::new(&[("module.toml", MODULE)]);
    let src = generate(&d.0).unwrap();
    assert!(src.contains("api_toml: None") && src.contains("db_toml: None"));
}

#[test]
fn manifest_errors_are_reported_together() {
    let d = Dir::new(&[("module.toml", MODULE), ("db.toml", "[[table]]\nname='Bad'\ncolumns=[]\n")]);
    let err = generate(&d.0).unwrap_err();
    assert!(err.contains("nome tabella non valido") && err.contains("manca la chiave primaria"), "{err}");
}

#[test]
fn missing_module_toml_is_an_error() {
    let d = Dir::new(&[("db.toml", DB)]);
    assert!(generate(&d.0).unwrap_err().contains("manca module.toml"));
}

#[test]
fn migration_files_must_be_named_and_numbered_correctly() {
    let d = Dir::new(&[("module.toml", MODULE), ("migrations/init.sql", "")]);
    assert!(generate(&d.0).unwrap_err().contains("nome non valido"));

    let d = Dir::new(&[("module.toml", MODULE), ("migrations/0002_second.sql", "SELECT 1;")]);
    assert!(generate(&d.0).unwrap_err().contains("consecutive"));

    let d = Dir::new(&[
        ("module.toml", MODULE),
        ("migrations/0001_a.sql", "SELECT 1;"),
        ("migrations/0002_b.sql", "SELECT 2;"),
    ]);
    let src = generate(&d.0).unwrap();
    assert!(src.contains("version: 1, name: \"a\"") && src.contains("version: 2, name: \"b\""));
}

#[test]
fn a_type_cannot_serve_two_tables() {
    let api = r#"
[type.Item]
id = "uuid"
label = "string"
[[route]]
id = "a"
method = "GET"
path = "/a"
auth = "public"
kind = "list"
table = "items"
response = "Item[]"
[[route]]
id = "b"
method = "GET"
path = "/b"
auth = "public"
kind = "list"
table = "others"
response = "Item[]"
"#;
    let d = Dir::new(&[("module.toml", MODULE), ("db.toml", DB), ("api.toml", api)]);
    assert!(generate(&d.0).unwrap_err().contains("tabelle diverse"));
}
