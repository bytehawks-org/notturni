//! Moduli di interazione con i post: note (`notes`), frammenti (`fragments`) e commenti
//! (`comments`).

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use uuid::Uuid;

const ALL: &[&str] = &["notes", "fragments", "comments"];

async fn world(modules: &[&str], env: &[(&str, &str)]) -> (Env, Uuid) {
    let env = setup(modules, env).await;
    let anna = env.user("anna", "utente").await;
    env.blog("diario", anna, "public").await;
    (env, anna)
}

async fn new_post(env: &Env, who: Uuid, slug: &str, extra: Value) -> Value {
    let mut body =
        json!({"slug": slug, "title": format!("T {slug}"), "content": "uno due tre quattro cinque sei sette"});
    body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    let (s, b) = env.post("/blogs/diario/posts", Some(who), body).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    b
}

fn id(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

async fn publish(env: &Env, who: Uuid, post: &Value) {
    let (s, b) = env.post(&format!("/posts/{}/publish", id(post)), Some(who), json!({})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

fn ids(list: &Value) -> Vec<String> {
    list.as_array().unwrap().iter().map(id).collect()
}

// ---- notes -------------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn post_notes_are_validated_linked_to_the_library_and_replaced() {
    let (env, anna) = world(ALL, &[]).await;
    let notes = json!([
        {"idx": 2, "content": "Zerby, C. The Devil's Details. 2002.", "kind": "book", "title": "The Devil's Details"},
        {"idx": 1, "content": "Vedi https://example.com/pagina."},
    ]);
    let p = new_post(&env, anna, "uno", json!({"notes": notes})).await;
    let n = p["notes"].as_array().unwrap();
    assert_eq!(
        (n[0]["idx"].clone(), n[1]["idx"].clone(), n[1]["title"].as_str()),
        (json!(1), json!(2), Some("The Devil's Details"))
    );

    // la libreria nasce dalle note: tipo e URL stimati se l'autore non li indica
    let (_, lib) = env.get("/blogs/diario/notes", Some(anna)).await;
    let lib = lib.as_array().unwrap();
    assert_eq!(lib.len(), 2);
    let web = lib.iter().find(|l| l["content"].as_str().unwrap().starts_with("Vedi")).unwrap();
    assert_eq!((web["kind"].as_str(), web["url"].as_str()), (Some("web"), Some("https://example.com/pagina")));
    let book = lib.iter().find(|l| l["kind"] == "book").unwrap();
    assert_eq!(book["title"], "The Devil's Details", "campi bibliografici riportati alla creazione");

    // stesso testo con maiuscole e apostrofi diversi → stessa nota di libreria, citata in due post
    let p2 =
        new_post(&env, anna, "due", json!({"notes": [{"idx": 1, "content": "zerby c. the devils   details. 2002."}]}))
            .await;
    let (_, lib) = env.get("/blogs/diario/notes", Some(anna)).await;
    assert_eq!(lib.as_array().unwrap().len(), 2, "nessun duplicato");
    let book = lib.as_array().unwrap().iter().find(|l| l["kind"] == "book").unwrap();
    let mut used: Vec<_> =
        book["used_in"].as_array().unwrap().iter().map(|u| u["post_slug"].as_str().unwrap()).collect();
    used.sort();
    assert_eq!(used, ["due", "uno"]);

    // note non valide annullano la richiesta: nessun post creato
    for bad in [
        json!([{"idx": 0, "content": "a"}]),
        json!([{"idx": 1, "content": "a"}, {"idx": 1, "content": "b"}]),
        json!([{"idx": 1, "content": "a", "url": "javascript:alert(1)"}]),
        json!([{"idx": 1, "content": "a", "kind": "film"}]),
    ] {
        let body = json!({"slug": "rotto", "title": "R", "content": "c", "notes": bad});
        assert_eq!(env.post("/blogs/diario/posts", Some(anna), body).await.0, StatusCode::BAD_REQUEST);
    }
    let n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM posts WHERE slug = 'rotto'").fetch_one(&env.pool).await.unwrap();
    assert_eq!(n, 0);

    // PATCH: assente o null = invariate, [] = le toglie
    let path = format!("/posts/{}", id(&p2));
    assert_eq!(env.patch(&path, Some(anna), json!({"title": "X"})).await.1["notes"].as_array().unwrap().len(), 1);
    assert_eq!(env.patch(&path, Some(anna), json!({"notes": null})).await.1["notes"].as_array().unwrap().len(), 1);
    assert_eq!(env.patch(&path, Some(anna), json!({"notes": []})).await.1["notes"], json!([]));
    // la nota di libreria resta, ma non è più citata
    let (_, lib) = env.get("/blogs/diario/notes", Some(anna)).await;
    let book = lib.as_array().unwrap().iter().find(|l| l["kind"] == "book").unwrap();
    assert_eq!(book["used_in"].as_array().unwrap().len(), 1);
    env.teardown().await;
}

#[tokio::test]
async fn concurrent_saves_do_not_duplicate_a_new_library_note() {
    let (env, anna) = world(ALL, &[]).await;
    let calls = (0..6).map(|i| {
        let body = json!({"slug": format!("p{i}"), "title": "T", "content": "c", "notes": [{"idx": 1, "content": "Stessa nota concorrente"}]});
        env.post("/blogs/diario/posts", Some(anna), body)
    });
    for (status, _) in futures::future::join_all(calls).await {
        assert_eq!(status, StatusCode::CREATED);
    }
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM blog_notes").fetch_one(&env.pool).await.unwrap();
    assert_eq!(n, 1, "un solo record di libreria per sei salvataggi concorrenti");
    env.teardown().await;
}

#[tokio::test]
async fn library_permissions_crud_merge_and_bibtex() {
    let (env, anna) = world(ALL, &[]).await;
    let base = "/blogs/diario/notes";
    let stranger = env.user("estraneo", "utente").await;
    let rita = env.user("rita", "utente").await;
    let blog: Uuid =
        sqlx::query_scalar("SELECT id FROM blogs WHERE slug = 'diario'").fetch_one(&env.pool).await.unwrap();
    env.member(blog, rita, "revisore").await;

    assert_eq!(env.get(base, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.get(base, Some(stranger)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(env.get(base, Some(rita)).await.0, StatusCode::OK, "ogni collaboratore legge");
    assert_eq!(
        env.post(base, Some(rita), json!({"content": "x"})).await.0,
        StatusCode::FORBIDDEN,
        "ma solo chi scrive modifica"
    );

    for bad in [
        json!({"content": "  "}),
        json!({"content": "x", "kind": "film"}),
        json!({"content": "x", "url": "ftp://x"}),
        json!({"content": "x", "isbn": "1".repeat(40)}),
    ] {
        assert_eq!(env.post(base, Some(anna), bad).await.0, StatusCode::BAD_REQUEST);
    }
    let (s, a) = env
        .post(
            base,
            Some(anna),
            json!({"content": "Rossi, M. Un libro sul mare e sulle onde. 2010.", "kind": "book", "author": "Rossi"}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{a}");
    let (_, b) = env
        .post(base, Some(anna), json!({"content": "Rossi, M. Un libro sul mare e sulle onde. 2010 (ristampa)."}))
        .await;
    let (_, c) = env.post(base, Some(anna), json!({"content": "Tutt'altra cosa"})).await;
    assert_eq!(b["kind"], "note");

    // duplicati e filtro
    let (_, lib) = env.get(base, Some(anna)).await;
    let by_id = |lib: &Value, x: &Value| lib.as_array().unwrap().iter().find(|l| l["id"] == x["id"]).cloned().unwrap();
    assert_eq!(by_id(&lib, &a)["possible_duplicates"], json!([b["id"]]));
    assert_eq!(by_id(&lib, &c)["possible_duplicates"], json!([]));
    let (_, f) = env.get(&format!("{base}?q=ALTRA"), Some(anna)).await;
    assert_eq!(ids(&f), [id(&c)]);

    // modifica: svuota un campo con "", il testo si propaga ai post
    let post = new_post(&env, anna, "uno", json!({"notes": [{"idx": 1, "content": "Tutt'altra cosa"}]})).await;
    let (s, u) = env
        .patch(
            &format!("{base}/{}", id(&c)),
            Some(anna),
            json!({"content": "Tutt'altra cosa, rivista", "author": "Neri"}),
        )
        .await;
    assert_eq!((s, u["author"].as_str()), (StatusCode::OK, Some("Neri")), "{u}");
    let (_, u) = env.patch(&format!("{base}/{}", id(&c)), Some(anna), json!({"author": ""})).await;
    assert_eq!(u["author"], Value::Null);
    let (_, p) = env.get(&format!("/posts/{}", id(&post)), Some(anna)).await;
    assert_eq!(p["notes"][0]["content"], "Tutt'altra cosa, rivista");
    assert_eq!(
        env.patch(&format!("{base}/{}", id(&c)), Some(anna), json!({"kind": "film"})).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        env.patch(&format!("{base}/{}", Uuid::new_v4()), Some(anna), json!({"kind": "web"})).await.0,
        StatusCode::NOT_FOUND
    );

    // una nota citata non si elimina; una libera sì
    let (s, e) = env.delete_json(&format!("{base}/{}", id(&c)), Some(anna), json!({})).await;
    assert_eq!(
        (s, detail(&e)),
        (StatusCode::CONFLICT, "La nota è citata in un post: rimuovila prima dal post o uniscila a un'altra.")
    );
    assert_eq!(env.delete(&format!("{base}/{}", id(&b)), Some(anna)).await, StatusCode::NO_CONTENT);

    // merge: il post passa alla nota di destinazione
    let merge = |from: &Value, into: &Value| (format!("{base}/{}/merge", id(from)), json!({"into_id": id(into)}));
    let (url, body) = merge(&c, &c);
    assert_eq!(env.post(&url, Some(anna), body).await.0, StatusCode::BAD_REQUEST);
    let (url, body) = merge(&c, &a);
    let (s, m) = env.post(&url, Some(anna), body).await;
    assert_eq!((s, m["used_in"].as_array().unwrap().len()), (StatusCode::OK, 1), "{m}");
    let (_, p) = env.get(&format!("/posts/{}", id(&post)), Some(anna)).await;
    assert_eq!(p["notes"][0]["content"], a["content"]);
    assert_eq!(
        env.patch(&format!("{base}/{}", id(&c)), Some(anna), json!({"kind": "web"})).await.0,
        StatusCode::NOT_FOUND,
        "la sorgente non esiste più"
    );

    // export e import BibTeX
    let (status, headers, bib) = env.get_text(&format!("{base}/export.bib"), Some(anna)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("content-type").unwrap(), "application/x-bibtex");
    assert!(headers.get("content-disposition").unwrap().to_str().unwrap().contains("diario.bib"));
    assert!(bib.contains("@book{notturni1,") && bib.contains("author = {Rossi}"), "{bib}");

    let import = "@article{k,\n  author = {Verdi},\n  title = {Sul fiume},\n  journal = {Rivista},\n  year = {2021},\n  doi = {10.1/abc},\n  url = {javascript:alert(1)}\n}\n\n\
                  @book{h,\n  note = {Rossi, M. Un libro sul mare e sulle onde. 2010.}\n}";
    let (s, created) = env.post(&format!("{base}/import"), Some(anna), json!({"bibtex": import})).await;
    assert_eq!(s, StatusCode::CREATED, "{created}");
    let created = created.as_array().unwrap();
    assert_eq!(created.len(), 1, "il duplicato di una nota esistente si salta");
    assert_eq!(
        (created[0]["kind"].as_str(), created[0]["url"].as_str()),
        (Some("article"), None),
        "URL non http scartato"
    );
    let (s, e) = env.post(&format!("{base}/import"), Some(anna), json!({"bibtex": "niente"})).await;
    assert_eq!((s, detail(&e)), (StatusCode::BAD_REQUEST, "Nessuna voce BibTeX riconosciuta."));
    assert_eq!(
        env.post(&format!("{base}/import"), Some(anna), json!({"bibtex": "x".repeat(200_001)})).await.0,
        StatusCode::BAD_REQUEST
    );
    env.teardown().await;
}

#[tokio::test]
async fn bibliographies_group_published_notes_and_personal_view_includes_drafts() {
    let (env, anna) = world(ALL, &[]).await;
    let note = |t: &str| json!({"notes": [{"idx": 1, "content": t}]});
    let a = new_post(&env, anna, "a", note("Fonte   Comune. 2001.")).await;
    let b = new_post(&env, anna, "b", note("fonte comune. 2001.")).await;
    let _draft = new_post(&env, anna, "bozza", note("Solo nella bozza")).await;
    publish(&env, anna, &a).await;
    publish(&env, anna, &b).await;

    let (s, bib) = env.get("/blogs/diario/bibliography", None).await;
    assert_eq!(s, StatusCode::OK);
    let bib = bib.as_array().unwrap();
    assert_eq!(bib.len(), 1, "raggruppate per testo identico, la bozza non compare");
    let cites = bib[0]["citations"].as_array().unwrap();
    assert_eq!(cites.len(), 2);
    assert!(cites[0]["permalink"].as_str().unwrap().starts_with("/diario/"));

    assert_eq!(env.get("/users/me/bibliography", None).await.0, StatusCode::UNAUTHORIZED);
    let (_, mine) = env.get("/users/me/bibliography", Some(anna)).await;
    assert_eq!(mine.as_array().unwrap().len(), 2, "vista autore: anche la bozza");
    assert_eq!(mine[0]["citations"][0]["blog_slug"], "diario");

    // blog privato: la bibliografia pubblica non si legge
    env.patch("/blogs/diario", Some(anna), json!({"visibility": "private"})).await;
    assert_eq!(env.get("/blogs/diario/bibliography", None).await.0, StatusCode::NOT_FOUND);
    env.teardown().await;
}

#[tokio::test]
async fn notes_field_is_ignored_when_the_module_is_disabled() {
    let (env, anna) = world(&["fragments"], &[]).await;
    let p = new_post(&env, anna, "uno", json!({"notes": [{"idx": 1, "content": "x"}]})).await;
    assert!(p.get("notes").is_none());
    assert_eq!(env.get("/blogs/diario/notes", Some(anna)).await.0, StatusCode::NOT_FOUND);
    env.teardown().await;
}

// ---- fragments ---------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn fragments_are_saved_only_on_published_posts_and_are_idempotent() {
    let (env, anna) = world(ALL, &[]).await;
    let lettore = env.user("lettore", "utente").await;
    let draft = new_post(&env, anna, "bozza", json!({})).await;
    let post = new_post(&env, anna, "uno", json!({"content": "a".repeat(200)})).await;
    publish(&env, anna, &post).await;
    let url = format!("/posts/{}/fragments", id(&post));

    assert_eq!(env.post(&url, None, json!({"text": "aaa"})).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        env.post(&format!("/posts/{}/fragments", id(&draft)), Some(lettore), json!({"text": "uno"})).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        env.post(&format!("/posts/{}/fragments", Uuid::new_v4()), Some(lettore), json!({"text": "uno"})).await.0,
        StatusCode::NOT_FOUND
    );
    for bad in [json!({"text": "   "}), json!({"text": "a".repeat(31)})] {
        assert_eq!(env.post(&url, Some(lettore), bad).await.0, StatusCode::BAD_REQUEST);
    }
    let (s, f) = env.post(&url, Some(lettore), json!({"text": "  aaa   aaa "})).await;
    assert_eq!(
        (s, f["text"].as_str(), f["is_public"].clone()),
        (StatusCode::CREATED, Some("aaa aaa"), json!(false)),
        "{f}"
    );
    let (_, again) = env.post(&url, Some(lettore), json!({"text": "aaa aaa", "is_public": true})).await;
    assert_eq!(again["id"], f["id"], "stesso frammento: idempotente, non un duplicato");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM post_fragments").fetch_one(&env.pool).await.unwrap();
    assert_eq!(n, 1);

    // doppio click in parallelo: un solo frammento
    let calls = (0..6).map(|_| env.post(&url, Some(lettore), json!({"text": "bbb"})));
    let mut seen = std::collections::HashSet::new();
    for (s, b) in futures::future::join_all(calls).await {
        assert_eq!(s, StatusCode::CREATED);
        seen.insert(id(&b));
    }
    assert_eq!(seen.len(), 1);

    // elenchi propri e raccolta
    assert_eq!(env.get(&url, Some(lettore)).await.1.as_array().unwrap().len(), 2);
    assert_eq!(env.get(&url, Some(anna)).await.1, json!([]), "ognuno vede i propri");
    let (_, coll) = env.get("/users/me/fragments", Some(lettore)).await;
    assert_eq!((coll[0]["permalink"].as_str(), coll[0]["post_title"].as_str()), (Some("/diario/uno"), Some("T uno")));
    assert_eq!(coll.as_array().unwrap().len(), 2);

    // visibilità: solo il proprietario del frammento
    let one = format!("/fragments/{}", id(&f));
    assert_eq!(env.patch(&one, Some(anna), json!({"is_public": true})).await.0, StatusCode::NOT_FOUND);
    let (_, u) = env.patch(&one, Some(lettore), json!({"is_public": true})).await;
    assert_eq!(u["is_public"], true);
    assert_eq!(env.delete(&one, Some(anna)).await, StatusCode::NOT_FOUND);
    assert_eq!(env.delete(&one, Some(lettore)).await, StatusCode::NO_CONTENT);
    assert_eq!(env.get(&url, Some(lettore)).await.1.as_array().unwrap().len(), 1);

    // blog privato: nemmeno chi lo conosce può salvare frammenti
    env.patch("/blogs/diario", Some(anna), json!({"visibility": "private"})).await;
    assert_eq!(env.post(&url, Some(lettore), json!({"text": "ccc"})).await.0, StatusCode::NOT_FOUND);
    env.teardown().await;
}

// ---- comments ----------------------------------------------------------------------------------------------------------

/// Turnstile finto: accetta il token `good`.
async fn turnstile() -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/siteverify", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { return };
            tokio::spawn(async move {
                let mut data = Vec::new();
                let mut buf = vec![0u8; 4096];
                // intestazioni e corpo (Content-Length)
                loop {
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    data.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&data).to_string();
                    if let Some((head, body)) = text.split_once("\r\n\r\n") {
                        let len = head
                            .to_lowercase()
                            .split("content-length:")
                            .nth(1)
                            .and_then(|r| r.split_whitespace().next())
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(0);
                        if body.len() >= len {
                            break;
                        }
                    }
                }
                let ok = String::from_utf8_lossy(&data).contains("response=good")
                    && String::from_utf8_lossy(&data).contains("secret=s3cret");
                let body = if ok { "{\"success\":true}" } else { "{\"success\":false}" };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });
    url
}

async fn comments_world() -> (Env, Uuid, Value) {
    let verify = turnstile().await;
    let (env, anna) =
        world(&["comments"], &[("NOCT_TURNSTILE_SECRET_KEY", "s3cret"), ("NOCT_TURNSTILE_VERIFY_URL", &verify)]).await;
    let post = new_post(&env, anna, "uno", json!({})).await;
    publish(&env, anna, &post).await;
    (env, anna, post)
}

#[tokio::test]
async fn registered_users_comment_freely_and_anonymous_ones_are_moderated() {
    let (env, anna, post) = comments_world().await;
    let bruno = env.user("bruno", "utente").await;
    let url = format!("/posts/{}/comments", id(&post));

    // default `members`: l'anonimo non commenta
    assert_eq!(env.get("/blogs/diario", None).await.1["comments_mode"], "members");
    let anon = json!({"content": "Ciao", "author_display_name": "Ospite", "author_email": "ospite@example.com", "captcha_token": "good"});
    assert_eq!(env.post(&url, None, anon.clone()).await.0, StatusCode::UNAUTHORIZED);

    // un registrato è subito pubblicato e il nome segue il profilo
    let (s, c) = env.post(&url, Some(bruno), json!({"content": "Bel post"})).await;
    assert_eq!(
        (s, c["status"].as_str(), c["author_display_name"].as_str()),
        (StatusCode::CREATED, Some("approved"), Some("bruno")),
        "{c}"
    );
    assert_eq!(env.post(&url, Some(bruno), json!({"content": "  "})).await.0, StatusCode::BAD_REQUEST);
    let (s, r) = env.post(&url, Some(anna), json!({"content": "Grazie", "parent_id": id(&c)})).await;
    assert_eq!((s, r["parent_id"].clone()), (StatusCode::CREATED, c["id"].clone()));
    let other = new_post(&env, anna, "due", json!({})).await;
    publish(&env, anna, &other).await;
    let (s, e) = env
        .post(&format!("/posts/{}/comments", id(&other)), Some(bruno), json!({"content": "x", "parent_id": id(&c)}))
        .await;
    assert_eq!((s, detail(&e)), (StatusCode::BAD_REQUEST, "Commento a cui rispondere non valido."));

    // modalità `everyone`: solo il proprietario la imposta
    assert_eq!(
        env.patch("/blogs/diario", Some(bruno), json!({"comments_mode": "everyone"})).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        env.patch("/blogs/diario", Some(anna), json!({"comments_mode": "tutti"})).await.0,
        StatusCode::BAD_REQUEST
    );
    let (s, b) = env.patch("/blogs/diario", Some(anna), json!({"comments_mode": "everyone"})).await;
    assert_eq!((s, b["comments_mode"].as_str()), (StatusCode::OK, Some("everyone")), "{b}");

    // captcha obbligatorio e verificato; il commento anonimo resta in moderazione
    let mut no_token = anon.clone();
    no_token.as_object_mut().unwrap().remove("captcha_token");
    for bad in [
        no_token,
        json!({"content": "x", "author_display_name": "O", "author_email": "o@example.com", "captcha_token": "bad"}),
    ] {
        let (s, e) = env.post(&url, None, bad).await;
        assert_eq!((s, detail(&e)), (StatusCode::BAD_REQUEST, "Verifica captcha non superata."));
    }
    let incomplete = json!({"content": "x", "captcha_token": "good"});
    assert_eq!(env.post(&url, None, incomplete).await.0, StatusCode::BAD_REQUEST);
    let wrong_mail =
        json!({"content": "x", "author_display_name": "O", "author_email": "non-una-mail", "captcha_token": "good"});
    assert_eq!(env.post(&url, None, wrong_mail).await.0, StatusCode::BAD_REQUEST);
    let (s, pending) = env.post(&url, None, anon).await;
    assert_eq!(
        (s, pending["status"].as_str(), pending["author_id"].clone()),
        (StatusCode::CREATED, Some("pending"), Value::Null),
        "{pending}"
    );
    assert!(pending.get("author_email").is_none(), "l'email non si espone");

    // la lista pubblica ha solo gli approvati (ordinati), la coda solo per chi modera
    let (_, public) = env.get(&url, None).await;
    assert_eq!(public.as_array().unwrap().len(), 2);
    assert_eq!(
        (public[0]["reported_to_platform"].clone(), public[0]["report_note"].clone()),
        (json!(false), Value::Null)
    );
    let queue = format!("{url}/pending");
    assert_eq!(env.get(&queue, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.get(&queue, Some(bruno)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(ids(&env.get(&queue, Some(anna)).await.1), [id(&pending)]);

    // approvazione e rifiuto sono azioni tracciate
    let approve = format!("/comments/{}/approve", id(&pending));
    assert_eq!(env.post(&approve, Some(bruno), json!({})).await.0, StatusCode::FORBIDDEN);
    let (s, ok) = env.post(&approve, Some(anna), json!({})).await;
    assert_eq!((s, ok["status"].as_str()), (StatusCode::OK, Some("approved")), "{ok}");
    assert_eq!(env.get(&url, None).await.1.as_array().unwrap().len(), 3);
    let (_, rej) = env.post(&format!("/comments/{}/reject", id(&c)), Some(anna), json!({})).await;
    assert_eq!(rej["status"], "rejected");
    assert_eq!(
        env.post(&format!("/comments/{}/reject", Uuid::new_v4()), Some(anna), json!({})).await.0,
        StatusCode::NOT_FOUND
    );
    let actions: Vec<String> =
        sqlx::query_scalar("SELECT action FROM audit_log WHERE action LIKE 'comment.%' ORDER BY occurred_at, id")
            .fetch_all(&env.pool)
            .await
            .unwrap();
    assert_eq!(actions, ["comment.approved", "comment.rejected"]);
    env.teardown().await;
}

#[tokio::test]
async fn comment_modes_per_post_auto_close_and_platform_switch() {
    let (env, anna, post) = comments_world().await;
    let bruno = env.user("bruno", "utente").await;
    let url = format!("/posts/{}/comments", id(&post));
    let path = format!("/posts/{}", id(&post));
    assert_eq!(
        (post["comments_mode"].clone(), post["effective_comments_mode"].as_str()),
        (Value::Null, Some("members"))
    );

    // override del post, tri-stato
    let (_, p) = env.patch(&path, Some(anna), json!({"comments_mode": "closed"})).await;
    assert_eq!((p["comments_mode"].as_str(), p["effective_comments_mode"].as_str()), (Some("closed"), Some("closed")));
    let (s, e) = env.post(&url, Some(bruno), json!({"content": "x"})).await;
    assert_eq!((s, detail(&e)), (StatusCode::FORBIDDEN, "I commenti sono chiusi per questo post."));
    let (_, p) = env.patch(&path, Some(anna), json!({"title": "Altro"})).await;
    assert_eq!(p["comments_mode"], "closed", "assente: invariato");
    let (_, p) = env.patch(&path, Some(anna), json!({"comments_mode": null})).await;
    assert_eq!((p["comments_mode"].clone(), p["effective_comments_mode"].as_str()), (Value::Null, Some("members")));
    assert_eq!(env.patch(&path, Some(anna), json!({"comments_mode": "boh"})).await.0, StatusCode::BAD_REQUEST);

    // chiusura automatica dopo N giorni dalla pubblicazione
    assert_eq!(
        env.patch("/blogs/diario", Some(anna), json!({"comments_auto_close_days": -1})).await.0,
        StatusCode::BAD_REQUEST
    );
    let (_, b) = env.patch("/blogs/diario", Some(anna), json!({"comments_auto_close_days": 30})).await;
    assert_eq!(b["comments_auto_close_days"], 30);
    assert_eq!(env.post(&url, Some(bruno), json!({"content": "ok"})).await.0, StatusCode::CREATED);
    sqlx::query("UPDATE posts SET published_at = now() - interval '31 days'").execute(&env.pool).await.unwrap();
    assert_eq!(env.get(&path, None).await.1["effective_comments_mode"], "closed");
    assert_eq!(env.post(&url, Some(bruno), json!({"content": "tardi"})).await.0, StatusCode::FORBIDDEN);
    let (_, b) = env.patch("/blogs/diario", Some(anna), json!({"comments_auto_close_days": null})).await;
    assert_eq!(b["comments_auto_close_days"], Value::Null);
    assert_eq!(env.post(&url, Some(bruno), json!({"content": "di nuovo"})).await.0, StatusCode::CREATED);

    // interruttore di piattaforma: con i commenti anonimi spenti nessun blog può aprirli
    sqlx::query("INSERT INTO comments_platform_settings (id, anonymous_comments_allowed) VALUES (1, false)")
        .execute(&env.pool)
        .await
        .unwrap();
    let (s, e) = env.patch("/blogs/diario", Some(anna), json!({"comments_mode": "everyone"})).await;
    assert_eq!(
        (s, detail(&e)),
        (StatusCode::BAD_REQUEST, "I commenti anonimi sono disattivati su questa piattaforma.")
    );
    env.teardown().await;
}

#[tokio::test]
async fn everyone_mode_needs_a_configured_captcha() {
    let (env, anna) = world(&["comments"], &[]).await;
    let (s, e) = env.patch("/blogs/diario", Some(anna), json!({"comments_mode": "everyone"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(detail(&e).contains("captcha"), "{e}");
    let post = new_post(&env, anna, "uno", json!({})).await;
    let (s, _) = env.patch(&format!("/posts/{}", id(&post)), Some(anna), json!({"comments_mode": "everyone"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    env.teardown().await;
}

#[tokio::test]
async fn moderation_report_block_list_and_unblock() {
    let (env, anna, post) = comments_world().await;
    let bruno = env.user("bruno", "utente").await;
    let carla = env.user("carla", "utente").await;
    let moderatore = env.user("mod", "moderatore").await;
    let blog: Uuid =
        sqlx::query_scalar("SELECT id FROM blogs WHERE slug = 'diario'").fetch_one(&env.pool).await.unwrap();
    env.member(blog, carla, "mediatore").await;
    env.patch("/blogs/diario", Some(anna), json!({"comments_mode": "everyone"})).await;
    let url = format!("/posts/{}/comments", id(&post));

    let (_, from_bruno) = env.post(&url, Some(bruno), json!({"content": "da bruno"})).await;
    let anon = json!({"content": "da anonimo", "author_display_name": "Ospite", "author_email": "Ospite@Example.com", "captcha_token": "good"});
    let (_, from_anon) = env.post(&url, None, anon.clone()).await;

    // la coda del blog: mediatore e moderatore di piattaforma sì, il semplice utente no
    let queue = "/blogs/diario/comments";
    assert_eq!(env.get(queue, Some(bruno)).await.0, StatusCode::FORBIDDEN);
    let (s, q) = env.get(queue, Some(carla)).await;
    assert_eq!((s, ids(&q)), (StatusCode::OK, vec![id(&from_anon)]));
    assert_eq!(q[0]["post_slug"], "uno");
    assert_eq!(env.get(queue, Some(moderatore)).await.0, StatusCode::OK);
    assert_eq!(ids(&env.get(&format!("{queue}?status=approved"), Some(anna)).await.1), [id(&from_bruno)]);
    assert_eq!(env.get(&format!("{queue}?status=boh"), Some(anna)).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    // segnalazione: nota obbligatoria, visibile solo a chi modera
    let report = format!("/comments/{}/report", id(&from_bruno));
    assert_eq!(env.post(&report, Some(anna), json!({"note": "  "})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(env.post(&report, Some(bruno), json!({"note": "spam"})).await.0, StatusCode::FORBIDDEN);
    let (s, r) = env.post(&report, Some(carla), json!({"note": "linguaggio offensivo"})).await;
    assert_eq!(
        (s, r["reported_to_platform"].clone(), r["report_note"].as_str()),
        (StatusCode::OK, json!(true), Some("linguaggio offensivo")),
        "{r}"
    );
    assert_eq!(ids(&env.get(&format!("{queue}?reported=true"), Some(anna)).await.1), [id(&from_bruno)]);
    let (_, public) = env.get(&url, None).await;
    assert_eq!(
        (public[0]["reported_to_platform"].clone(), public[0]["report_note"].clone()),
        (json!(false), Value::Null)
    );

    // blocco: il proprietario non si blocca; un anonimo si identifica dall'hash dell'email
    let block = |c: &Value| format!("/comments/{}/block-author", id(c));
    let (_, owners) = env.post(&url, Some(anna), json!({"content": "del proprietario"})).await;
    let (s, e) = env.post(&block(&owners), Some(carla), json!({})).await;
    assert_eq!((s, detail(&e)), (StatusCode::BAD_REQUEST, "Non puoi bloccare il proprietario del blog."));
    let (s, b) = env.post(&block(&from_anon), Some(anna), json!({"note": "spam ricorrente"})).await;
    assert_eq!(
        (s, b["is_anonymous"].clone(), b["label"].as_str(), b["note"].as_str()),
        (StatusCode::OK, json!(true), Some("Ospite"), Some("spam ricorrente")),
        "{b}"
    );
    let (_, again) = env.post(&block(&from_anon), Some(carla), json!({})).await;
    assert_eq!(again["id"], b["id"], "un solo blocco per autore");
    let hashes: Vec<Option<String>> =
        sqlx::query_scalar("SELECT email_hash FROM blog_blocked_authors").fetch_all(&env.pool).await.unwrap();
    assert_eq!(hashes.len(), 1);
    assert!(hashes[0].as_deref().is_some_and(|h| h.len() == 64 && !h.contains("ospite")), "mai l'email in chiaro");
    // lo stesso anonimo, con altre maiuscole, non commenta più; il commento bloccato è rifiutato
    let (s, e) = env.post(&url, None, anon).await;
    assert_eq!((s, detail(&e)), (StatusCode::FORBIDDEN, "Non puoi commentare su questo blog."));
    let (_, rejected) = env.get(&format!("{queue}?status=rejected"), Some(anna)).await;
    assert_eq!(ids(&rejected), [id(&from_anon)]);

    // blocco di un utente registrato
    let (_, bl) = env.post(&block(&from_bruno), Some(anna), json!({})).await;
    assert_eq!((bl["is_anonymous"].clone(), bl["label"].as_str()), (json!(false), Some("@bruno")));
    assert_eq!(env.post(&url, Some(bruno), json!({"content": "ancora"})).await.0, StatusCode::FORBIDDEN);

    // elenco e sblocco
    assert_eq!(env.get("/blogs/diario/blocked", Some(bruno)).await.0, StatusCode::FORBIDDEN);
    let (_, list) = env.get("/blogs/diario/blocked", Some(carla)).await;
    assert_eq!(list.as_array().unwrap().len(), 2);
    assert_eq!(env.delete(&format!("/blogs/diario/blocked/{}", id(&bl)), Some(bruno)).await, StatusCode::FORBIDDEN);
    assert_eq!(env.delete(&format!("/blogs/diario/blocked/{}", id(&bl)), Some(carla)).await, StatusCode::NO_CONTENT);
    assert_eq!(env.delete(&format!("/blogs/diario/blocked/{}", id(&bl)), Some(carla)).await, StatusCode::NOT_FOUND);
    assert_eq!(env.post(&url, Some(bruno), json!({"content": "di nuovo"})).await.0, StatusCode::CREATED);
    env.teardown().await;
}

#[tokio::test]
async fn comments_need_a_visible_published_post() {
    let (env, anna) = world(&["comments"], &[]).await;
    let bruno = env.user("bruno", "utente").await;
    let draft = new_post(&env, anna, "bozza", json!({})).await;
    let url = format!("/posts/{}/comments", id(&draft));
    assert_eq!(env.post(&url, Some(bruno), json!({"content": "x"})).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get(&url, None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        env.post(&format!("/posts/{}/comments", Uuid::new_v4()), Some(bruno), json!({"content": "x"})).await.0,
        StatusCode::NOT_FOUND
    );
    publish(&env, anna, &draft).await;
    env.patch("/blogs/diario", Some(anna), json!({"visibility": "private"})).await;
    assert_eq!(env.post(&url, Some(bruno), json!({"content": "x"})).await.0, StatusCode::NOT_FOUND, "blog privato");
    assert_eq!(env.post(&url, Some(anna), json!({"content": "x"})).await.0, StatusCode::CREATED);
    env.teardown().await;
}

#[tokio::test]
async fn comment_fields_are_ignored_when_the_module_is_disabled() {
    let (env, anna) = world(&["notes"], &[]).await;
    let p = new_post(&env, anna, "uno", json!({"comments_mode": "closed"})).await;
    assert!(p.get("comments_mode").is_none() && p.get("effective_comments_mode").is_none());
    let (_, b) = env.patch("/blogs/diario", Some(anna), json!({"comments_mode": "closed"})).await;
    assert!(b.get("comments_mode").is_none());
    assert_eq!(env.get("/blogs/diario/blocked", Some(anna)).await.0, StatusCode::NOT_FOUND);
    env.teardown().await;
}
