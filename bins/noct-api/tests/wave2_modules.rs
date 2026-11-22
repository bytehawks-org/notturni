//! Moduli della seconda ondata: segnalazioni, newsletter e domini personali.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use uuid::Uuid;

async fn published_post(env: &Env, who: Uuid, blog: &str, slug: &str) -> String {
    let (s, p) = env
        .post(
            &format!("/blogs/{blog}/posts"),
            Some(who),
            json!({"slug": slug, "title": slug, "content": "testo del post"}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{p}");
    let id = p["id"].as_str().unwrap().to_string();
    let (s, b) = env.post(&format!("/posts/{id}/publish"), Some(who), json!({})).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    id
}

// ---- segnalazioni ---------------------------------------------------------------------------------------------------

#[tokio::test]
async fn reports_are_one_per_reader_and_target_and_respect_visibility() {
    let env = setup(&["reports"], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    let carla = env.user("carla", "utente").await;
    env.blog("diario", anna, "public").await;
    env.blog("segreto", anna, "private").await;
    let post = published_post(&env, anna, "diario", "uno").await;
    let (_, draft) =
        env.post("/blogs/diario/posts", Some(anna), json!({"slug": "bozza", "title": "b", "content": "x"})).await;

    // blog: segnalazione con nota (tagliata a 500 caratteri), idempotente
    let long_note = "x".repeat(700);
    let (s, r) = env
        .post("/blogs/diario/report", Some(bruno), json!({"reason": "spam", "note": format!("  {long_note}  ")}))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{r}");
    assert_eq!(
        (r["target_type"].as_str(), r["reason"].as_str(), r["status"].as_str()),
        (Some("blog"), Some("spam"), Some("open"))
    );
    assert_eq!(r["note"].as_str().unwrap().len(), 500);
    let (s, again) = env.post("/blogs/diario/report", Some(bruno), json!({"reason": "abuse"})).await;
    assert_eq!(
        (s, again["id"].clone(), again["reason"].clone()),
        (StatusCode::CREATED, r["id"].clone(), json!("spam")),
        "la prima segnalazione resta"
    );
    // un altro lettore ne crea una propria
    let (_, other) = env.post("/blogs/diario/report", Some(carla), json!({"reason": "other"})).await;
    assert_ne!(other["id"], r["id"]);
    assert_eq!(other["note"], Value::Null);

    // regole
    assert_eq!(
        env.post("/blogs/diario/report", Some(anna), json!({"reason": "spam"})).await.0,
        StatusCode::BAD_REQUEST,
        "non il proprio blog"
    );
    assert_eq!(
        env.post("/blogs/segreto/report", Some(bruno), json!({"reason": "spam"})).await.0,
        StatusCode::NOT_FOUND,
        "blog non visibile"
    );
    assert_eq!(
        env.post("/blogs/nessuno/report", Some(bruno), json!({"reason": "spam"})).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(env.post("/blogs/diario/report", None, json!({"reason": "spam"})).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        env.post("/blogs/diario/report", Some(bruno), json!({"reason": "boh"})).await.0,
        StatusCode::BAD_REQUEST
    );

    // post
    let (s, r) = env.post(&format!("/posts/{post}/report"), Some(bruno), json!({"reason": "illegal"})).await;
    assert_eq!(
        (s, r["target_type"].clone(), r["target_id"].clone()),
        (StatusCode::CREATED, json!("post"), json!(post)),
        "{r}"
    );
    assert_eq!(
        env.post(&format!("/posts/{post}/report"), Some(anna), json!({"reason": "spam"})).await.0,
        StatusCode::BAD_REQUEST,
        "non un contenuto proprio"
    );
    assert_eq!(
        env.post(&format!("/posts/{}/report", draft["id"].as_str().unwrap()), Some(bruno), json!({"reason": "spam"}))
            .await
            .0,
        StatusCode::NOT_FOUND,
        "una bozza non si vede"
    );
    assert_eq!(
        env.post(&format!("/posts/{}/report", Uuid::new_v4()), Some(bruno), json!({"reason": "spam"})).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        env.post("/posts/non-un-uuid/report", Some(bruno), json!({"reason": "spam"})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM content_reports").fetch_one(&env.pool).await.unwrap();
    assert_eq!(total, 3);
    // un blog che diventa privato dopo la segnalazione non può più essere segnalato da chi non lo vede
    sqlx::query("UPDATE blogs SET visibility = 'private' WHERE slug = 'diario'").execute(&env.pool).await.unwrap();
    assert_eq!(
        env.post(&format!("/posts/{post}/report"), Some(carla), json!({"reason": "spam"})).await.0,
        StatusCode::NOT_FOUND
    );
    env.teardown().await;
}

#[tokio::test]
async fn report_rate_limit_is_per_user() {
    let env = setup(&["reports"], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    for i in 0..11 {
        env.blog(&format!("blog{i:02}"), anna, "public").await;
    }
    let mut last = StatusCode::CREATED;
    for i in 0..11 {
        last = env.post(&format!("/blogs/blog{i:02}/report"), Some(bruno), json!({"reason": "spam"})).await.0;
        if i < 10 {
            assert_eq!(last, StatusCode::CREATED, "{i}");
        }
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS, "l'undicesima nell'ora");
    env.teardown().await;
}

// ---- newsletter -----------------------------------------------------------------------------------------------------

mod newsletter {
    use super::*;
    use std::time::Duration;

    pub const MODULES: &[&str] = &["newsletter"];

    pub async fn wait_mail(env: &Env, count: usize) -> Vec<noct_core::mail::Email> {
        for _ in 0..200 {
            if env.mailer.sent().len() >= count {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let sent = env.mailer.sent();
        assert_eq!(sent.len(), count, "{sent:#?}");
        sent
    }

    /// Nessuna email in più nel giro di un istante.
    pub async fn settle(env: &Env, count: usize) {
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(env.mailer.sent().len(), count, "{:#?}", env.mailer.sent());
    }

    pub fn link(mail: &noct_core::mail::Email, marker: &str) -> String {
        let start = mail.text.find(marker).unwrap_or_else(|| panic!("{marker} in {}", mail.text));
        mail.text[start..].split_whitespace().next().unwrap().trim_end_matches('.').to_string()
    }

    pub fn token_of(url: &str) -> String {
        url.split_once("token=").unwrap().1.to_string()
    }

    static NEXT_IP: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

    /// Iscrizione da un IP sempre diverso: il limite per IP della rotta non interferisce con gli altri controlli.
    pub async fn subscribe(env: &Env, email: &str, blog: Option<&str>) -> (StatusCode, Value) {
        let ip = format!("203.0.113.{}", NEXT_IP.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 250 + 1);
        subscribe_from(env, &ip, email, blog).await
    }

    pub async fn subscribe_from(env: &Env, ip: &str, email: &str, blog: Option<&str>) -> (StatusCode, Value) {
        let mut body = json!({"email": email});
        if let Some(b) = blog {
            body["blog_slug"] = json!(b);
        }
        let req = axum::http::Request::builder()
            .method(axum::http::Method::POST)
            .uri("/api/v1/newsletter/subscribe")
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .header("x-forwarded-for", ip)
            .body(axum::body::Body::from(body.to_string()))
            .unwrap();
        let (s, _, j) = env.send(req).await;
        (s, j)
    }

    /// Iscrive e conferma `email`; restituisce il token di disiscrizione dalla prima campagna ricevuta più avanti.
    pub async fn confirmed(env: &Env, email: &str, blog: Option<&str>) {
        let before = env.mailer.sent().len();
        assert_eq!(subscribe(env, email, blog).await.0, StatusCode::ACCEPTED);
        let mails = wait_mail(env, before + 1).await;
        let mail = mails.last().unwrap();
        assert_eq!(mail.to, [email.to_string()]);
        let token = token_of(&link(mail, "https://"));
        let (s, b) = env.get(&format!("/newsletter/confirm?token={token}"), None).await;
        assert_eq!((s, b["status"].as_str()), (StatusCode::OK, Some("confirmed")), "{b}");
    }
}

#[tokio::test]
async fn subscription_needs_a_confirmed_double_opt_in_and_leaks_nothing() {
    let env = setup(newsletter::MODULES, &[]).await;
    let anna = env.user("anna", "utente").await;
    env.blog("diario", anna, "public").await;
    env.blog("segreto", anna, "private").await;
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();

    let (s, first) = newsletter::subscribe(&env, "Lettore@Example.com", Some("diario")).await;
    assert_eq!((s, first.clone()), (StatusCode::ACCEPTED, json!({"status": "ok"})));
    let mails = newsletter::wait_mail(&env, 1).await;
    assert_eq!(mails[0].to, ["lettore@example.com"]);
    assert!(mails[0].subject.contains("diario") || mails[0].text.contains("diario"), "{:?}", mails[0]);
    let url = newsletter::link(&mails[0], "https://notturni.eu/newsletter/confirm");
    let token = newsletter::token_of(&url);
    // il token non è nel database né nell'outbox: si rigenera dal nonce
    let leaked: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM noct_outbox WHERE payload::text LIKE $1) + (SELECT count(*) FROM newsletter_subscribers WHERE confirm_nonce::text = $1 OR consent_ip = $1)")
        .bind(format!("%{}%", token.split_once('.').unwrap().1))
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(leaked, 0);

    // finché non conferma non è un destinatario
    let stats = |status: &str| {
        let env = &env;
        let status = status.to_string();
        async move {
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM newsletter_subscribers WHERE status::text = $1")
                .bind(status)
                .fetch_one(&env.pool)
                .await
                .unwrap()
        }
    };
    assert_eq!((stats("pending").await, stats("confirmed").await), (1, 0));

    // token manomesso, di un altro iscritto o senza token
    let (s, b) = env.get(&format!("/newsletter/confirm?token={token}00"), None).await;
    assert_eq!((s, b["status"].as_str()), (StatusCode::OK, Some("invalid")));
    assert_eq!(env.get("/newsletter/confirm?token=non-un-token", None).await.1["status"], "invalid");
    assert_eq!(env.get("/newsletter/confirm", None).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    let (s, b) = env.get(&format!("/newsletter/confirm?token={token}"), None).await;
    assert_eq!((s, b["status"].as_str()), (StatusCode::OK, Some("confirmed")));
    assert_eq!(
        env.get(&format!("/newsletter/confirm?token={token}"), None).await.1["status"],
        "already_confirmed",
        "idempotente"
    );
    assert_eq!((stats("pending").await, stats("confirmed").await), (0, 1));

    // già confermato: stessa risposta, nessuna nuova email e nessun cambio di stato
    let (s, again) = newsletter::subscribe(&env, "lettore@example.com", Some("diario")).await;
    assert_eq!((s, again), (StatusCode::ACCEPTED, first));
    newsletter::settle(&env, 1).await;
    assert_eq!(stats("confirmed").await, 1);

    // blog non visibile o inesistente: 404; email non valida: 400
    assert_eq!(newsletter::subscribe(&env, "a@example.com", Some("segreto")).await.0, StatusCode::NOT_FOUND);
    assert_eq!(newsletter::subscribe(&env, "a@example.com", Some("nessuno")).await.0, StatusCode::NOT_FOUND);
    assert_eq!(newsletter::subscribe(&env, "non-una-email", None).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        env.post("/newsletter/subscribe", None, json!({"email": "a@example.com", "locale": "ita"})).await.0,
        StatusCode::BAD_REQUEST
    );
    // lo stesso indirizzo può stare nel digest e in un blog: sono liste diverse
    assert_eq!(newsletter::subscribe(&env, "lettore@example.com", None).await.0, StatusCode::ACCEPTED);
    newsletter::wait_mail(&env, 2).await;
    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn a_new_request_invalidates_the_previous_link_and_expired_links_do_not_work() {
    let env = setup(newsletter::MODULES, &[]).await;
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();
    newsletter::subscribe(&env, "lettore@example.com", None).await;
    let first = newsletter::token_of(&newsletter::link(&newsletter::wait_mail(&env, 1).await[0], "https://"));
    newsletter::subscribe(&env, "lettore@example.com", None).await;
    let second = newsletter::token_of(&newsletter::link(&newsletter::wait_mail(&env, 2).await[1], "https://"));
    assert_ne!(first, second);
    assert_eq!(env.get(&format!("/newsletter/confirm?token={first}"), None).await.1["status"], "invalid");
    // scaduto
    sqlx::query("UPDATE newsletter_subscribers SET confirm_expires_at = now() - interval '1 minute'")
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(env.get(&format!("/newsletter/confirm?token={second}"), None).await.1["status"], "invalid");
    sqlx::query("UPDATE newsletter_subscribers SET confirm_expires_at = now() + interval '1 hour'")
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(env.get(&format!("/newsletter/confirm?token={second}"), None).await.1["status"], "confirmed");
    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn subscribing_is_rate_limited_per_address() {
    let env = setup(newsletter::MODULES, &[]).await;
    for _ in 0..3 {
        assert_eq!(newsletter::subscribe(&env, "bersaglio@example.com", None).await.0, StatusCode::ACCEPTED);
    }
    assert_eq!(newsletter::subscribe(&env, "bersaglio@example.com", None).await.0, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(newsletter::subscribe(&env, "altro@example.com", None).await.0, StatusCode::ACCEPTED);
    env.teardown().await;
}

#[tokio::test]
async fn subscribing_is_rate_limited_per_ip() {
    let env = setup(newsletter::MODULES, &[]).await;
    for i in 0..5 {
        assert_eq!(
            newsletter::subscribe_from(&env, "198.51.100.7", &format!("l{i}@example.com"), None).await.0,
            StatusCode::ACCEPTED
        );
    }
    assert_eq!(
        newsletter::subscribe_from(&env, "198.51.100.7", "l9@example.com", None).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(newsletter::subscribe_from(&env, "198.51.100.8", "l9@example.com", None).await.0, StatusCode::ACCEPTED);
    env.teardown().await;
}

#[tokio::test]
async fn manual_campaign_reaches_only_confirmed_subscribers_with_footer_and_sender_name() {
    let env = setup(newsletter::MODULES, &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    let carla = env.user("carla", "utente").await;
    let blog = env.blog("diario", anna, "public").await;
    env.member(blog, bruno, "autore").await;
    env.member(blog, carla, "revisore").await;
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();
    for email in ["uno@example.com", "due@example.com", "tre@example.com"] {
        newsletter::confirmed(&env, email, Some("diario")).await;
    }
    let before = env.mailer.sent().len();
    // uno si disiscrive, un altro resta in attesa, un quarto è del digest
    sqlx::query("UPDATE newsletter_subscribers SET status = 'unsubscribed' WHERE email = 'tre@example.com'")
        .execute(&env.pool)
        .await
        .unwrap();
    newsletter::subscribe(&env, "attesa@example.com", Some("diario")).await;
    newsletter::wait_mail(&env, before + 1).await;
    newsletter::confirmed(&env, "digest@example.com", None).await;
    let before = before + 2;
    newsletter::wait_mail(&env, before).await;

    // impostazioni: nome del mittente e banner; `null` azzera, assente lascia invariato
    let (s, set) = env.patch("/blogs/diario/newsletter/settings", Some(anna), json!({"newsletter_sender_name": "  Redazione  ", "newsletter_banner_url": "https://example.com/b.png", "newsletter_banner_alt_text": "Logo"})).await;
    assert_eq!(s, StatusCode::OK, "{set}");
    assert_eq!(
        (set["newsletter_sender_name"].as_str(), set["newsletter_auto_notify_enabled"].as_bool()),
        (Some("Redazione"), Some(true))
    );
    let (_, set) =
        env.patch("/blogs/diario/newsletter/settings", Some(anna), json!({"newsletter_banner_alt_text": null})).await;
    assert_eq!(
        (set["newsletter_sender_name"].as_str(), set["newsletter_banner_alt_text"].as_str()),
        (Some("Redazione"), Some(""))
    );
    let (s, b) = env
        .patch("/blogs/diario/newsletter/settings", Some(anna), json!({"newsletter_banner_url": "javascript:alert(1)"}))
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(
        env.patch("/blogs/diario/newsletter/settings", Some(anna), json!({"newsletter_sender_name": "x".repeat(121)}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );

    // permessi: gli autori scrivono, un revisore e un estraneo no
    let (s, camp) = env
        .post(
            "/blogs/diario/newsletter/campaigns",
            Some(bruno),
            json!({"subject": "Novità", "body_markdown": "Ciao **a tutti**"}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{camp}");
    assert_eq!(
        env.post("/blogs/diario/newsletter/campaigns", Some(carla), json!({"subject": "x", "body_markdown": "y"}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(env.get("/blogs/diario/newsletter/stats", Some(carla)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(env.get("/blogs/diario/newsletter/stats", None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        env.post("/blogs/diario/newsletter/campaigns", Some(anna), json!({"subject": " ", "body_markdown": "y"}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );

    let mails = newsletter::wait_mail(&env, before + 2).await;
    let sent = &mails[before..];
    let mut to: Vec<&str> = sent.iter().map(|m| m.to[0].as_str()).collect();
    to.sort();
    assert_eq!(to, ["due@example.com", "uno@example.com"], "solo i confermati di questo blog");
    for m in sent {
        assert_eq!(m.subject, "Novità");
        assert_eq!(m.from_name.as_deref(), Some("Redazione"));
        assert!(m.text.starts_with("Ciao **a tutti**") && m.text.contains("newsletter di diario"), "{}", m.text);
        let html = m.html.as_ref().expect("versione HTML");
        assert!(html.contains("<strong>a tutti</strong>") && html.contains("https://example.com/b.png"), "{html}");
        // il link di disiscrizione funziona e riguarda proprio questo iscritto
        let unsub = newsletter::token_of(&newsletter::link(m, "https://notturni.eu/newsletter/unsubscribe"));
        assert!(html.contains(&unsub));
    }
    // stato della campagna (l'ultimo aggiornamento arriva subito dopo l'ultima email)
    for _ in 0..100 {
        if env.get("/blogs/diario/newsletter/campaigns", Some(anna)).await.1[0]["status"] == "sent" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let (_, list) = env.get("/blogs/diario/newsletter/campaigns", Some(anna)).await;
    assert_eq!(
        (list[0]["status"].as_str(), list[0]["recipient_count"].as_i64(), list[0]["kind"].as_str()),
        (Some("sent"), Some(2), Some("manual"))
    );
    assert!(list[0]["sent_at"].is_string());
    let (_, stats) = env.get("/blogs/diario/newsletter/stats", Some(anna)).await;
    assert_eq!(stats, json!({"pending": 1, "confirmed": 2, "unsubscribed": 1}));
    // il digest ha le sue statistiche e il blog non le vede
    let root = env.user("root", "super_admin").await;
    assert_eq!(
        env.get("/admin/newsletter/stats", Some(root)).await.1,
        json!({"pending": 0, "confirmed": 1, "unsubscribed": 0})
    );
    assert_eq!(env.get("/admin/newsletter/stats", Some(anna)).await.0, StatusCode::FORBIDDEN);
    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn unsubscribe_and_delete_work_from_the_email_link_without_login() {
    let env = setup(newsletter::MODULES, &[]).await;
    let root = env.user("root", "super_admin").await;
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();
    newsletter::confirmed(&env, "uno@example.com", None).await;
    newsletter::confirmed(&env, "due@example.com", None).await;
    let (s, c) = env
        .post("/admin/newsletter/campaigns", Some(root), json!({"subject": "Digest", "body_markdown": "testo"}))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{c}");
    let mails = newsletter::wait_mail(&env, 4).await;
    let uno = mails.iter().skip(2).find(|m| m.to[0] == "uno@example.com").unwrap();
    let due = mails.iter().skip(2).find(|m| m.to[0] == "due@example.com").unwrap();
    let uno_token = newsletter::token_of(&newsletter::link(uno, "https://notturni.eu/newsletter/unsubscribe"));
    let due_token = newsletter::token_of(&newsletter::link(due, "https://notturni.eu/newsletter/unsubscribe"));
    // il formato è quello della v1: `<id>.<hmac>`
    assert!(uuid::Uuid::parse_str(uno_token.split('.').next().unwrap()).is_ok());

    for bad in ["", "x.y", &format!("{}x", uno_token)] {
        assert_eq!(
            env.post("/newsletter/unsubscribe", None, json!({"token": bad})).await.0,
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
        assert_eq!(
            env.post("/newsletter/unsubscribe/delete", None, json!({"token": bad})).await.0,
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }
    let (s, b) = env.post("/newsletter/unsubscribe", None, json!({"token": uno_token, "reason": "troppe email"})).await;
    assert_eq!((s, b), (StatusCode::OK, json!({"status": "ok"})));
    let (status, reason): (String, Option<String>) = sqlx::query_as(
        "SELECT status::text, unsubscribe_reason FROM newsletter_subscribers WHERE email = 'uno@example.com'",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert_eq!((status.as_str(), reason.as_deref()), ("unsubscribed", Some("troppe email")));
    assert_eq!(
        env.post("/newsletter/unsubscribe", None, json!({"token": uno_token})).await.0,
        StatusCode::OK,
        "idempotente"
    );
    // chi si è disiscritto non riceve la campagna successiva, e il vecchio link di conferma non lo riporta in lista
    env.post("/admin/newsletter/campaigns", Some(root), json!({"subject": "Secondo", "body_markdown": "testo"})).await;
    let mails = newsletter::wait_mail(&env, 5).await;
    assert_eq!(mails[4].to, ["due@example.com"]);

    // la cancellazione elimina la riga (GDPR art. 17); un secondo uso è comunque ok
    assert_eq!(env.post("/newsletter/unsubscribe/delete", None, json!({"token": due_token})).await.0, StatusCode::OK);
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM newsletter_subscribers WHERE email = 'due@example.com'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
    assert_eq!(env.post("/newsletter/unsubscribe/delete", None, json!({"token": due_token})).await.0, StatusCode::OK);
    assert_eq!(env.post("/newsletter/unsubscribe", None, json!({"token": due_token})).await.0, StatusCode::OK);
    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn scheduled_campaigns_can_be_edited_cancelled_and_dispatched_when_due() {
    let env = setup(newsletter::MODULES, &[]).await;
    let anna = env.user("anna", "utente").await;
    env.blog("diario", anna, "public").await;
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();
    newsletter::confirmed(&env, "uno@example.com", Some("diario")).await;
    let base = "/blogs/diario/newsletter/campaigns";
    let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();

    let (s, c) = env
        .post(base, Some(anna), json!({"subject": "Domani", "body_markdown": "testo", "scheduled_at": future}))
        .await;
    assert_eq!((s, c["status"].as_str()), (StatusCode::CREATED, Some("scheduled")), "{c}");
    let id = c["id"].as_str().unwrap().to_string();
    newsletter::settle(&env, 1).await;

    // modifica: l'oggetto cambia, l'orario non c'è nel corpo → invariato
    let (s, c) = env.patch(&format!("{base}/{id}"), Some(anna), json!({"subject": "Dopodomani"})).await;
    assert_eq!(
        (s, c["subject"].as_str(), c["status"].as_str()),
        (StatusCode::OK, Some("Dopodomani"), Some("scheduled")),
        "{c}"
    );
    assert_eq!(c["scheduled_at"].as_str().map(|t| t.starts_with(&future[..10])), Some(true));
    assert_eq!(env.patch(&format!("{base}/{id}"), Some(anna), json!({"subject": ""})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        env.patch(&format!("{base}/{}", uuid::Uuid::new_v4()), Some(anna), json!({"subject": "x"})).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        env.patch(&format!("{base}/non-un-uuid"), Some(anna), json!({"subject": "x"})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // a scadenza il job la mette in invio
    sqlx::query("UPDATE newsletter_campaigns SET scheduled_at = now() - interval '1 minute'")
        .execute(&env.pool)
        .await
        .unwrap();
    let mut tx = env.pool.begin().await.unwrap();
    assert_eq!(noct_newsletter::jobs::dispatch_due_campaigns(&mut tx).await.unwrap(), 1);
    tx.commit().await.unwrap();
    let mails = newsletter::wait_mail(&env, 2).await;
    assert_eq!(mails[1].subject, "Dopodomani");
    let mut tx = env.pool.begin().await.unwrap();
    assert_eq!(
        noct_newsletter::jobs::dispatch_due_campaigns(&mut tx).await.unwrap(),
        0,
        "una campagna parte una volta sola"
    );
    tx.commit().await.unwrap();

    // una campagna inviata non si modifica né si annulla
    for req in [
        env.patch(&format!("{base}/{id}"), Some(anna), json!({"subject": "x"})).await.0,
        env.delete(&format!("{base}/{id}"), Some(anna)).await,
    ] {
        assert_eq!(req, StatusCode::CONFLICT);
    }
    // annullare una pianificata, e passarla a «subito» con scheduled_at null
    let (_, c2) =
        env.post(base, Some(anna), json!({"subject": "Annullata", "body_markdown": "t", "scheduled_at": future})).await;
    let id2 = c2["id"].as_str().unwrap().to_string();
    assert_eq!(env.delete(&format!("{base}/{id2}"), Some(anna)).await, StatusCode::OK);
    assert_eq!(env.get(base, Some(anna)).await.1[0]["status"], "canceled");
    let (_, c3) =
        env.post(base, Some(anna), json!({"subject": "Subito", "body_markdown": "t", "scheduled_at": future})).await;
    let id3 = c3["id"].as_str().unwrap().to_string();
    let (s, c3) = env.patch(&format!("{base}/{id3}"), Some(anna), json!({"scheduled_at": null})).await;
    assert_eq!((s, c3["status"].as_str()), (StatusCode::OK, Some("sending")), "{c3}");
    assert_eq!(newsletter::wait_mail(&env, 3).await[2].subject, "Subito");
    // una campagna di un altro blog o del digest non si tocca da qui
    let other = env.blog("altro", anna, "public").await;
    let _ = other;
    assert_eq!(
        env.delete(&format!("/blogs/altro/newsletter/campaigns/{id2}"), Some(anna)).await,
        StatusCode::NOT_FOUND
    );
    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn publishing_a_post_notifies_subscribers_once_unless_disabled() {
    let env = setup(newsletter::MODULES, &[]).await;
    let anna = env.user("anna", "utente").await;
    env.blog("diario", anna, "public").await;
    env.blog("muto", anna, "public").await;
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();
    newsletter::confirmed(&env, "uno@example.com", Some("diario")).await;
    newsletter::confirmed(&env, "due@example.com", Some("muto")).await;

    let (_, p) = env
        .post(
            "/blogs/diario/posts",
            Some(anna),
            json!({"slug": "primo", "title": "Il primo", "content": "Un testo abbastanza lungo da avere un estratto"}),
        )
        .await;
    let post = p["id"].as_str().unwrap().to_string();
    assert_eq!(env.post(&format!("/posts/{post}/publish"), Some(anna), json!({})).await.0, StatusCode::OK);
    let mails = newsletter::wait_mail(&env, 3).await;
    let m = &mails[2];
    assert_eq!((m.to[0].as_str(), m.subject.as_str()), ("uno@example.com", "Nuovo post su diario: Il primo"));
    assert!(
        m.text.contains("Il primo")
            && m.text.contains("Un testo abbastanza lungo")
            && m.text.contains("Leggi tutto: https://notturni.eu/diario/primo"),
        "{}",
        m.text
    );
    assert!(m.html.as_ref().unwrap().contains("href=\"https://notturni.eu/diario/primo\""));
    for _ in 0..100 {
        let done: i64 = sqlx::query_scalar("SELECT count(*) FROM newsletter_campaigns WHERE status = 'sent'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
        if done == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let (kind, status): (String, String) =
        sqlx::query_as("SELECT kind::text, status::text FROM newsletter_campaigns").fetch_one(&env.pool).await.unwrap();
    assert_eq!((kind.as_str(), status.as_str()), ("post_notification", "sent"));

    // ripubblicare lo stesso post non rinotifica
    env.post(&format!("/posts/{post}/unpublish"), Some(anna), json!({})).await;
    env.post(&format!("/posts/{post}/publish"), Some(anna), json!({})).await;
    newsletter::settle(&env, 3).await;
    let campaigns: i64 =
        sqlx::query_scalar("SELECT count(*) FROM newsletter_campaigns").fetch_one(&env.pool).await.unwrap();
    assert_eq!(campaigns, 1);

    // invio automatico spento per un blog
    assert_eq!(
        env.patch("/blogs/muto/newsletter/settings", Some(anna), json!({"newsletter_auto_notify_enabled": false}))
            .await
            .0,
        StatusCode::OK
    );
    let (_, q) =
        env.post("/blogs/muto/posts", Some(anna), json!({"slug": "zitto", "title": "Zitto", "content": "x"})).await;
    env.post(&format!("/posts/{}/publish", q["id"].as_str().unwrap()), Some(anna), json!({})).await;
    newsletter::settle(&env, 3).await;

    // un post pianificato nel futuro dà una campagna pianificata alla stessa ora
    let future = (chrono::Utc::now() + chrono::Duration::days(2)).to_rfc3339();
    let (_, r) =
        env.post("/blogs/diario/posts", Some(anna), json!({"slug": "dopo", "title": "Dopo", "content": "x"})).await;
    let (s, b) = env
        .post(&format!("/posts/{}/publish", r["id"].as_str().unwrap()), Some(anna), json!({"published_at": future}))
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    newsletter::settle(&env, 3).await;
    let (st, at): (String, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT status::text, scheduled_at FROM newsletter_campaigns WHERE subject LIKE '%Dopo'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_eq!(st, "scheduled");
    assert!(at.is_some());
    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn an_interrupted_campaign_resumes_without_sending_twice() {
    let env = setup(newsletter::MODULES, &[]).await;
    let anna = env.user("anna", "utente").await;
    env.blog("diario", anna, "public").await;
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();
    for email in ["uno@example.com", "due@example.com"] {
        newsletter::confirmed(&env, email, Some("diario")).await;
    }
    // la campagna è «in invio» e uno dei due ha già ricevuto la copia prima dell'interruzione
    let campaign: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO newsletter_campaigns (blog_id, kind, subject, body_markdown, status) SELECT id, 'manual', 'Ripresa', 'testo', 'sending' FROM blogs RETURNING id",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO newsletter_deliveries (campaign_id, subscriber_id) SELECT $1, id FROM newsletter_subscribers WHERE email = 'uno@example.com'").bind(campaign).execute(&env.pool).await.unwrap();
    let mut tx = env.pool.begin().await.unwrap();
    noct_core::bus::outbox::enqueue(
        &mut tx,
        noct_core::bus::Kind::Job,
        "newsletter.campaign.send",
        &json!({"campaign_id": campaign}),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let mails = newsletter::wait_mail(&env, 3).await;
    assert_eq!(mails[2].to, ["due@example.com"]);
    // lo stesso messaggio consegnato di nuovo dopo l'invio non rispedisce nulla
    let mut tx = env.pool.begin().await.unwrap();
    noct_core::bus::outbox::enqueue(
        &mut tx,
        noct_core::bus::Kind::Job,
        "newsletter.campaign.send",
        &json!({"campaign_id": campaign}),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    newsletter::settle(&env, 3).await;
    let (status, sent): (String, i32) =
        sqlx::query_as("SELECT status::text, recipient_count FROM newsletter_campaigns WHERE id = $1")
            .bind(campaign)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_eq!((status.as_str(), sent), ("sent", 2));
    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn campaigns_of_unreachable_blogs_or_hidden_posts_are_cancelled_not_sent() {
    let env = setup(newsletter::MODULES, &[]).await;
    let anna = env.user("anna", "utente").await;
    env.blog("diario", anna, "public").await;
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();
    newsletter::confirmed(&env, "uno@example.com", Some("diario")).await;
    sqlx::query("UPDATE blogs SET is_suspended = true").execute(&env.pool).await.unwrap();
    let campaign: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO newsletter_campaigns (blog_id, kind, subject, body_markdown, status) SELECT id, 'manual', 'Sospeso', 'testo', 'sending' FROM blogs RETURNING id",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap();
    let mut tx = env.pool.begin().await.unwrap();
    noct_core::bus::outbox::enqueue(
        &mut tx,
        noct_core::bus::Kind::Job,
        "newsletter.campaign.send",
        &json!({"campaign_id": campaign}),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    newsletter::settle(&env, 1).await;
    let status: String = sqlx::query_scalar("SELECT status::text FROM newsletter_campaigns WHERE id = $1")
        .bind(campaign)
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(status, "canceled");
    background.stop().await;
    env.teardown().await;
}

// ---- domini personali -----------------------------------------------------------------------------------------------

mod domains {
    use super::*;
    use std::{
        net::SocketAddr,
        sync::{Arc, Mutex},
    };
    use tokio::net::UdpSocket;

    /// Server DNS minimo (UDP): risponde alle query TXT con i record impostati e `NXDOMAIN` per il resto.
    pub struct FakeDns {
        pub addr: SocketAddr,
        pub records: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl FakeDns {
        pub async fn start() -> FakeDns {
            let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let addr = socket.local_addr().unwrap();
            let records: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
            let shared = records.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 512];
                loop {
                    let Ok((n, from)) = socket.recv_from(&mut buf).await else { return };
                    let query = &buf[..n];
                    // nome richiesto: sequenza di etichette dopo i 12 byte dell'intestazione
                    let (mut name, mut i) = (Vec::new(), 12);
                    while i < n && query[i] != 0 {
                        let len = query[i] as usize;
                        name.push(String::from_utf8_lossy(&query[i + 1..i + 1 + len]).to_lowercase());
                        i += len + 1;
                    }
                    let name = name.join(".");
                    let question_end = i + 1 + 4;
                    let txt = shared.lock().unwrap().iter().find(|(n2, _)| *n2 == name).map(|(_, v)| v.clone());
                    let mut reply = Vec::new();
                    reply.extend_from_slice(&query[..2]); // id
                    // QR=1, RD, RA; RCODE 0 se c'è il record, 3 (NXDOMAIN) altrimenti
                    reply.extend_from_slice(&[0x81, if txt.is_some() { 0x80 } else { 0x83 }]);
                    reply.extend_from_slice(&[0, 1, 0, if txt.is_some() { 1 } else { 0 }, 0, 0, 0, 0]);
                    reply.extend_from_slice(&query[12..question_end.min(n)]);
                    if let Some(txt) = txt {
                        reply.extend_from_slice(&[0xC0, 0x0C, 0, 16, 0, 1, 0, 0, 0, 30]); // puntatore al nome, TXT, IN, TTL
                        reply.extend_from_slice(&((txt.len() + 1) as u16).to_be_bytes());
                        reply.push(txt.len() as u8);
                        reply.extend_from_slice(txt.as_bytes());
                    }
                    let _ = socket.send_to(&reply, from).await;
                }
            });
            FakeDns { addr, records }
        }

        pub fn publish(&self, name: &str, value: &str) {
            self.records.lock().unwrap().push((name.to_string(), value.to_string()));
        }
    }

    pub async fn world(dns: &FakeDns) -> (Env, Uuid, Uuid) {
        let server = dns.addr.to_string();
        let env = setup(&["custom_domains"], &[("NOCT_DNS_SERVER", server.as_str())]).await;
        let anna = env.user("anna", "utente").await;
        let bruno = env.user("bruno", "utente").await;
        (env, anna, bruno)
    }
}

#[tokio::test]
async fn domain_is_verified_through_a_txt_record_and_shown_on_the_profile() {
    let dns = domains::FakeDns::start().await;
    let (env, anna, bruno) = domains::world(&dns).await;

    // formato e domini della piattaforma
    for (domain, needle) in [
        ("anna", "Formato"),
        ("https://anna.example.org", "Formato"),
        ("notturni.eu", "piattaforma"),
        ("blog.notturni.eu", "piattaforma"),
    ] {
        let (s, b) = env.post("/users/me/domain", Some(anna), json!({"domain": domain})).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{domain}: {b}");
        assert!(detail(&b).contains(needle), "{domain}: {b}");
    }
    assert_eq!(
        env.post("/users/me/domain", None, json!({"domain": "anna.example.org"})).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        env.post("/users/me/domain/verify", Some(anna), json!({})).await.0,
        StatusCode::NOT_FOUND,
        "nessun dominio impostato"
    );

    let (s, d) = env.post("/users/me/domain", Some(anna), json!({"domain": " Anna.Example.ORG. "})).await;
    assert_eq!(s, StatusCode::OK, "{d}");
    assert_eq!((d["domain"].as_str(), d["status"].as_str()), (Some("anna.example.org"), Some("pending")));
    assert_eq!(d["txt_record_name"], "_notturni-challenge.anna.example.org");
    let value = d["txt_record_value"].as_str().unwrap().to_string();
    assert!(value.starts_with("notturni-verify=") && value.len() > 20, "{value}");

    // il profilo privato mostra l'attesa e le istruzioni; quello pubblico non mostra ancora nulla
    let (_, me) = env.get("/users/me", Some(anna)).await;
    assert_eq!(
        (me["domain_pending_verification"].as_str(), me["domain_status"].as_str()),
        (Some("anna.example.org"), Some("pending")),
        "{me}"
    );
    assert_eq!(me["domain_verification_instructions"]["txt_record_value"], value);
    assert_eq!(env.get("/users/anna", None).await.1["custom_domain"], Value::Null);

    // record assente o sbagliato: 400 e stato `failed`; poi quello giusto verifica
    let (s, b) = env.post("/users/me/domain/verify", Some(anna), json!({})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(env.get("/users/me", Some(anna)).await.1["domain_status"], "failed");
    dns.publish("_notturni-challenge.anna.example.org", "notturni-verify=sbagliato");
    assert_eq!(env.post("/users/me/domain/verify", Some(anna), json!({})).await.0, StatusCode::BAD_REQUEST);
    dns.records.lock().unwrap().clear();
    dns.publish("_notturni-challenge.anna.example.org", &value);
    let (s, d) = env.post("/users/me/domain/verify", Some(anna), json!({})).await;
    assert_eq!((s, d["status"].as_str()), (StatusCode::OK, Some("verified")), "{d}");
    assert_eq!(env.post("/users/me/domain/verify", Some(anna), json!({})).await.1["status"], "verified", "idempotente");

    // ora il dominio compare nel profilo (pubblico e privato), dà il sigillo bronzo e risolve il profilo
    let (_, public) = env.get("/users/anna", None).await;
    assert_eq!(
        (public["custom_domain"].as_str(), public["verification_tier"].as_str()),
        (Some("anna.example.org"), Some("bronze"))
    );
    let (s, by_domain) = env.get("/users/anna.example.org", None).await;
    assert_eq!((s, by_domain["username"].as_str()), (StatusCode::OK, Some("anna")));
    let (_, me) = env.get("/users/me", Some(anna)).await;
    assert_eq!(me["domain_pending_verification"], Value::Null, "{me}");
    let audit: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE action = 'user.domain_verified'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(audit, 1);

    // un altro account non può rivendicare un dominio già verificato
    let (s, b) = env.post("/users/me/domain", Some(bruno), json!({"domain": "anna.example.org"})).await;
    assert_eq!((s, detail(&b)), (StatusCode::CONFLICT, "Dominio già rivendicato da un altro account."));

    // firmarsi con il dominio verificato; se lo si perde si torna allo username
    env.blog("diario", anna, "public").await;
    assert_eq!(
        env.patch("/users/me", Some(anna), json!({"post_author_name_style": "verified_domain"})).await.0,
        StatusCode::OK
    );
    let (_, p) =
        env.post("/blogs/diario/posts", Some(anna), json!({"slug": "uno", "title": "Uno", "content": "x"})).await;
    assert_eq!(p["author_display_name"], "anna.example.org");
    env.post(&format!("/posts/{}/publish", p["id"].as_str().unwrap()), Some(anna), json!({})).await;
    let (_, posts) = env.get("/users/anna/posts", None).await;
    assert_eq!(posts.as_array().unwrap().len(), 1, "firmato con il dominio verificato: è la sua identità pubblica");

    // rimuoverlo azzera dominio, sigillo e firma
    assert_eq!(env.delete("/users/me/domain", Some(anna)).await, StatusCode::NO_CONTENT);
    let (_, public) = env.get("/users/anna", None).await;
    assert_eq!((public["custom_domain"].clone(), public["verification_tier"].as_str()), (Value::Null, Some("none")));
    assert_eq!(env.get("/users/anna.example.org", None).await.0, StatusCode::NOT_FOUND);
    let (_, posts) = env.get("/blogs/diario/posts/uno", None).await;
    assert_eq!(posts["author_display_name"], "anna", "senza dominio verificato si torna allo username");
    assert_eq!(env.delete("/users/me/domain", Some(anna)).await, StatusCode::NO_CONTENT, "idempotente");
    env.teardown().await;
}

#[tokio::test]
async fn unverified_claims_do_not_block_a_domain_and_replacing_clears_the_old_one() {
    let dns = domains::FakeDns::start().await;
    let (env, anna, bruno) = domains::world(&dns).await;
    // bruno prenota un dominio senza verificarlo: anna lo può comunque rivendicare e verificare
    assert_eq!(
        env.post("/users/me/domain", Some(bruno), json!({"domain": "condiviso.example.org"})).await.0,
        StatusCode::OK
    );
    let (s, d) = env.post("/users/me/domain", Some(anna), json!({"domain": "condiviso.example.org"})).await;
    assert_eq!(s, StatusCode::OK, "{d}");
    let value = d["txt_record_value"].as_str().unwrap().to_string();
    dns.publish("_notturni-challenge.condiviso.example.org", &value);
    assert_eq!(env.post("/users/me/domain/verify", Some(anna), json!({})).await.0, StatusCode::OK);
    // la richiesta di bruno è sparita: non c'è più nulla da verificare e non può più contendere il dominio
    assert_eq!(env.post("/users/me/domain/verify", Some(bruno), json!({})).await.0, StatusCode::NOT_FOUND);

    // sostituire il dominio verificato con uno nuovo azzera subito quello vecchio dal profilo
    let (_, public) = env.get("/users/anna", None).await;
    assert_eq!(public["verification_tier"], "bronze");
    let (s, d) = env.post("/users/me/domain", Some(anna), json!({"domain": "nuovo.example.org"})).await;
    assert_eq!((s, d["status"].as_str()), (StatusCode::OK, Some("pending")), "{d}");
    let (_, public) = env.get("/users/anna", None).await;
    assert_eq!((public["custom_domain"].clone(), public["verification_tier"].as_str()), (Value::Null, Some("none")));
    assert_eq!(env.get("/users/condiviso.example.org", None).await.0, StatusCode::NOT_FOUND);
    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM custom_domains WHERE user_id = (SELECT id FROM users WHERE username = 'anna')",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert_eq!(rows, 1, "un dominio per utente");
    env.teardown().await;
}

#[tokio::test]
async fn domain_verification_is_rate_limited_and_unreachable_dns_is_just_a_failure() {
    // nessun server DNS in ascolto sulla porta indicata: la verifica fallisce senza errori del server
    let env = setup(&["custom_domains"], &[("NOCT_DNS_SERVER", "127.0.0.1:9"), ("NOCT_DNS_TIMEOUT_MS", "150")]).await;
    let anna = env.user("anna", "utente").await;
    env.post("/users/me/domain", Some(anna), json!({"domain": "anna.example.org"})).await;
    for _ in 0..5 {
        assert_eq!(env.post("/users/me/domain/verify", Some(anna), json!({})).await.0, StatusCode::BAD_REQUEST);
    }
    assert_eq!(env.post("/users/me/domain/verify", Some(anna), json!({})).await.0, StatusCode::TOO_MANY_REQUESTS);
    env.teardown().await;
}
