//! Prove end-to-end dell'autenticazione, con Postgres vero e il router reale.

mod common;

use axum::{
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use common::*;
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

const PASSWORD: &str = "una-password-lunga-1";

fn req(method: Method, path: &str, body: Option<Value>, headers: &[(&str, String)]) -> Request<Body> {
    let mut b = Request::builder().method(method).uri(format!("/api/v1{path}"));
    for (k, v) in headers {
        b = b.header(*k, v);
    }
    match body {
        Some(j) => b.header(header::CONTENT_TYPE, "application/json").body(Body::from(j.to_string())).unwrap(),
        None => b.body(Body::empty()).unwrap(),
    }
}

async fn register(env: &Env, username: &str) -> (StatusCode, Value) {
    env.post(
        "/auth/register",
        None,
        json!({"username": username, "email": format!("{username}@example.com"), "password": PASSWORD}),
    )
    .await
}

async fn login(env: &Env, email: &str, password: &str, headers: &[(&str, String)]) -> (StatusCode, HeaderMap, Value) {
    env.send(req(Method::POST, "/auth/login", Some(json!({"email": email, "password": password})), headers)).await
}

/// Cookie `Cookie:` + header CSRF come li invierebbe il browser dopo un login/refresh.
fn browser_headers(res: &HeaderMap) -> Vec<(&'static str, String)> {
    let refresh = set_cookie_value(res, "noct_refresh_token").expect("cookie di refresh");
    let csrf = set_cookie_value(res, "noct_csrf_token").expect("cookie csrf");
    vec![("cookie", format!("noct_refresh_token={refresh}; noct_csrf_token={csrf}")), ("x-csrf-token", csrf)]
}

fn bearer(token: &str) -> Vec<(&'static str, String)> {
    vec![("authorization", format!("Bearer {token}"))]
}

async fn audit_actions(env: &Env) -> Vec<String> {
    sqlx::query_scalar("SELECT action FROM audit_log ORDER BY occurred_at, id").fetch_all(&env.pool).await.unwrap()
}

// ---- registrazione ------------------------------------------------------------------

#[tokio::test]
async fn registration_validates_and_rejects_duplicates() {
    let env = setup(&["taxonomy"], &[]).await;
    let (s, u) = register(&env, "anna").await;
    assert_eq!(s, StatusCode::CREATED, "{u}");
    assert_eq!(
        (u["username"].as_str(), u["email"].as_str(), u["platform_role"].as_str(), u["mfa_enabled"].as_bool()),
        (Some("anna"), Some("anna@example.com"), Some("utente"), Some(false))
    );
    assert!(u.get("hashed_password").is_none(), "l'hash non esce mai");
    assert!(u["display_name"].is_null() && u["ui_locale"].is_null());

    // l'hash è salvato come Argon2id
    let hash: String = sqlx::query_scalar("SELECT hashed_password FROM users WHERE username = 'anna'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert!(hash.starts_with("$argon2id$"));

    // duplicati: stesso messaggio per username ed email (409)
    let (s, j) = register(&env, "anna").await;
    assert_eq!((s, detail(&j)), (StatusCode::CONFLICT, "Username o email già in uso."));
    let (s, j) = env
        .post("/auth/register", None, json!({"username": "altra", "email": "ANNA@example.com", "password": PASSWORD}))
        .await;
    assert_eq!(
        (s, detail(&j)),
        (StatusCode::CONFLICT, "Username o email già in uso."),
        "l'email si confronta senza maiuscole"
    );

    for (body, needle) in [
        (json!({"username": "Ab", "email": "a@example.com", "password": PASSWORD}), "username"),
        (json!({"username": "admin", "email": "a@example.com", "password": PASSWORD}), "riservato"),
        (json!({"username": "bruno", "email": "non-una-email", "password": PASSWORD}), "email"),
        (json!({"username": "bruno", "email": "b@example.com", "password": "corta"}), "almeno 10"),
    ] {
        let (s, j) = env.post("/auth/register", None, body).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{j}");
        assert!(detail(&j).to_lowercase().contains(needle), "{j}");
    }
    assert_eq!(env.post("/auth/register", None, json!({"username": "x"})).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    env.teardown().await;
}

#[tokio::test]
async fn registration_modes_and_solo_deployment() {
    let closed = setup(&["taxonomy"], &[("NOCT_REGISTRATION_MODE", "closed")]).await;
    let (s, j) = register(&closed, "anna").await;
    assert_eq!((s, detail(&j)), (StatusCode::FORBIDDEN, "Le registrazioni sono chiuse su questa piattaforma."));
    closed.teardown().await;

    let invite = setup(&["taxonomy"], &[("NOCT_REGISTRATION_MODE", "invite")]).await;
    assert!(detail(&register(&invite, "anna").await.1).contains("su invito"));
    invite.teardown().await;

    // "solo": il primo utente è Super Admin, poi la registrazione si chiude
    let solo = setup(&["taxonomy"], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let (s, first) = register(&solo, "anna").await;
    assert_eq!((s, first["platform_role"].as_str()), (StatusCode::CREATED, Some("super_admin")));
    let (s, j) = register(&solo, "bruno").await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert!(detail(&j).contains("singolo proprietario"));
    solo.teardown().await;
}

#[tokio::test]
async fn concurrent_solo_registrations_create_exactly_one_super_admin() {
    let env = setup(&["taxonomy"], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let names: Vec<String> = (0..6).map(|i| format!("utente{i}")).collect();
    let results = futures_join(names.iter().map(|n| register(&env, n))).await;
    assert_eq!(results.iter().filter(|(s, _)| *s == StatusCode::CREATED).count(), 1, "{results:?}");
    let admins: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE platform_role = 'super_admin'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(admins, 1);
    env.teardown().await;
}

/// Esegue le richieste in modo concorrente, intrecciando le query sul database.
async fn futures_join<F: std::future::Future>(futs: impl IntoIterator<Item = F>) -> Vec<F::Output> {
    futures::future::join_all(futs).await
}

#[tokio::test]
async fn username_availability() {
    let env = setup(&["taxonomy"], &[]).await;
    register(&env, "anna").await;
    assert_eq!(
        env.get("/auth/username-available?username=nuovo", None).await,
        (StatusCode::OK, json!({"available": true, "reason": null}))
    );
    assert_eq!(
        env.get("/auth/username-available?username=anna", None).await.1,
        json!({"available": false, "reason": "taken"})
    );
    assert_eq!(
        env.get("/auth/username-available?username=Non%20Valido", None).await.1,
        json!({"available": false, "reason": "invalid_format"})
    );
    assert_eq!(env.get("/auth/username-available", None).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    env.teardown().await;
}

// ---- login, sessione, refresh, logout ------------------------------------------------------

#[tokio::test]
async fn login_issues_access_token_and_hardened_cookies() {
    let env = setup(&["taxonomy"], &[]).await;
    register(&env, "anna").await;
    let (s, headers, body) =
        login(&env, "Anna@Example.com", PASSWORD, &[("x-forwarded-for", "203.0.113.9".into())]).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["token_type"], "bearer");
    assert!(body.get("refresh_token").is_none(), "il refresh token non è mai nel corpo JSON");
    let access = body["access_token"].as_str().unwrap();

    let refresh = set_cookie_header(&headers, "noct_refresh_token").expect("cookie di refresh");
    assert!(
        refresh.contains("HttpOnly")
            && refresh.contains("Path=/api/v1/auth")
            && refresh.contains("SameSite=Lax")
            && refresh.contains("Secure"),
        "{refresh}"
    );
    assert!(refresh.contains("Max-Age=2592000"), "{refresh}");
    let csrf = set_cookie_header(&headers, "noct_csrf_token").expect("cookie csrf");
    assert!(!csrf.contains("HttpOnly") && csrf.contains("Path=/;"), "il CSRF deve essere leggibile da JS: {csrf}");

    // il refresh token è salvato solo come hash
    let plain = set_cookie_value(&headers, "noct_refresh_token").unwrap();
    assert!(plain.starts_with("noct_sess_"));
    let stored: String =
        sqlx::query_scalar("SELECT refresh_token_hash FROM user_sessions").fetch_one(&env.pool).await.unwrap();
    assert_ne!(stored, plain);
    assert_eq!(stored, noct_core::security::token::sha256_hex(&plain));

    // l'access token apre /auth/me
    let (s, _, me) = env.send(req(Method::GET, "/auth/me", None, &bearer(access))).await;
    assert_eq!((s, me["username"].as_str()), (StatusCode::OK, Some("anna")));
    assert_eq!(env.get("/auth/me", None).await.0, StatusCode::UNAUTHORIZED);

    // audit: registrazione e login, con l'IP del client dietro il proxy fidato
    assert_eq!(audit_actions(&env).await, ["auth.register", "auth.login"]);
    let ip: Option<String> =
        sqlx::query_scalar("SELECT ip FROM audit_log WHERE action = 'auth.login'").fetch_one(&env.pool).await.unwrap();
    assert_eq!(ip.as_deref(), Some("203.0.113.9"));
    env.teardown().await;
}

#[tokio::test]
async fn failed_logins_are_indistinguishable_and_audited() {
    let env = setup(&["taxonomy"], &[]).await;
    register(&env, "anna").await;
    let wrong = login(&env, "anna@example.com", "password-sbagliata", &[]).await;
    let unknown = login(&env, "nessuno@example.com", PASSWORD, &[]).await;
    assert_eq!(wrong.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        (wrong.0, &wrong.2),
        (unknown.0, &unknown.2),
        "stessa risposta per password errata e utente inesistente"
    );
    assert_eq!(detail(&wrong.2), "Email o password non corrette.");
    assert!(set_cookie_value(&wrong.1, "noct_refresh_token").is_none());

    // account disattivato: password giusta ma accesso negato
    sqlx::query("UPDATE users SET is_active = false WHERE username = 'anna'").execute(&env.pool).await.unwrap();
    let (s, _, j) = login(&env, "anna@example.com", PASSWORD, &[]).await;
    assert_eq!((s, detail(&j)), (StatusCode::UNAUTHORIZED, "Utente disattivato."));

    // i tentativi falliti restano nell'audit anche se l'azione non è riuscita
    let failed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE action = 'auth.login_failed' AND actor_type = 'anonymous'",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert_eq!(failed, 3);
    let reasons: Vec<Value> =
        sqlx::query_scalar("SELECT payload FROM audit_log WHERE action = 'auth.login_failed' ORDER BY occurred_at")
            .fetch_all(&env.pool)
            .await
            .unwrap();
    assert_eq!(reasons[2]["reason"], "inactive");
    assert!(reasons.iter().all(|p| p.get("password").is_none()), "mai la password nell'audit");
    env.teardown().await;
}

#[tokio::test]
async fn login_is_rate_limited_per_email_and_per_ip() {
    let env = setup(&["taxonomy"], &[]).await;
    register(&env, "anna").await;
    // per email: 5 tentativi ogni 5 minuti, da IP diversi
    for i in 0..5 {
        let (s, _, _) =
            login(&env, "anna@example.com", "sbagliata-sbagliata", &[("x-forwarded-for", format!("198.51.100.{i}"))])
                .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }
    let (s, h, _) = login(&env, "anna@example.com", PASSWORD, &[("x-forwarded-for", "198.51.100.99".into())]).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS, "anche con la password giusta");
    assert!(h.get(header::RETRY_AFTER).is_some());

    // per IP: 20 tentativi, su email diverse
    for i in 0..20 {
        let (s, _, _) =
            login(&env, &format!("x{i}@example.com"), "boh-boh-boh", &[("x-forwarded-for", "192.0.2.50".into())]).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{i}");
    }
    let (s, _, _) = login(&env, "x99@example.com", "boh-boh-boh", &[("x-forwarded-for", "192.0.2.50".into())]).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    env.teardown().await;
}

#[tokio::test]
async fn client_cannot_spoof_the_ip_used_for_audit_and_limits() {
    let env = setup(&["taxonomy"], &[]).await;
    register(&env, "anna").await;
    // il client antepone un IP falso e prova a impostare l'header interno del peer
    let headers = [("x-forwarded-for", "6.6.6.6, 203.0.113.77".to_string()), ("x-noct-peer-ip", "7.7.7.7".to_string())];
    login(&env, "anna@example.com", PASSWORD, &headers).await;
    let ip: Option<String> =
        sqlx::query_scalar("SELECT ip FROM audit_log WHERE action = 'auth.login'").fetch_one(&env.pool).await.unwrap();
    assert_eq!(ip.as_deref(), Some("203.0.113.77"), "vale il valore scritto dal proxy fidato");
    // senza X-Forwarded-For l'header interno inviato dal client viene scartato
    login(&env, "anna@example.com", PASSWORD, &[("x-noct-peer-ip", "7.7.7.7".to_string())]).await;
    let ips: Vec<Option<String>> =
        sqlx::query_scalar("SELECT ip FROM audit_log WHERE action = 'auth.login' ORDER BY occurred_at")
            .fetch_all(&env.pool)
            .await
            .unwrap();
    assert_eq!(ips[1], None);
    env.teardown().await;
}

#[tokio::test]
async fn refresh_rotates_the_token_and_rejects_reuse() {
    let env = setup(&["taxonomy"], &[]).await;
    register(&env, "anna").await;
    let (_, h1, _) = login(&env, "anna@example.com", PASSWORD, &[]).await;
    let first = browser_headers(&h1);

    // senza CSRF: 403; senza cookie: 401
    let (s, _, j) = env.send(req(Method::POST, "/auth/refresh", None, &[first[0].clone()])).await;
    assert_eq!((s, detail(&j)), (StatusCode::FORBIDDEN, "CSRF token mancante o non valido."));
    let (s, _, _) = env
        .send(req(
            Method::POST,
            "/auth/refresh",
            None,
            &[("x-csrf-token", "x".into()), ("cookie", "noct_csrf_token=x".into())],
        ))
        .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    // CSRF che non coincide
    let (s, _, _) =
        env.send(req(Method::POST, "/auth/refresh", None, &[first[0].clone(), ("x-csrf-token", "altro".into())])).await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    // refresh valido: nuovo access + nuovo refresh, diverso dal precedente
    let (s, h2, body) = env.send(req(Method::POST, "/auth/refresh", None, &first)).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_ne!(set_cookie_value(&h2, "noct_refresh_token"), set_cookie_value(&h1, "noct_refresh_token"));
    let access = body["access_token"].as_str().unwrap();
    assert_eq!(env.send(req(Method::GET, "/auth/me", None, &bearer(access))).await.0, StatusCode::OK);

    // riuso del vecchio refresh token: respinto, e il cookie viene cancellato
    let (s, h3, j) = env.send(req(Method::POST, "/auth/refresh", None, &first)).await;
    assert_eq!((s, detail(&j)), (StatusCode::UNAUTHORIZED, "Refresh token non valido o revocato."));
    let cleared = set_cookie_header(&h3, "noct_refresh_token").expect("cookie azzerato");
    assert!(cleared.contains("Max-Age=0"), "{cleared}");

    // il nuovo token invece funziona
    let second = browser_headers(&h2);
    assert_eq!(env.send(req(Method::POST, "/auth/refresh", None, &second)).await.0, StatusCode::OK);
    env.teardown().await;
}

#[tokio::test]
async fn concurrent_refreshes_with_the_same_token_succeed_only_once() {
    let env = setup(&["taxonomy"], &[]).await;
    register(&env, "anna").await;
    let (_, h, _) = login(&env, "anna@example.com", PASSWORD, &[]).await;
    let headers = browser_headers(&h);
    let results = futures_join((0..5).map(|_| env.send(req(Method::POST, "/auth/refresh", None, &headers)))).await;
    let ok = results.iter().filter(|(s, _, _)| *s == StatusCode::OK).count();
    assert_eq!(ok, 1, "una sola rotazione per token: {:?}", results.iter().map(|r| r.0).collect::<Vec<_>>());
    env.teardown().await;
}

#[tokio::test]
async fn expired_or_deactivated_sessions_cannot_refresh() {
    let env = setup(&["taxonomy"], &[]).await;
    register(&env, "anna").await;
    let (_, h, _) = login(&env, "anna@example.com", PASSWORD, &[]).await;
    sqlx::query("UPDATE user_sessions SET expires_at = now() - interval '1 minute'").execute(&env.pool).await.unwrap();
    let (s, _, j) = env.send(req(Method::POST, "/auth/refresh", None, &browser_headers(&h))).await;
    assert_eq!((s, detail(&j)), (StatusCode::UNAUTHORIZED, "Refresh token scaduto."));

    let (_, h, _) = login(&env, "anna@example.com", PASSWORD, &[]).await;
    sqlx::query("UPDATE users SET is_active = false").execute(&env.pool).await.unwrap();
    let (s, _, j) = env.send(req(Method::POST, "/auth/refresh", None, &browser_headers(&h))).await;
    assert_eq!((s, detail(&j)), (StatusCode::UNAUTHORIZED, "Utente non valido."));
    env.teardown().await;
}

#[tokio::test]
async fn logout_revokes_the_session_and_clears_cookies() {
    let env = setup(&["taxonomy"], &[]).await;
    register(&env, "anna").await;
    let (_, h, _) = login(&env, "anna@example.com", PASSWORD, &[]).await;
    let headers = browser_headers(&h);
    // CSRF obbligatorio anche per il logout
    assert_eq!(env.send(req(Method::POST, "/auth/logout", None, &[headers[0].clone()])).await.0, StatusCode::FORBIDDEN);

    let (s, out, _) = env.send(req(Method::POST, "/auth/logout", None, &headers)).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert!(set_cookie_header(&out, "noct_refresh_token").unwrap().contains("Max-Age=0"));
    assert!(set_cookie_header(&out, "noct_csrf_token").unwrap().contains("Max-Age=0"));
    let (s, _, _) = env.send(req(Method::POST, "/auth/refresh", None, &headers)).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "il refresh token non è più valido");
    env.teardown().await;
}

#[tokio::test]
async fn tampered_expired_and_foreign_tokens_are_rejected() {
    let env = setup(&["taxonomy"], &[]).await;
    register(&env, "anna").await;
    let (_, _, body) = login(&env, "anna@example.com", PASSWORD, &[]).await;
    let good = body["access_token"].as_str().unwrap().to_string();
    let uid: Uuid = sqlx::query_scalar("SELECT id FROM users").fetch_one(&env.pool).await.unwrap();
    let secret = env.state.config.jwt_secret.as_bytes();
    let now = chrono::Utc::now().timestamp();

    let env_ref = &env;
    let me = |t: String| async move { env_ref.send(req(Method::GET, "/auth/me", None, &bearer(&t))).await.0 };
    assert_eq!(me(good.clone()).await, StatusCode::OK);
    assert_eq!(me(format!("{good}x")).await, StatusCode::UNAUTHORIZED, "firma alterata");
    assert_eq!(
        me(noct_core::security::jwt::encode(secret, "access", uid, now - 1000, 900)).await,
        StatusCode::UNAUTHORIZED,
        "scaduto"
    );
    assert_eq!(
        me(noct_core::security::jwt::encode(b"un-altro-segreto-un-altro-segreto-!!", "access", uid, now, 900)).await,
        StatusCode::UNAUTHORIZED,
        "segreto diverso"
    );
    assert_eq!(
        me(noct_core::security::jwt::encode(secret, "mfa_challenge", uid, now, 900)).await,
        StatusCode::UNAUTHORIZED,
        "tipo diverso"
    );
    assert_eq!(
        me(noct_core::security::jwt::encode(secret, "access", Uuid::new_v4(), now, 900)).await,
        StatusCode::UNAUTHORIZED,
        "utente inesistente"
    );
    // un access token non è un API token e viceversa
    assert_eq!(
        env.send(req(Method::GET, "/auth/me", None, &[("authorization", "Basic YTpi".into())])).await.0,
        StatusCode::UNAUTHORIZED
    );
    sqlx::query("UPDATE users SET is_active = false").execute(&env.pool).await.unwrap();
    assert_eq!(me(good).await, StatusCode::UNAUTHORIZED, "utente disattivato dopo l'emissione");
    env.teardown().await;
}

// ---- password ------------------------------------------------------------------------------

#[tokio::test]
async fn changing_the_password_revokes_every_session_and_old_access_tokens() {
    let env = setup(&["taxonomy"], &[]).await;
    register(&env, "anna").await;
    let (_, h, body) = login(&env, "anna@example.com", PASSWORD, &[]).await;
    let old_access = body["access_token"].as_str().unwrap().to_string();
    login(&env, "anna@example.com", PASSWORD, &[]).await; // una seconda sessione
    // `iat` è in secondi interi: il confronto con il cambio è al secondo
    tokio::time::sleep(Duration::from_millis(1100)).await;

    let change = |cur: &str, new: &str, token: &str| {
        env.send(req(
            Method::POST,
            "/users/me/password",
            Some(json!({"current_password": cur, "new_password": new})),
            &bearer(token),
        ))
    };
    let (s, _, j) = change("sbagliata-sbagliata", "nuova-password-123", &old_access).await;
    assert_eq!((s, detail(&j)), (StatusCode::BAD_REQUEST, "Password attuale non corretta."));
    let (s, _, j) = change(PASSWORD, "corta", &old_access).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{j}");
    let (s, _, j) = change(PASSWORD, "nuova-password-123", &old_access).await;
    assert_eq!((s, j), (StatusCode::OK, json!({"status": "ok"})));

    // access token precedente: invalidato; tutte le sessioni di refresh: cancellate
    assert_eq!(env.send(req(Method::GET, "/auth/me", None, &bearer(&old_access))).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        env.send(req(Method::POST, "/auth/refresh", None, &browser_headers(&h))).await.0,
        StatusCode::UNAUTHORIZED
    );
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM user_sessions").fetch_one(&env.pool).await.unwrap();
    assert_eq!(sessions, 0);

    // vecchia password rifiutata, nuova accettata, e il nuovo access token vale subito
    assert_eq!(login(&env, "anna@example.com", PASSWORD, &[]).await.0, StatusCode::UNAUTHORIZED);
    let (s, _, body) = login(&env, "anna@example.com", "nuova-password-123", &[]).await;
    assert_eq!(s, StatusCode::OK);
    let fresh = body["access_token"].as_str().unwrap();
    assert_eq!(env.send(req(Method::GET, "/auth/me", None, &bearer(fresh))).await.0, StatusCode::OK);
    assert!(audit_actions(&env).await.contains(&"user.password_change".to_string()));
    env.teardown().await;
}

#[tokio::test]
async fn password_change_requires_a_session_and_a_local_password() {
    let env = setup(&["taxonomy"], &[]).await;
    let body = json!({"current_password": "x", "new_password": "nuova-password-123"});
    assert_eq!(env.post("/users/me/password", None, body.clone()).await.0, StatusCode::UNAUTHORIZED);
    // utente senza password locale (SSO)
    let sso = env.user("sso", "utente").await;
    let (s, j) = env.post("/users/me/password", Some(sso), body).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(detail(&j).contains("SSO"), "{j}");
    env.teardown().await;
}

#[tokio::test]
async fn password_reset_flow_end_to_end() {
    let env = setup(&["taxonomy"], &[]).await;
    register(&env, "anna").await;
    let (_, h, _) = login(&env, "anna@example.com", PASSWORD, &[]).await;
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();

    // 202 identico per un'email registrata e per una sconosciuta
    let known = env.post("/auth/password/forgot", None, json!({"email": "Anna@example.com"})).await;
    let unknown = env.post("/auth/password/forgot", None, json!({"email": "nessuno@example.com"})).await;
    assert_eq!((known.0, unknown.0), (StatusCode::ACCEPTED, StatusCode::ACCEPTED));
    assert_eq!(known.1, unknown.1);

    // l'email parte dal job: una sola, solo all'indirizzo registrato
    let mailer = env.mailer.clone();
    for _ in 0..100 {
        if !mailer.sent().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let sent = mailer.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, ["anna@example.com"]);
    let code: String = sent[0].text.split_whitespace().skip_while(|w| *w != "è:").nth(1).unwrap().to_string();
    assert_eq!(code.len(), 6, "{}", sent[0].text);

    // il codice non compare né nel database né nell'outbox: si rigenera dall'id
    let leaked: i64 = sqlx::query_scalar("SELECT count(*) FROM noct_outbox WHERE payload::text LIKE $1")
        .bind(format!("%{code}%"))
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(leaked, 0);
    let in_codes: i64 = sqlx::query_scalar("SELECT count(*) FROM password_reset_codes WHERE id::text LIKE $1")
        .bind(format!("%{code}%"))
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(in_codes, 0);

    let reset = |email: &str, code: &str, pw: &str| {
        env.post("/auth/password/reset", None, json!({"email": email, "code": code, "new_password": pw}))
    };
    // codice sbagliato, email sconosciuta, password debole: messaggi coerenti
    let wrong = if code == "000000" { "000001" } else { "000000" };
    let (s, j) = reset("anna@example.com", wrong, "password-nuova-1").await;
    assert_eq!((s, detail(&j)), (StatusCode::BAD_REQUEST, "Codice non valido o scaduto."));
    let (s, j) = reset("nessuno@example.com", &code, "password-nuova-1").await;
    assert_eq!(
        (s, detail(&j)),
        (StatusCode::BAD_REQUEST, "Codice non valido o scaduto."),
        "stessa risposta per un'email sconosciuta"
    );
    assert_eq!(reset("anna@example.com", &code, "corta").await.0, StatusCode::BAD_REQUEST);

    // reset riuscito: 204, sessioni cancellate, login con la nuova password
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert_eq!(reset("anna@example.com", &code, "password-nuova-1").await.0, StatusCode::NO_CONTENT);
    assert_eq!(
        env.send(req(Method::POST, "/auth/refresh", None, &browser_headers(&h))).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(login(&env, "anna@example.com", PASSWORD, &[]).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(login(&env, "anna@example.com", "password-nuova-1", &[]).await.0, StatusCode::OK);
    // il codice è monouso
    let (s, _) = reset("anna@example.com", &code, "password-ancora-2").await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(audit_actions(&env).await.contains(&"auth.password_reset".to_string()));
    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn only_the_latest_reset_code_works_and_codes_expire() {
    let env = setup(&["taxonomy"], &[]).await;
    register(&env, "anna").await;
    let uid: Uuid = sqlx::query_scalar("SELECT id FROM users").fetch_one(&env.pool).await.unwrap();
    let secret = env.state.config.jwt_secret.clone();
    env.post("/auth/password/forgot", None, json!({"email": "anna@example.com"})).await;
    env.post("/auth/password/forgot", None, json!({"email": "anna@example.com"})).await;
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM password_reset_codes ORDER BY created_at")
        .fetch_all(&env.pool)
        .await
        .unwrap();
    assert_eq!(ids.len(), 2);
    let otp = |id: Uuid| noct_core::security::otp::derive_otp(secret.as_bytes(), "password_reset", id);
    let reset = |code: String| {
        env.post(
            "/auth/password/reset",
            None,
            json!({"email": "anna@example.com", "code": code, "new_password": "password-nuova-1"}),
        )
    };

    assert_eq!(reset(otp(ids[0])).await.0, StatusCode::BAD_REQUEST, "il primo codice è stato invalidato dal secondo");
    // scaduto
    sqlx::query("UPDATE password_reset_codes SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(ids[1])
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(reset(otp(ids[1])).await.0, StatusCode::BAD_REQUEST);
    sqlx::query("UPDATE password_reset_codes SET expires_at = now() + interval '5 minutes' WHERE id = $1")
        .bind(ids[1])
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(reset(otp(ids[1])).await.0, StatusCode::NO_CONTENT);
    let _ = uid;
    env.teardown().await;
}

#[tokio::test]
async fn reset_attempts_are_rate_limited_per_email() {
    let env = setup(&["taxonomy"], &[]).await;
    register(&env, "anna").await;
    env.post("/auth/password/forgot", None, json!({"email": "anna@example.com"})).await;
    for _ in 0..4 {
        let (s, _) = env
            .post(
                "/auth/password/reset",
                None,
                json!({"email": "anna@example.com", "code": "000000", "new_password": "password-nuova-1"}),
            )
            .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    }
    // 5 per email in 5 minuti (la richiesta del codice ne ha già usata una): la successiva è bloccata
    let (s, _) = env
        .post(
            "/auth/password/reset",
            None,
            json!({"email": "anna@example.com", "code": "000000", "new_password": "password-nuova-1"}),
        )
        .await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    env.teardown().await;
}

// ---- API token ----------------------------------------------------------------------------------

async fn create_token(env: &Env, bearer_token: &str, name: &str) -> (StatusCode, Value) {
    let (s, _, j) = env.send(req(Method::POST, "/tokens", Some(json!({"name": name})), &bearer(bearer_token))).await;
    (s, j)
}

#[tokio::test]
async fn api_tokens_lifecycle_for_user_owners() {
    let env = setup(&["taxonomy"], &[]).await;
    let anna = env.user("anna", "utente").await;
    let bruno = env.user("bruno", "utente").await;
    let jwt = env.access_token(anna);

    assert_eq!(env.post("/tokens", None, json!({"name": "x"})).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(create_token(&env, &jwt, "   ").await.0, StatusCode::BAD_REQUEST, "nome vuoto");

    let (s, created) = create_token(&env, &jwt, "ci").await;
    assert_eq!(s, StatusCode::CREATED, "{created}");
    let token = created["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("noct_") && !token.starts_with("noct_sess_") && token.len() > 40);
    assert_eq!(created["token_prefix"], token[..16]);
    let id = created["id"].as_str().unwrap().to_string();

    // in DB solo l'hash
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM api_tokens").fetch_one(&env.pool).await.unwrap();
    assert_eq!(stored, noct_core::security::token::sha256_hex(&token));
    assert!(!stored.contains(&token));

    // il token autentica sulle rotte `user_or_token`, ma non su quelle riservate alle sessioni
    let (s, _, list) = env.send(req(Method::GET, "/tokens", None, &bearer(&token))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert!(list[0].get("token").is_none(), "il valore in chiaro non si rilegge");
    assert_eq!((list[0]["name"].as_str(), list[0]["owner_type"].as_str()), (Some("ci"), Some("user")));
    assert!(!list[0]["last_used_at"].is_null(), "l'uso viene tracciato");
    assert_eq!(env.send(req(Method::GET, "/auth/me", None, &bearer(&token))).await.0, StatusCode::UNAUTHORIZED);

    // un altro utente non vede né revoca i token altrui
    let (_, _, theirs) = env.send(req(Method::GET, "/tokens", None, &bearer(&env.access_token(bruno)))).await;
    assert_eq!(theirs, json!([]));
    let (s, _, j) =
        env.send(req(Method::DELETE, &format!("/tokens/{id}"), None, &bearer(&env.access_token(bruno)))).await;
    assert_eq!((s, detail(&j)), (StatusCode::FORBIDDEN, "Non è possibile revocare questo token."));
    let (s, _, j) = env.send(req(Method::DELETE, &format!("/tokens/{}", Uuid::new_v4()), None, &bearer(&jwt))).await;
    assert_eq!((s, detail(&j)), (StatusCode::NOT_FOUND, "Token non trovato."));
    assert_eq!(
        env.send(req(Method::DELETE, "/tokens/non-un-uuid", None, &bearer(&jwt))).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // revoca: il token smette di funzionare ma resta in elenco, marcato
    assert_eq!(
        env.send(req(Method::DELETE, &format!("/tokens/{id}"), None, &bearer(&jwt))).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(env.send(req(Method::GET, "/tokens", None, &bearer(&token))).await.0, StatusCode::UNAUTHORIZED);
    let (_, _, list) = env.send(req(Method::GET, "/tokens", None, &bearer(&jwt))).await;
    assert!(!list[0]["revoked_at"].is_null());

    let actions = audit_actions(&env).await;
    assert!(actions.contains(&"api_token.created".to_string()) && actions.contains(&"api_token.revoked".to_string()));
    // l'audit non contiene mai il valore del token
    let leaked: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE payload::text LIKE $1")
        .bind(format!("%{token}%"))
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(leaked, 0);
    env.teardown().await;
}

#[tokio::test]
async fn expired_tokens_and_deactivated_owners_are_rejected() {
    let env = setup(&["taxonomy"], &[]).await;
    let anna = env.user("anna", "utente").await;
    let (_, created) = create_token(&env, &env.access_token(anna), "t").await;
    let token = created["token"].as_str().unwrap().to_string();
    let list = || env.send(req(Method::GET, "/tokens", None, &bearer(&token)));
    assert_eq!(list().await.0, StatusCode::OK);
    sqlx::query("UPDATE api_tokens SET expires_at = now() - interval '1 second'").execute(&env.pool).await.unwrap();
    assert_eq!(list().await.0, StatusCode::UNAUTHORIZED, "scaduto");
    sqlx::query("UPDATE api_tokens SET expires_at = NULL").execute(&env.pool).await.unwrap();
    assert_eq!(list().await.0, StatusCode::OK);
    sqlx::query("UPDATE users SET is_active = false").execute(&env.pool).await.unwrap();
    assert_eq!(list().await.0, StatusCode::UNAUTHORIZED, "proprietario disattivato");
    env.teardown().await;
}

#[tokio::test]
async fn core_tokens_are_separate_from_user_tokens() {
    let env = setup(&["taxonomy"], &[]).await;
    let anna = env.user("anna", "utente").await;
    create_token(&env, &env.access_token(anna), "di-anna").await;

    // un token core (machine-to-machine) si crea fuori dall'API, come fa `noct-ctl token create`
    let (plain, prefix, hash) = noct_core::security::token::generate_api_token();
    sqlx::query(
        "INSERT INTO api_tokens (name, owner_type, token_prefix, token_hash) VALUES ('motore', 'core', $1, $2)",
    )
    .bind(&prefix)
    .bind(&hash)
    .execute(&env.pool)
    .await
    .unwrap();

    let (s, _, list) = env.send(req(Method::GET, "/tokens", None, &bearer(&plain))).await;
    assert_eq!(s, StatusCode::OK);
    let names: Vec<_> = list.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["motore"], "il core non vede i token degli utenti");

    // emette nuovi token core, non utente
    let (s, created) = create_token(&env, &plain, "rotazione").await;
    assert_eq!(s, StatusCode::CREATED);
    let owner: String = sqlx::query_scalar("SELECT owner_type::text FROM api_tokens WHERE name = 'rotazione'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(owner, "core");
    let _ = created;
    // e non può revocare quelli di un utente
    let anna_token: Uuid =
        sqlx::query_scalar("SELECT id FROM api_tokens WHERE name = 'di-anna'").fetch_one(&env.pool).await.unwrap();
    assert_eq!(
        env.send(req(Method::DELETE, &format!("/tokens/{anna_token}"), None, &bearer(&plain))).await.0,
        StatusCode::FORBIDDEN
    );
    let actor_types: Vec<String> = sqlx::query_scalar(
        "SELECT actor_type::text FROM audit_log WHERE action = 'api_token.created' ORDER BY occurred_at",
    )
    .fetch_all(&env.pool)
    .await
    .unwrap();
    assert_eq!(actor_types, ["user", "core_token"]);
    env.teardown().await;
}

// ---- Super Admin e configurazione --------------------------------------------------------------

#[tokio::test]
async fn super_admin_is_provisioned_once_at_startup() {
    let vars = [
        ("NOCT_SUPER_ADMIN_USERNAME", "root-admin"),
        ("NOCT_SUPER_ADMIN_EMAIL", "Root@Example.com"),
        ("NOCT_SUPER_ADMIN_PASSWORD", "password-del-super-admin"),
    ];
    let env = setup(&["taxonomy"], &vars).await;
    let (role, email): (String, String) =
        sqlx::query_as("SELECT platform_role::text, email FROM users WHERE username = 'root-admin'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_eq!((role.as_str(), email.as_str()), ("super_admin", "root@example.com"));
    assert!(audit_actions(&env).await.contains(&"auth.super_admin_bootstrap".to_string()));
    let (s, _, _) = login(&env, "root@example.com", "password-del-super-admin", &[]).await;
    assert_eq!(s, StatusCode::OK);

    // idempotente: un secondo avvio non duplica né fallisce
    env.registry.run_startup(&env.enabled, &env.state).await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(&env.pool).await.unwrap();
    assert_eq!(n, 1);
    env.teardown().await;
}

#[tokio::test]
async fn invalid_provisioning_never_blocks_startup_but_bad_config_does() {
    // password debole nel provisioning: l'avvio prosegue, l'utente non viene creato
    let weak = [
        ("NOCT_SUPER_ADMIN_USERNAME", "root-admin"),
        ("NOCT_SUPER_ADMIN_EMAIL", "r@example.com"),
        ("NOCT_SUPER_ADMIN_PASSWORD", "corta"),
    ];
    let env = setup(&["taxonomy"], &weak).await;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(&env.pool).await.unwrap();
    assert_eq!(n, 0);
    env.teardown().await;

    // configurazione incoerente: l'avvio si ferma con un messaggio chiaro
    let registry = noct_core::Registry::new(noct_bundle::all()).unwrap();
    let enabled = registry.resolve_enabled(Some(&["taxonomy".to_string()])).unwrap();
    for (var, value, needle) in [
        ("NOCT_REGISTRATION_MODE", "aperta", "NOCT_REGISTRATION_MODE"),
        ("NOCT_DEPLOYMENT_MODE", "cloud", "NOCT_DEPLOYMENT_MODE"),
        ("NOCT_SESSION_COOKIE_SAMESITE", "boh", "SAMESITE"),
        ("NOCT_SESSION_COOKIE_DOMAIN", "x.com; Path=/", "cookie"),
    ] {
        let lookup = |k: &str| (k == var).then(|| value.to_string());
        let cfg = registry.module_configs(&enabled, &lookup).unwrap();
        let (services, _) = noct_core::Services::in_memory(std::env::temp_dir().join("noct-bad-config"));
        let pool =
            sqlx::postgres::PgPoolOptions::new().max_connections(1).connect_lazy("postgres://x@127.0.0.1:1/x").unwrap();
        let state = noct_core::AppState::new(pool, noct_core::Config::minimal("postgres://x"), services, cfg);
        let err = registry.run_startup(&enabled, &state).await.unwrap_err();
        assert!(err.contains(needle), "{var}: {err}");
    }
}
