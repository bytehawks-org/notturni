//! Profilo utente: lettura e modifica, username con cooldown, avatar, link social, cambio email in
//! tre passi, directory, sigillo di verifica e viste pubbliche di blog e post.

mod common;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use common::*;
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000";
const JPG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0];

async fn world(modules: &[&str]) -> (Env, Uuid) {
    let env = setup(modules, &[]).await;
    let anna = env.user("anna", "utente").await;
    (env, anna)
}

async fn upload(env: &Env, who: Uuid, ct: &str, data: &[u8]) -> (StatusCode, Value) {
    let mut body = format!(
        "--XBOUNDARYX\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a\"\r\nContent-Type: {ct}\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(data);
    body.extend_from_slice(b"\r\n--XBOUNDARYX--\r\n");
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/users/me/avatar")
        .header(header::CONTENT_TYPE, "multipart/form-data; boundary=XBOUNDARYX")
        .header(header::AUTHORIZATION, format!("Bearer {}", env.access_token(who)))
        .body(Body::from(body))
        .unwrap();
    let (s, _, j) = env.send(req).await;
    (s, j)
}

fn on_disk(env: &Env, url: &str) -> std::path::PathBuf {
    env.storage_dir.join(url.strip_prefix("http://localhost:8000/storage/").expect("URL del backend locale"))
}

async fn last_code(env: &Env, count: usize) -> (String, String) {
    for _ in 0..100 {
        if env.mailer.sent().len() >= count {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let sent = env.mailer.sent();
    let mail = sent.last().expect("email attesa");
    assert_eq!(sent.len(), count, "{sent:?}");
    let code = mail.text.split_once("Codice di conferma: ").unwrap().1.split_whitespace().next().unwrap().to_string();
    (mail.to[0].clone(), code)
}

fn wrong(code: &str) -> &'static str {
    if code == "000000" { "000001" } else { "000000" }
}

// ---- lettura e modifica -------------------------------------------------------------------------------------------

#[tokio::test]
async fn own_profile_has_private_fields_and_public_profile_has_none() {
    let (env, anna) = world(&[]).await;
    let (s, me) = env.get("/users/me", Some(anna)).await;
    assert_eq!(s, StatusCode::OK, "{me}");
    assert_eq!(me["username"], "anna");
    assert_eq!(me["email"], "anna@example.com");
    assert_eq!(me["directory_listed"], true);
    assert_eq!(me["fallback_languages"], json!([]));
    assert_eq!(me["social_links"], json!([]));
    assert_eq!(me["post_author_name_style"], "username");
    assert_eq!(me["verification_tier"], "none");
    assert_eq!(me["pending_email_change"], Value::Null);
    assert_eq!(me["next_username_change_allowed_at"], Value::Null);
    assert_eq!(me["domain_pending_verification"], Value::Null);
    let id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE username = 'anna'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(me["atproto_did"], format!("did:web:notturni.eu:users:{id}"));
    assert_eq!(me["activitypub_actor_id"], format!("https://notturni.eu/ap/actors/{id}"));
    assert_eq!(env.get("/users/me", None).await.0, StatusCode::UNAUTHORIZED);

    let (s, public) = env.get("/users/anna", None).await;
    assert_eq!(s, StatusCode::OK, "{public}");
    for private in ["email", "directory_listed", "pending_email_change", "username_changed_at"] {
        assert!(public.get(private).is_none(), "{private} non deve essere pubblico");
    }
    assert_eq!(env.get("/users/nessuno", None).await.0, StatusCode::NOT_FOUND);
    // `me` non è uno username raggiungibile come profilo pubblico
    assert_eq!(env.get("/users/ME", None).await.0, StatusCode::NOT_FOUND);

    // un account disattivato non ha un profilo pubblico
    sqlx::query("UPDATE users SET is_active = false WHERE username = 'anna'").execute(&env.pool).await.unwrap();
    assert_eq!(env.get("/users/anna", None).await.0, StatusCode::NOT_FOUND);
    env.teardown().await;
}

#[tokio::test]
async fn patch_updates_only_the_given_fields_and_validates_the_rest() {
    let (env, anna) = world(&["interests"]).await;
    let (s, me) = env
        .patch(
            "/users/me",
            Some(anna),
            json!({
                "bio": "Scrivo di notte", "first_name": "Anna", "last_name": "Rossi", "display_name": "  Penna  ",
                "country": "it", "native_language": "IT", "fallback_languages": ["en", "EN", "de"],
                "ui_locale": "en", "post_author_name_style": "full_name", "directory_listed": false,
            }),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{me}");
    assert_eq!(
        (me["bio"].as_str(), me["first_name"].as_str(), me["display_name"].as_str(), me["country"].as_str()),
        (Some("Scrivo di notte"), Some("Anna"), Some("Penna"), Some("IT"))
    );
    assert_eq!(me["native_language"], "it");
    assert_eq!(me["fallback_languages"], json!(["en", "de"]), "senza duplicati");
    assert_eq!(
        (me["post_author_name_style"].as_str(), me["directory_listed"].as_bool()),
        (Some("full_name"), Some(false))
    );
    let ui: Option<String> =
        sqlx::query_scalar("SELECT ui_locale FROM users WHERE username = 'anna'").fetch_one(&env.pool).await.unwrap();
    assert_eq!(ui.as_deref(), Some("en"));

    // campi assenti: invariati; "" azzera
    let (_, me) = env.patch("/users/me", Some(anna), json!({"bio": "", "country": "", "ui_locale": ""})).await;
    assert_eq!((me["bio"].clone(), me["country"].clone()), (Value::Null, Value::Null));
    assert_eq!(me["first_name"], "Anna");
    assert_eq!(me["fallback_languages"], json!(["en", "de"]));
    let (_, me) = env.patch("/users/me", Some(anna), json!({"fallback_languages": []})).await;
    assert_eq!(me["fallback_languages"], json!([]));

    for (body, needle) in [
        (json!({"country": "ITA"}), "paese"),
        (json!({"native_language": "ita"}), "lingua"),
        (json!({"fallback_languages": ["it", "en", "de", "fr", "es", "pt"]}), "Massimo 5"),
        (json!({"ui_locale": "de"}), "non supportata"),
        (json!({"interests": ["nope"]}), "sconosciuti"),
    ] {
        let (s, b) = env.patch("/users/me", Some(anna), body.clone()).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}: {b}");
        assert!(detail(&b).contains(needle), "{body}: {b}");
    }
    assert_eq!(
        env.patch("/users/me", Some(anna), json!({"post_author_name_style": "x"})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(env.patch("/users/me", None, json!({"bio": "x"})).await.0, StatusCode::UNAUTHORIZED);
    env.teardown().await;
}

#[tokio::test]
async fn interests_come_from_the_platform_list_and_need_the_module() {
    let (env, anna) = world(&["interests"]).await;
    sqlx::query("INSERT INTO interests_settings (id, items) VALUES (1, $1) ON CONFLICT (id) DO UPDATE SET items = EXCLUDED.items")
        .bind(
            json!([{"key": "music", "translations": {"it": "Musica"}}, {"key": "art", "translations": {"it": "Arte"}}]),
        )
        .execute(&env.pool)
        .await
        .unwrap();
    let (s, me) = env.patch("/users/me", Some(anna), json!({"interests": ["Music", "music", "art"]})).await;
    assert_eq!((s, me["interests"].clone()), (StatusCode::OK, json!(["music", "art"])));
    assert_eq!(env.get("/users/anna", None).await.1["interests"], json!(["music", "art"]));
    let (_, me) = env.patch("/users/me", Some(anna), json!({"interests": []})).await;
    assert_eq!(me["interests"], json!([]));
    env.teardown().await;

    // senza il modulo non c'è nulla da scegliere
    let (env, anna) = world(&[]).await;
    assert_eq!(env.patch("/users/me", Some(anna), json!({"interests": ["music"]})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(env.patch("/users/me", Some(anna), json!({"interests": []})).await.0, StatusCode::OK);
    env.teardown().await;
}

#[tokio::test]
async fn username_change_has_a_cooldown_and_is_audited() {
    let (env, anna) = world(&[]).await;
    env.user("bruno", "utente").await;
    // formato, riservati, già in uso
    for (name, status) in [("A", 400), ("admin", 400), ("me", 400), ("ab", 400), ("bruno", 409), ("BRUNO", 409)] {
        let (s, b) = env.patch("/users/me", Some(anna), json!({"username": name})).await;
        assert_eq!(s.as_u16(), status, "{name}: {b}");
    }
    // lo stesso username (anche con maiuscole e spazi) non è un cambio
    let (s, me) = env.patch("/users/me", Some(anna), json!({"username": " Anna "})).await;
    assert_eq!((s, me["username_changed_at"].clone()), (StatusCode::OK, Value::Null));

    let (s, me) = env.patch("/users/me", Some(anna), json!({"username": " Anna-Rossi "})).await;
    assert_eq!(s, StatusCode::OK, "{me}");
    assert_eq!(me["username"], "anna-rossi");
    assert!(me["username_changed_at"].is_string() && me["next_username_change_allowed_at"].is_string());
    // il vecchio nome non risolve più, il nuovo sì
    assert_eq!(env.get("/users/anna", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/users/anna-rossi", None).await.0, StatusCode::OK);

    let (s, b) = env.patch("/users/me", Some(anna), json!({"username": "terzo"})).await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert!(detail(&b).contains("una volta ogni 5 giorni"), "{b}");
    // passato il cooldown si può cambiare di nuovo
    sqlx::query("UPDATE users SET username_changed_at = now() - interval '6 days'").execute(&env.pool).await.unwrap();
    assert_eq!(env.patch("/users/me", Some(anna), json!({"username": "terzo"})).await.0, StatusCode::OK);

    let audit: Vec<(String, Value)> = sqlx::query_as(
        "SELECT action, payload FROM audit_log WHERE action = 'user.username_changed' ORDER BY occurred_at",
    )
    .fetch_all(&env.pool)
    .await
    .unwrap();
    assert_eq!(audit.len(), 2);
    assert_eq!(audit[0].1, json!({"old_username": "anna", "new_username": "anna-rossi"}));
    env.teardown().await;
}

#[tokio::test]
async fn verification_tier_is_computed_on_read() {
    let (env, anna) = world(&[]).await;
    let tier = |who: &'static str| {
        let env = &env;
        async move { env.get(&format!("/users/{who}"), None).await.1["verification_tier"].as_str().unwrap().to_string() }
    };
    assert_eq!(tier("anna").await, "none");
    // dominio personale verificato → bronzo, e il profilo si trova anche con quel dominio
    sqlx::query("UPDATE users SET verified_domain = 'anna.example.org' WHERE username = 'anna'")
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(tier("anna").await, "bronze");
    let (s, p) = env.get("/users/anna.example.org", None).await;
    assert_eq!(
        (s, p["username"].clone(), p["custom_domain"].clone()),
        (StatusCode::OK, json!("anna"), json!("anna.example.org"))
    );
    // dominio email dell'istanza o di un'organizzazione → blu
    sqlx::query("UPDATE users SET email = 'anna@notturni.eu' WHERE username = 'anna'")
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(tier("anna").await, "blue");
    sqlx::query(
        "INSERT INTO verification_settings (id, silver_identifiers, gold_identifiers) VALUES (1, '{anna}', '{}')",
    )
    .execute(&env.pool)
    .await
    .unwrap();
    assert_eq!(tier("anna").await, "silver");
    sqlx::query("UPDATE verification_settings SET gold_identifiers = '{anna@notturni.eu}'")
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(tier("anna").await, "gold");
    // togliere l'utente dalle liste lo riporta al sigillo che gli spetta: nulla da risincronizzare
    sqlx::query("UPDATE verification_settings SET gold_identifiers = '{}', silver_identifiers = '{}'")
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(tier("anna").await, "blue");
    let _ = anna;
    env.teardown().await;
}

// ---- avatar e link social -----------------------------------------------------------------------------------------

#[tokio::test]
async fn avatar_upload_replace_and_delete() {
    let (env, anna) = world(&[]).await;
    let (s, out) = upload(&env, anna, "image/png", PNG).await;
    assert_eq!(s, StatusCode::OK, "{out}");
    let url = out["avatar_url"].as_str().unwrap().to_string();
    assert!(url.contains("/avatars/") && url.ends_with(".png"), "{url}");
    assert!(on_disk(&env, &url).exists());
    assert_eq!(env.get("/users/me", Some(anna)).await.1["avatar_url"], url);
    assert_eq!(env.get("/users/anna", None).await.1["avatar_url"], url);

    // sostituire l'avatar cancella il file precedente
    let (s, out2) = upload(&env, anna, "image/jpeg", JPG).await;
    assert_eq!(s, StatusCode::OK, "{out2}");
    let url2 = out2["avatar_url"].as_str().unwrap();
    assert!(!on_disk(&env, &url).exists() && on_disk(&env, url2).exists());

    // tipo dichiarato diverso dal reale, formato non ammesso, troppo grande, non autenticato
    assert_eq!(upload(&env, anna, "image/jpeg", PNG).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(upload(&env, anna, "image/png", b"<svg onload=alert(1)>").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(upload(&env, anna, "image/gif", b"GIF89a....").await.0, StatusCode::BAD_REQUEST);
    let mut big = PNG.to_vec();
    big.resize(2 * 1024 * 1024 + 1, 0);
    let (s, b) = upload(&env, anna, "image/png", &big).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert!(detail(&b).contains("2 MiB"), "{b}");
    assert_eq!(env.post("/users/me/avatar", None, json!({})).await.0, StatusCode::UNAUTHORIZED);

    assert_eq!(env.delete("/users/me/avatar", Some(anna)).await, StatusCode::NO_CONTENT);
    assert!(!on_disk(&env, url2).exists());
    assert_eq!(env.get("/users/me", Some(anna)).await.1["avatar_url"], Value::Null);
    assert_eq!(env.delete("/users/me/avatar", Some(anna)).await, StatusCode::NO_CONTENT, "idempotente");
    env.teardown().await;
}

#[tokio::test]
async fn social_links_are_limited_validated_and_owned() {
    let (env, anna) = world(&[]).await;
    let bruno = env.user("bruno", "utente").await;
    for bad in ["javascript:alert(1)", "data:text/html,x", "example.com", "//example.com", "https://"] {
        let (s, b) = env.post("/users/me/social-links", Some(anna), json!({"label": "x", "url": bad})).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad}: {b}");
    }
    assert_eq!(
        env.post("/users/me/social-links", Some(anna), json!({"label": "  ", "url": "https://example.com"})).await.0,
        StatusCode::BAD_REQUEST
    );
    let mut ids = Vec::new();
    for i in 0..5 {
        let (s, l) = env
            .post(
                "/users/me/social-links",
                Some(anna),
                json!({"label": format!("L{i}"), "url": format!("https://example.com/{i}")}),
            )
            .await;
        assert_eq!((s, l["position"].clone()), (StatusCode::CREATED, json!(i)), "{l}");
        ids.push(l["id"].as_str().unwrap().to_string());
    }
    let (s, b) =
        env.post("/users/me/social-links", Some(anna), json!({"label": "x", "url": "https://example.com"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert!(detail(&b).contains("Massimo 5"));

    let links = env.get("/users/anna", None).await.1["social_links"].clone();
    assert_eq!(
        links.as_array().unwrap().iter().map(|l| l["label"].as_str().unwrap()).collect::<Vec<_>>(),
        ["L0", "L1", "L2", "L3", "L4"]
    );

    // solo il proprietario cancella; id inesistente o non valido → 404
    assert_eq!(env.delete(&format!("/users/me/social-links/{}", ids[0]), Some(bruno)).await, StatusCode::NOT_FOUND);
    assert_eq!(env.delete("/users/me/social-links/non-un-uuid", Some(anna)).await, StatusCode::NOT_FOUND);
    assert_eq!(env.delete(&format!("/users/me/social-links/{}", ids[0]), Some(anna)).await, StatusCode::NO_CONTENT);
    assert_eq!(env.delete(&format!("/users/me/social-links/{}", ids[0]), Some(anna)).await, StatusCode::NOT_FOUND);
    // dopo una cancellazione la posizione successiva non collide con quelle esistenti
    let (_, l) =
        env.post("/users/me/social-links", Some(anna), json!({"label": "L5", "url": "https://example.com/5"})).await;
    assert_eq!(l["position"], 5);
    env.teardown().await;
}

// ---- cambio email -------------------------------------------------------------------------------------------------

#[tokio::test]
async fn email_change_goes_through_three_steps() {
    let (env, anna) = world(&[]).await;
    let bruno = env.user("bruno", "utente").await;
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();

    // validazioni del passo 1
    for (email, needle) in
        [("non-una-email", "non valido"), ("ANNA@example.com", "coincide"), ("bruno@example.com", "già in uso")]
    {
        let (s, b) = env.post("/users/me/email/request", Some(anna), json!({"new_email": email})).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{email}: {b}");
        assert!(detail(&b).contains(needle), "{email}: {b}");
    }
    // i passi successivi senza una richiesta
    assert_eq!(
        env.post("/users/me/email/verify-current", Some(anna), json!({"code": "123456"})).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        env.post("/users/me/email/verify-new", Some(anna), json!({"code": "123456"})).await.0,
        StatusCode::BAD_REQUEST
    );

    // passo 1: il codice parte dalla casella attuale
    let (s, b) = env.post("/users/me/email/request", Some(anna), json!({"new_email": " Nuova@Example.com "})).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{b}");
    let (to, old_code) = last_code(&env, 1).await;
    assert_eq!(to, "anna@example.com");
    let (_, me) = env.get("/users/me", Some(anna)).await;
    assert_eq!(
        me["pending_email_change"],
        json!({"new_email": "nuova@example.com", "stage": "awaiting_old_confirmation"})
    );
    // il codice non sta né nel database né nell'outbox
    let leaked: i64 = sqlx::query_scalar("SELECT count(*) FROM noct_outbox WHERE payload::text LIKE $1")
        .bind(format!("%{old_code}%"))
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(leaked, 0);
    // il passo 3 non si salta
    assert_eq!(
        env.post("/users/me/email/verify-new", Some(anna), json!({"code": old_code})).await.0,
        StatusCode::BAD_REQUEST
    );

    // passo 2: codice sbagliato, quello di un altro utente, poi quello giusto
    let (s, b) = env.post("/users/me/email/verify-current", Some(anna), json!({"code": wrong(&old_code)})).await;
    assert_eq!((s, detail(&b)), (StatusCode::BAD_REQUEST, "Codice non valido o scaduto."));
    assert_eq!(
        env.post("/users/me/email/verify-current", Some(bruno), json!({"code": old_code})).await.0,
        StatusCode::BAD_REQUEST
    );
    let (s, b) = env.post("/users/me/email/verify-current", Some(anna), json!({"code": &old_code})).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{b}");
    let (to, new_code) = last_code(&env, 2).await;
    assert_eq!(to, "nuova@example.com", "il secondo codice va alla casella nuova");
    assert_eq!(
        env.get("/users/me", Some(anna)).await.1["pending_email_change"],
        json!({"new_email": "nuova@example.com", "stage": "awaiting_new_confirmation"})
    );
    // il passo 2 non si ripete e il codice vecchio non vale per la casella nuova
    let (s, b) = env.post("/users/me/email/verify-current", Some(anna), json!({"code": &old_code})).await;
    assert_eq!((s, detail(&b)), (StatusCode::BAD_REQUEST, "Passo già completato."));
    assert_ne!(old_code, new_code);
    assert_eq!(
        env.post("/users/me/email/verify-new", Some(anna), json!({"code": &old_code})).await.0,
        StatusCode::BAD_REQUEST
    );

    // un altro account nel frattempo prende l'indirizzo: il cambio non si completa
    sqlx::query("UPDATE users SET email = 'nuova@example.com' WHERE username = 'bruno'")
        .execute(&env.pool)
        .await
        .unwrap();
    let (s, b) = env.post("/users/me/email/verify-new", Some(anna), json!({"code": &new_code})).await;
    assert_eq!((s, detail(&b)), (StatusCode::BAD_REQUEST, "Email già in uso da un altro account."));
    sqlx::query("UPDATE users SET email = 'bruno@example.com' WHERE username = 'bruno'")
        .execute(&env.pool)
        .await
        .unwrap();

    // passo 3
    let (s, me) = env.post("/users/me/email/verify-new", Some(anna), json!({"code": &new_code})).await;
    assert_eq!(s, StatusCode::OK, "{me}");
    assert_eq!((me["email"].as_str(), me["pending_email_change"].clone()), (Some("nuova@example.com"), Value::Null));
    assert_eq!(
        env.post("/users/me/email/verify-new", Some(anna), json!({"code": &new_code})).await.0,
        StatusCode::BAD_REQUEST,
        "monouso"
    );
    let audit: Value = sqlx::query_scalar("SELECT payload FROM audit_log WHERE action = 'user.email_changed'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(audit, json!({"old_email": "anna@example.com", "new_email": "nuova@example.com"}));
    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn email_change_can_be_cancelled_replaced_and_expires() {
    let (env, anna) = world(&[]).await;
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();
    env.post("/users/me/email/request", Some(anna), json!({"new_email": "uno@example.com"})).await;
    let (_, first) = last_code(&env, 1).await;
    // una nuova richiesta sostituisce la precedente: il suo codice non vale più
    env.post("/users/me/email/request", Some(anna), json!({"new_email": "due@example.com"})).await;
    let (_, second) = last_code(&env, 2).await;
    let (s, _) = env.post("/users/me/email/verify-current", Some(anna), json!({"code": &first})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // scaduto
    sqlx::query("UPDATE email_change_requests SET old_expires_at = now() - interval '1 minute'")
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(
        env.post("/users/me/email/verify-current", Some(anna), json!({"code": &second})).await.0,
        StatusCode::BAD_REQUEST
    );
    // annullata a qualunque passo; idempotente
    assert_eq!(env.delete("/users/me/email/request", Some(anna)).await, StatusCode::NO_CONTENT);
    assert_eq!(env.get("/users/me", Some(anna)).await.1["pending_email_change"], Value::Null);
    assert_eq!(env.delete("/users/me/email/request", Some(anna)).await, StatusCode::NO_CONTENT);
    assert_eq!(
        env.post("/users/me/email/verify-current", Some(anna), json!({"code": &second})).await.0,
        StatusCode::BAD_REQUEST
    );
    background.stop().await;
    env.teardown().await;
}

// ---- directory e viste pubbliche ----------------------------------------------------------------------------------

#[tokio::test]
async fn directory_lists_active_opted_in_users_with_filters() {
    let (env, anna) = world(&["discovery"]).await;
    let bruno = env.user("bruno", "utente").await;
    let carla = env.user("carla", "utente").await;
    env.user("dario", "utente").await;
    env.user("root", "super_admin").await;
    env.user("eva", "utente").await;
    env.patch(
        "/users/me",
        Some(anna),
        json!({"bio": "scrive di viaggi", "native_language": "it", "display_name": "Penna"}),
    )
    .await;
    env.patch("/users/me", Some(bruno), json!({"native_language": "en", "bio": "100% vero"})).await;
    sqlx::query("UPDATE users SET interests = '{music,art}' WHERE username = 'carla'")
        .execute(&env.pool)
        .await
        .unwrap();
    env.patch("/users/me", Some(carla), json!({"directory_listed": true})).await;
    sqlx::query("UPDATE users SET directory_listed = false WHERE username = 'dario'").execute(&env.pool).await.unwrap();
    sqlx::query("UPDATE users SET is_active = false WHERE username = 'eva'").execute(&env.pool).await.unwrap();
    // follower: carla è seguita da anna e bruno, anna da bruno
    for (who, target) in [(anna, "carla"), (bruno, "carla"), (bruno, "anna")] {
        assert_eq!(env.post(&format!("/users/{target}/follow"), Some(who), json!({})).await.0, StatusCode::NO_CONTENT);
    }
    let names = |v: &Value| {
        v.as_array().unwrap().iter().map(|u| u["username"].as_str().unwrap().to_string()).collect::<Vec<_>>()
    };

    let (s, all) = env.get("/users", None).await;
    assert_eq!(s, StatusCode::OK, "{all}");
    // più recenti prima; niente super admin, opt-out o disattivati
    assert_eq!(names(&all), ["carla", "bruno", "anna"]);
    assert!(all[0].get("email").is_none() && all[0].get("directory_listed").is_none());
    assert_eq!(names(&env.get("/users?sort=followers", None).await.1), ["carla", "anna", "bruno"]);
    assert_eq!(env.get("/users?sort=followers", None).await.1[0]["follower_count"], 2);
    assert_eq!(names(&env.get("/users?q=pen", None).await.1), ["anna"], "cerca anche nel nome pubblico");
    assert_eq!(names(&env.get("/users?q=viaggi", None).await.1), ["anna"], "e nella bio");
    assert_eq!(names(&env.get("/users?q=100%25", None).await.1), ["bruno"], "il % è letterale");
    assert_eq!(names(&env.get("/users?q=%25", None).await.1), ["bruno"]);
    assert_eq!(names(&env.get("/users?locale=en", None).await.1), ["bruno"]);
    assert_eq!(names(&env.get("/users?interest=art", None).await.1), ["carla"]);
    assert_eq!(names(&env.get("/users?interest=nope", None).await.1), Vec::<String>::new());
    assert_eq!(names(&env.get("/users?limit=1&offset=1", None).await.1), ["bruno"]);
    assert_eq!(env.get("/users?limit=abc", None).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    env.teardown().await;

    // senza `discovery` non ci sono follow: nessun conteggio, ordinamento per data
    let (env, _) = world(&[]).await;
    let (s, list) = env.get("/users?sort=followers", None).await;
    assert_eq!((s, list[0]["follower_count"].clone()), (StatusCode::OK, json!(0)));
    env.teardown().await;
}

#[tokio::test]
async fn public_blogs_and_posts_of_a_user_hide_aliased_ones() {
    let (env, anna) = world(&[]).await;
    let bruno = env.user("bruno", "utente").await;
    env.blog("diario", anna, "public").await;
    env.blog("ombra", anna, "public").await;
    env.blog("riservato", anna, "private").await;
    env.blog("altrui", bruno, "public").await;
    sqlx::query("UPDATE blogs SET default_author_display_name = 'Penna' WHERE slug = 'ombra'")
        .execute(&env.pool)
        .await
        .unwrap();
    let publish = |blog: &'static str, slug: &'static str, who: Uuid| {
        let env = &env;
        async move {
            let (s, p) = env
                .post(
                    &format!("/blogs/{blog}/posts"),
                    Some(who),
                    json!({"slug": slug, "title": slug, "content": "testo"}),
                )
                .await;
            assert_eq!(s, StatusCode::CREATED, "{p}");
            let (s, b) = env.post(&format!("/posts/{}/publish", p["id"].as_str().unwrap()), Some(who), json!({})).await;
            assert_eq!(s, StatusCode::OK, "{b}");
        }
    };
    publish("diario", "firmato", anna).await;
    publish("diario", "secondo", anna).await;
    publish("ombra", "con-alias", anna).await;
    publish("riservato", "privato", anna).await;
    publish("altrui", "di-bruno", bruno).await;
    // un post in bozza non si vede
    env.post("/blogs/diario/posts", Some(anna), json!({"slug": "bozza", "title": "bozza", "content": "x"})).await;

    let (s, blogs) = env.get("/users/anna/blogs", None).await;
    assert_eq!(s, StatusCode::OK, "{blogs}");
    assert_eq!(blogs.as_array().unwrap().iter().map(|b| b["slug"].as_str().unwrap()).collect::<Vec<_>>(), ["diario"]);
    assert_eq!(blogs[0]["owner_id"], Value::Null, "l'identità del proprietario non esce dall'elenco");

    let (s, posts) = env.get("/users/anna/posts", None).await;
    assert_eq!(s, StatusCode::OK, "{posts}");
    let slugs: Vec<&str> = posts.as_array().unwrap().iter().map(|p| p["slug"].as_str().unwrap()).collect();
    assert_eq!(slugs.len(), 2);
    assert!(slugs.contains(&"firmato") && slugs.contains(&"secondo"), "{slugs:?}");
    assert_eq!(env.get("/users/anna/posts?limit=1", None).await.1.as_array().unwrap().len(), 1);
    assert_eq!(env.get("/users/anna/posts?limit=1&offset=1", None).await.1.as_array().unwrap().len(), 1);
    assert_eq!(env.get("/users/anna/posts?limit=x", None).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(env.get("/users/nessuno/posts", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(env.get("/users/nessuno/blogs", None).await.0, StatusCode::NOT_FOUND);

    // firmare con lo username in un blog con alias (alias uguale allo username) lo rende visibile
    sqlx::query("UPDATE blogs SET default_author_display_name = 'Anna' WHERE slug = 'ombra'")
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(env.get("/users/anna/blogs", None).await.1.as_array().unwrap().len(), 2);
    env.teardown().await;
}

#[tokio::test]
async fn public_comments_of_a_user_are_approved_visible_and_signed_with_the_username() {
    let (env, anna) = world(&["comments"]).await;
    let bruno = env.user("bruno", "utente").await;
    env.blog("diario", anna, "public").await;
    env.blog("riservato", anna, "public").await;
    let mut posts = Vec::new();
    for (blog, slug) in [("diario", "uno"), ("diario", "due"), ("riservato", "privato")] {
        let (_, p) = env
            .post(&format!("/blogs/{blog}/posts"), Some(anna), json!({"slug": slug, "title": slug, "content": "x"}))
            .await;
        let id = p["id"].as_str().unwrap().to_string();
        assert_eq!(env.post(&format!("/posts/{id}/publish"), Some(anna), json!({})).await.0, StatusCode::OK);
        posts.push(id);
    }
    for (post, text) in [(&posts[0], "primo"), (&posts[1], "secondo"), (&posts[2], "nascosto")] {
        let (s, c) = env.post(&format!("/posts/{post}/comments"), Some(bruno), json!({"content": text})).await;
        assert_eq!(s, StatusCode::CREATED, "{c}");
    }
    // il blog diventa privato dopo il commento: il commento non è più pubblico
    sqlx::query("UPDATE blogs SET visibility = 'private' WHERE slug = 'riservato'").execute(&env.pool).await.unwrap();
    // un commento rifiutato o firmato con un altro nome non si mostra
    sqlx::query("UPDATE comments SET status = 'rejected' WHERE content = 'secondo'").execute(&env.pool).await.unwrap();
    let (_, extra) =
        env.post(&format!("/posts/{}/comments", posts[0]), Some(bruno), json!({"content": "con-nome"})).await;
    sqlx::query("UPDATE comments SET author_display_name = 'Maschera' WHERE id = $1::uuid")
        .bind(extra["id"].as_str().unwrap())
        .execute(&env.pool)
        .await
        .unwrap();

    let (s, list) = env.get("/users/bruno/comments", None).await;
    assert_eq!(s, StatusCode::OK, "{list}");
    assert_eq!(list.as_array().unwrap().len(), 1, "{list}");
    assert_eq!(
        (list[0]["content"].as_str(), list[0]["post_title"].as_str(), list[0]["permalink"].as_str()),
        (Some("primo"), Some("uno"), Some("/diario/uno"))
    );
    assert_eq!(env.get("/users/bruno/comments?limit=1&offset=1", None).await.1, json!([]));
    assert_eq!(env.get("/users/bruno/comments?limit=x", None).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(env.get("/users/nessuno/comments", None).await.0, StatusCode::NOT_FOUND);
    env.teardown().await;
}
