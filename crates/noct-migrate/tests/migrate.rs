use noct_manifest::{Manifest, Schema, Sources};
use noct_migrate::*;
use sqlx::{PgPool, postgres::PgPoolOptions};

fn schema(db: &str) -> Schema {
    let m = Manifest::parse(Sources {
        module: "[module]\nname='t'\nversion='0.1.0'\nkind='optional'\n",
        db: Some(db),
        ..Default::default()
    })
    .unwrap_or_else(|e| panic!("{e}"));
    m.schema
}

const BASE: &str = r#"
[[enum]]
name = "t_kind"
values = ["a", "b"]

[[table]]
name = "t_parent"
columns = [{ name = "id", type = "uuid", pk = true, default = "gen_random_uuid()" }]

[[table]]
name = "t_child"
timestamps = true
columns = [
  { name = "id", type = "uuid", pk = true, default = "gen_random_uuid()" },
  { name = "parent_id", type = "uuid", ref = "t_parent.id", on_delete = "cascade" },
  { name = "title", type = "varchar(80)" },
  { name = "kind", type = "enum:t_kind", default = "'a'" },
]
indexes = [{ columns = ["parent_id"] }]
uniques = [{ name = "uq_child_title", columns = ["parent_id", "title"] }]
"#;

#[test]
fn initial_diff_creates_enum_then_tables_in_fk_order() {
    // t_child dichiarata PRIMA di t_parent nel file: l'ordine deve seguire le FK
    let reordered = BASE.replace("[[table]]\nname = \"t_parent\"\ncolumns = [{ name = \"id\", type = \"uuid\", pk = true, default = \"gen_random_uuid()\" }]\n", "")
        + "\n[[table]]\nname = \"t_parent\"\ncolumns = [{ name = \"id\", type = \"uuid\", pk = true, default = \"gen_random_uuid()\" }]\n";
    let plan = diff(&Schema::default(), &schema(&reordered));
    assert!(plan.blocked.is_empty(), "{:?}", plan.blocked);
    let sql: Vec<&str> = plan.steps.iter().map(|s| s.sql.as_str()).collect();
    assert!(sql[0].starts_with("CREATE TYPE \"t_kind\""));
    let parent = sql.iter().position(|s| s.contains("CREATE TABLE \"t_parent\"")).unwrap();
    let child = sql.iter().position(|s| s.contains("CREATE TABLE \"t_child\"")).unwrap();
    assert!(parent < child);
    assert!(sql[child].contains("REFERENCES \"t_parent\" (\"id\") ON DELETE CASCADE"));
    assert!(sql[child].contains("CREATE INDEX \"ix_t_child_parent_id\""));
    assert!(sql[child].contains("CONSTRAINT \"uq_child_title\" UNIQUE"));
}

#[test]
fn identical_schemas_produce_an_empty_plan() {
    let s = schema(BASE);
    assert!(diff(&s, &s).is_empty());
}

#[test]
fn additive_changes_are_generated() {
    let old = schema(BASE);
    let new = schema(
        &BASE
            .replace("values = [\"a\", \"b\"]", "values = [\"a\", \"b\", \"c\"]")
            .replace(
                "  { name = \"kind\", type = \"enum:t_kind\", default = \"'a'\" },",
                "  { name = \"kind\", type = \"enum:t_kind\", default = \"'a'\" },\n  { name = \"note\", type = \"text\", null = true },\n  { name = \"pos\", type = \"integer\", default = \"0\" },",
            )
            .replace("indexes = [{ columns = [\"parent_id\"] }]", "indexes = [{ columns = [\"parent_id\"] }, { columns = [\"kind\"] }]"),
    );
    let plan = diff(&old, &new);
    assert!(plan.blocked.is_empty(), "{:?}", plan.blocked);
    let all = plan.steps.iter().map(|s| s.sql.clone()).collect::<Vec<_>>().join("\n");
    assert!(all.contains("ALTER TYPE \"t_kind\" ADD VALUE IF NOT EXISTS 'c'"));
    assert!(all.contains("ADD COLUMN \"note\" text;"));
    assert!(all.contains("ADD COLUMN \"pos\" integer NOT NULL DEFAULT 0;"));
    assert!(all.contains("CREATE INDEX \"ix_t_child_kind\""));
}

#[test]
fn rename_is_emitted_when_declared() {
    let old = schema(BASE);
    let new = schema(
        &BASE
            .replace(
                "{ name = \"title\", type = \"varchar(80)\" }",
                "{ name = \"heading\", type = \"varchar(80)\", renamed_from = \"title\" }",
            )
            .replace("columns = [\"parent_id\", \"title\"]", "columns = [\"parent_id\", \"heading\"]"),
    );
    let plan = diff(&old, &new);
    assert!(
        plan.steps.iter().any(|s| s.sql == "ALTER TABLE \"t_child\" RENAME COLUMN \"title\" TO \"heading\";"),
        "{:?}",
        plan
    );
    // senza `renamed_from` lo stesso cambiamento sarebbe drop + add: bloccato
    let no_hint = schema(&BASE.replace("\"title\"", "\"heading\""));
    assert!(!diff(&old, &no_hint).blocked.is_empty());
}

#[test]
fn destructive_changes_are_blocked_with_a_reason() {
    let old = schema(BASE);
    let cases: Vec<(String, &str)> = vec![
        (BASE.replace("type = \"varchar(80)\"", "type = \"text\""), "cambio di tipo"),
        (BASE.replace("values = [\"a\", \"b\"]", "values = [\"a\"]"), "rimozione del valore"),
        (BASE.replace("  { name = \"kind\", type = \"enum:t_kind\", default = \"'a'\" },\n", ""), "DROP COLUMN"),
        (BASE.replace(", on_delete = \"cascade\"", ""), "foreign key"),
        (
            BASE.replace(
                "  { name = \"kind\", type = \"enum:t_kind\", default = \"'a'\" },",
                "  { name = \"kind\", type = \"enum:t_kind\", default = \"'a'\" },\n  { name = \"must\", type = \"text\" },",
            ),
            "NOT NULL senza default",
        ),
    ];
    for (db, needle) in cases {
        let plan = diff(&old, &schema(&db));
        assert!(plan.blocked.iter().any(|b| b.contains(needle)), "atteso `{needle}`, trovato {:?}", plan.blocked);
    }
    // tabella rimossa
    let only_parent =
        schema("[[table]]\nname = \"t_parent\"\ncolumns = [{ name = \"id\", type = \"uuid\", pk = true }]\n");
    assert!(diff(&old, &only_parent).blocked.iter().any(|b| b.contains("DROP TABLE")));
}

#[test]
fn migration_file_names() {
    assert_eq!(parse_file_name("0003_add_x.sql"), Some((3, "add_x".into())));
    assert_eq!(parse_file_name("3_add_x.sql"), None);
    assert_eq!(parse_file_name("0000_zero.sql"), None);
    assert_eq!(parse_file_name("0001_Bad.sql"), None);
    assert_eq!(parse_file_name("0001_x.txt"), None);
}

// ---- contro un Postgres reale ---------------------------------------------------

async fn test_pool() -> (PgPool, String, PgPool) {
    let admin_url = std::env::var("NOCT_TEST_DATABASE_URL")
        .expect("NOCT_TEST_DATABASE_URL non impostata (es. postgres://postgres@127.0.0.1:55432/postgres)");
    let admin = PgPoolOptions::new().max_connections(2).connect(&admin_url).await.unwrap();
    let name = format!("noct_t_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE DATABASE {name}")).execute(&admin).await.unwrap();
    let url = format!("{}/{name}", admin_url.rsplit_once('/').unwrap().0);
    let pool = PgPoolOptions::new().max_connections(4).connect(&url).await.unwrap();
    (pool, name, admin)
}

async fn drop_db(pool: PgPool, name: String, admin: PgPool) {
    pool.close().await;
    sqlx::query(&format!("DROP DATABASE {name} WITH (FORCE)")).execute(&admin).await.unwrap();
}

#[tokio::test]
async fn generated_sql_applies_and_matches_the_manifest() {
    let (pool, name, admin) = test_pool().await;
    let target = schema(BASE);
    let plan = diff(&Schema::default(), &target);
    let sql = render_file("t", 1, &plan);

    let migrations = [Migration { version: 1, name: "init", sql: &sql }];
    let sets = [ModuleMigrations { module: "t", migrations: &migrations }];
    let done = apply(&pool, &sets).await.unwrap();
    assert_eq!(done.len(), 1);
    assert!(verify_live(&pool, &target).await.unwrap().is_empty());

    // idempotente: la seconda esecuzione non applica nulla
    assert!(apply(&pool, &sets).await.unwrap().is_empty());
    let st = status(&pool, &sets).await.unwrap();
    assert_eq!(st[0].status, Status::Applied);

    // il vincolo e il default funzionano davvero
    sqlx::query("INSERT INTO t_parent DEFAULT VALUES").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO t_child (parent_id, title) SELECT id, 'x' FROM t_parent").execute(&pool).await.unwrap();
    let dup = sqlx::query("INSERT INTO t_child (parent_id, title) SELECT id, 'x' FROM t_parent").execute(&pool).await;
    assert!(dup.is_err(), "il vincolo unique deve scattare");

    // evoluzione additiva: seconda migrazione generata dal diff e applicata
    let evolved = schema(&BASE.replace(
        "  { name = \"kind\", type = \"enum:t_kind\", default = \"'a'\" },",
        "  { name = \"kind\", type = \"enum:t_kind\", default = \"'a'\" },\n  { name = \"note\", type = \"text\", null = true },",
    ));
    let sql2 = render_file("t", 2, &diff(&target, &evolved));
    let migrations2 = [migrations[0], Migration { version: 2, name: "add_note", sql: &sql2 }];
    let sets2 = [ModuleMigrations { module: "t", migrations: &migrations2 }];
    assert_eq!(apply(&pool, &sets2).await.unwrap().len(), 1);
    assert!(verify_live(&pool, &evolved).await.unwrap().is_empty());
    // la riga esistente sopravvive e la nuova colonna è NULL
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM t_child WHERE note IS NULL").fetch_one(&pool).await.unwrap();
    assert_eq!(n, 1);

    drop_db(pool, name, admin).await;
}

#[tokio::test]
async fn modified_applied_file_is_rejected() {
    let (pool, name, admin) = test_pool().await;
    let m = [Migration { version: 1, name: "init", sql: "CREATE TABLE d1 (id int PRIMARY KEY);" }];
    apply(&pool, &[ModuleMigrations { module: "d", migrations: &m }]).await.unwrap();

    let tampered = [Migration { version: 1, name: "init", sql: "CREATE TABLE d1 (id int PRIMARY KEY, x int);" }];
    let err = apply(&pool, &[ModuleMigrations { module: "d", migrations: &tampered }]).await.unwrap_err();
    assert!(matches!(err, MigrationError::Drift { version: 1, .. }), "{err}");
    let st = status(&pool, &[ModuleMigrations { module: "d", migrations: &tampered }]).await.unwrap();
    assert_eq!(st[0].status, Status::Drifted);

    drop_db(pool, name, admin).await;
}

#[tokio::test]
async fn binary_older_than_database_is_rejected() {
    let (pool, name, admin) = test_pool().await;
    let two = [
        Migration { version: 1, name: "a", sql: "CREATE TABLE e1 (id int PRIMARY KEY);" },
        Migration { version: 2, name: "b", sql: "CREATE TABLE e2 (id int PRIMARY KEY);" },
    ];
    apply(&pool, &[ModuleMigrations { module: "e", migrations: &two }]).await.unwrap();
    let err = apply(&pool, &[ModuleMigrations { module: "e", migrations: &two[..1] }]).await.unwrap_err();
    assert!(matches!(err, MigrationError::DatabaseAhead { db_version: 2, binary_version: 1, .. }), "{err}");
    drop_db(pool, name, admin).await;
}

#[tokio::test]
async fn failing_migration_rolls_back_and_is_not_recorded() {
    let (pool, name, admin) = test_pool().await;
    let bad = [Migration {
        version: 1,
        name: "bad",
        sql: "CREATE TABLE f1 (id int PRIMARY KEY); SELECT * FROM does_not_exist;",
    }];
    let sets = [ModuleMigrations { module: "f", migrations: &bad }];
    let err = apply(&pool, &sets).await.unwrap_err();
    assert!(matches!(err, MigrationError::Sql { version: 1, .. }), "{err}");
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('f1') IS NOT NULL").fetch_one(&pool).await.unwrap();
    assert!(!exists, "la transazione doveva essere annullata");
    assert_eq!(status(&pool, &sets).await.unwrap()[0].status, Status::Pending);
    drop_db(pool, name, admin).await;
}

#[tokio::test]
async fn non_consecutive_versions_are_invalid() {
    let (pool, name, admin) = test_pool().await;
    let m = [Migration { version: 2, name: "skip", sql: "SELECT 1;" }];
    let err = apply(&pool, &[ModuleMigrations { module: "g", migrations: &m }]).await.unwrap_err();
    assert!(matches!(err, MigrationError::InvalidSet { .. }), "{err}");
    drop_db(pool, name, admin).await;
}

#[tokio::test]
async fn verify_live_reports_missing_pieces() {
    let (pool, name, admin) = test_pool().await;
    let problems = verify_live(&pool, &schema(BASE)).await.unwrap();
    assert!(problems.iter().any(|p| p.contains("enum `t_kind` assente")));
    assert!(problems.iter().any(|p| p.contains("tabella `t_child` assente")));
    drop_db(pool, name, admin).await;
}
