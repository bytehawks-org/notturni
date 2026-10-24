//! Moduli di accesso: autenticazione a due fattori (`auth_mfa`) e accesso con provider esterni
//! (`auth_sso`).

mod common;

use axum::{
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use common::*;
use noct_auth_mfa::totp;
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

const PASSWORD: &str = "una-password-lunga-1";

async fn register(env: &Env, username: &str) -> Uuid {
    let (s, u) = env
        .post(
            "/auth/register",
            None,
            json!({"username": username, "email": format!("{username}@example.com"), "password": PASSWORD}),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{u}");
    u["id"].as_str().unwrap().parse().unwrap()
}

async fn login(env: &Env, username: &str) -> (StatusCode, HeaderMap, Value) {
    let body = json!({"email": format!("{username}@example.com"), "password": PASSWORD});
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    env.send(req).await
}

async fn verify(env: &Env, challenge: &str, code: &str) -> (StatusCode, HeaderMap, Value) {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/mfa/verify")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"challenge": challenge, "code": code}).to_string()))
        .unwrap();
    env.send(req).await
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Codice dell'app per il passo successivo: è dentro la finestra di tolleranza e non coincide con
/// quello usato per confermare la configurazione (che non si può riusare).
fn next_code(secret: &str, ahead: u64) -> String {
    totp::hotp(&totp::base32_decode(secret).unwrap(), totp::step_at(now()) + ahead)
}

async fn enable_totp(env: &Env, user: Uuid) -> String {
    let (s, setup) = env.post("/auth/mfa/totp/setup", Some(user), json!({})).await;
    assert_eq!(s, StatusCode::OK, "{setup}");
    let secret = setup["secret"].as_str().unwrap().to_string();
    let code = totp::hotp(&totp::base32_decode(&secret).unwrap(), totp::step_at(now()));
    let (s, e) = env.post("/auth/mfa/totp/confirm", Some(user), json!({"code": code})).await;
    assert_eq!(s, StatusCode::NO_CONTENT, "{e}");
    secret
}

async fn wait_for_mail(env: &Env, count: usize) -> Vec<noct_core::mail::Email> {
    for _ in 0..100 {
        if env.mailer.sent().len() >= count {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    env.mailer.sent()
}

fn code_in(email: &noct_core::mail::Email) -> String {
    email.text.split_whitespace().skip_while(|w| *w != "è:").nth(1).unwrap().to_string()
}

// ---- MFA con app TOTP ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn totp_setup_confirm_login_challenge_and_replay_protection() {
    let env = setup(&["auth_mfa"], &[]).await;
    let anna = register(&env, "anna").await;

    // senza MFA il login emette subito la sessione
    let (s, h, j) = login(&env, "anna").await;
    assert_eq!(
        (s, j["access_token"].is_string(), set_cookie_value(&h, "noct_refresh_token").is_some()),
        (StatusCode::OK, true, true)
    );

    assert_eq!(env.post("/auth/mfa/totp/setup", None, json!({})).await.0, StatusCode::UNAUTHORIZED);
    let (s, setup) = env.post("/auth/mfa/totp/setup", Some(anna), json!({})).await;
    assert_eq!(s, StatusCode::OK);
    let secret = setup["secret"].as_str().unwrap().to_string();
    assert_eq!(secret.len(), 32);
    assert!(
        setup["provisioning_uri"].as_str().unwrap().starts_with("otpauth://totp/Notturni:anna%40example.com?secret=")
    );
    assert!(setup["qr_code_data_uri"].as_str().unwrap().starts_with("data:image/svg+xml;base64,"));

    // finché non è confermato non è attivo
    assert_eq!(env.get("/auth/me", Some(anna)).await.1["mfa_enabled"], false);
    let (_, _, j) = login(&env, "anna").await;
    assert!(j["access_token"].is_string());
    assert_eq!(
        env.post("/auth/mfa/totp/confirm", Some(anna), json!({"code": "000000"})).await.0,
        StatusCode::BAD_REQUEST
    );
    let code = totp::hotp(&totp::base32_decode(&secret).unwrap(), totp::step_at(now()));
    assert_eq!(env.post("/auth/mfa/totp/confirm", Some(anna), json!({"code": code})).await.0, StatusCode::NO_CONTENT);
    assert_eq!(env.get("/auth/me", Some(anna)).await.1["mfa_enabled"], true);

    // ora il login è una sfida, senza sessione né cookie
    let (s, h, j) = login(&env, "anna").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!((j["mfa_required"].clone(), j["method"].as_str()), (json!(true), Some("totp")));
    assert!(j.get("access_token").is_none() && set_cookie_value(&h, "noct_refresh_token").is_none());
    let challenge = j["challenge"].as_str().unwrap().to_string();

    let (s, _, e) = verify(&env, &challenge, "000000").await;
    assert_eq!((s, detail(&e)), (StatusCode::UNAUTHORIZED, "Codice MFA non valido o scaduto."));
    // il codice usato per confermare non si può riusare; quello del passo dopo sì
    let (s, _, _) = verify(&env, &challenge, &code).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "stesso intervallo della conferma");
    let fresh = next_code(&secret, 1);
    let (s, h, j) = verify(&env, &challenge, &fresh).await;
    assert_eq!(s, StatusCode::OK, "{j}");
    assert!(j["access_token"].is_string() && set_cookie_value(&h, "noct_refresh_token").is_some());
    let token = j["access_token"].as_str().unwrap();
    let me = env
        .send(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(me.0, StatusCode::OK);

    // replay: lo stesso codice con una nuova sfida è respinto
    let (_, _, j) = login(&env, "anna").await;
    let (s, _, _) = verify(&env, j["challenge"].as_str().unwrap(), &fresh).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    let actions: Vec<String> =
        sqlx::query_scalar("SELECT action FROM audit_log WHERE action LIKE 'auth.%' ORDER BY occurred_at, id")
            .fetch_all(&env.pool)
            .await
            .unwrap();
    assert!(actions.contains(&"auth.mfa_enabled".to_string()) && actions.contains(&"auth.login".to_string()));
    env.teardown().await;
}

#[tokio::test]
async fn challenges_are_typed_expiring_and_tied_to_the_current_method() {
    let env = setup(&["auth_mfa"], &[]).await;
    let anna = register(&env, "anna").await;
    let secret = enable_totp(&env, anna).await;
    let (_, _, j) = login(&env, "anna").await;
    let challenge = j["challenge"].as_str().unwrap().to_string();

    for bad in ["", "abc.def.ghi", &format!("{challenge}x")] {
        assert_eq!(verify(&env, bad, "123456").await.0, StatusCode::UNAUTHORIZED, "{bad}");
    }
    // un access token non è una sfida, né una sfida un access token
    let access = env.access_token(anna);
    assert_eq!(verify(&env, &access, "123456").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(env.get(&format!("/auth/me?x={challenge}"), None).await.0, StatusCode::UNAUTHORIZED);
    let me = env
        .send(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", format!("Bearer {challenge}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(me.0, StatusCode::UNAUTHORIZED);
    // scaduta
    let expired = noct_core::security::jwt::encode_with(
        env.state.config.jwt_secret.as_bytes(),
        "mfa_challenge",
        anna,
        now() - 1000,
        300,
        serde_json::Map::from_iter([("method".to_string(), json!("totp"))]),
    );
    assert_eq!(verify(&env, &expired, &next_code(&secret, 1)).await.0, StatusCode::UNAUTHORIZED);
    // metodo cambiato dopo l'emissione: la sfida non vale più
    sqlx::query("UPDATE mfa_settings SET method = 'email'").execute(&env.pool).await.unwrap();
    assert_eq!(verify(&env, &challenge, &next_code(&secret, 1)).await.0, StatusCode::UNAUTHORIZED);
    sqlx::query("UPDATE mfa_settings SET method = 'totp'").execute(&env.pool).await.unwrap();
    // utente disattivato
    sqlx::query("UPDATE users SET is_active = false").execute(&env.pool).await.unwrap();
    assert_eq!(verify(&env, &challenge, &next_code(&secret, 1)).await.0, StatusCode::UNAUTHORIZED);
    env.teardown().await;
}

#[tokio::test]
async fn regenerating_the_secret_keeps_the_active_one_until_confirmed() {
    let env = setup(&["auth_mfa"], &[]).await;
    let anna = register(&env, "anna").await;
    let old = enable_totp(&env, anna).await;
    let (_, again) = env.post("/auth/mfa/totp/setup", Some(anna), json!({})).await;
    assert_ne!(again["secret"].as_str().unwrap(), old);
    // il login continua a funzionare con il segreto in uso
    let (_, _, j) = login(&env, "anna").await;
    let (s, _, _) = verify(&env, j["challenge"].as_str().unwrap(), &next_code(&old, 1)).await;
    assert_eq!(s, StatusCode::OK);
    // confermando il nuovo, il vecchio smette di valere
    let new = again["secret"].as_str().unwrap();
    let code = totp::hotp(&totp::base32_decode(new).unwrap(), totp::step_at(now()));
    assert_eq!(env.post("/auth/mfa/totp/confirm", Some(anna), json!({"code": code})).await.0, StatusCode::NO_CONTENT);
    let (_, _, j) = login(&env, "anna").await;
    let challenge = j["challenge"].as_str().unwrap().to_string();
    assert_eq!(verify(&env, &challenge, &next_code(&old, 0)).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(verify(&env, &challenge, &next_code(new, 1)).await.0, StatusCode::OK);
    env.teardown().await;
}

#[tokio::test]
async fn verification_attempts_are_limited_per_account() {
    let env = setup(&["auth_mfa"], &[]).await;
    let anna = register(&env, "anna").await;
    enable_totp(&env, anna).await;
    let (_, _, j) = login(&env, "anna").await;
    let challenge = j["challenge"].as_str().unwrap().to_string();
    let mut last = StatusCode::OK;
    for _ in 0..12 {
        last = verify(&env, &challenge, "000000").await.0;
        if last == StatusCode::TOO_MANY_REQUESTS {
            break;
        }
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
    env.teardown().await;
}

// ---- MFA via email ------------------------------------------------------------------------------------------------

#[tokio::test]
async fn email_mfa_codes_are_derived_single_use_and_the_latest_wins() {
    let env = setup(&["auth_mfa"], &[]).await;
    let anna = register(&env, "anna").await;
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();

    assert_eq!(env.post("/auth/mfa/email/setup", Some(anna), json!({})).await.0, StatusCode::ACCEPTED);
    let mails = wait_for_mail(&env, 1).await;
    assert_eq!((mails.len(), mails[0].to.clone()), (1, vec!["anna@example.com".to_string()]));
    let code = code_in(&mails[0]);
    assert_eq!(code.len(), 6);
    let leaked: i64 = sqlx::query_scalar("SELECT count(*) FROM noct_outbox WHERE payload::text LIKE $1")
        .bind(format!("%{code}%"))
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(leaked, 0, "il codice non passa da outbox né bus");

    let wrong = if code == "000000" { "000001" } else { "000000" };
    assert_eq!(
        env.post("/auth/mfa/email/confirm", Some(anna), json!({"code": wrong})).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(env.post("/auth/mfa/email/confirm", Some(anna), json!({"code": code})).await.0, StatusCode::NO_CONTENT);
    assert_eq!(
        env.post("/auth/mfa/email/confirm", Some(anna), json!({"code": code})).await.0,
        StatusCode::BAD_REQUEST,
        "monouso"
    );
    assert_eq!(env.get("/auth/me", Some(anna)).await.1["mfa_enabled"], true);

    // il login manda un codice nuovo; vale solo l'ultimo richiesto
    let (_, _, j) = login(&env, "anna").await;
    assert_eq!(j["method"], "email");
    let first = code_in(&wait_for_mail(&env, 2).await[1]);
    let (_, _, j2) = login(&env, "anna").await;
    let second = code_in(&wait_for_mail(&env, 3).await[2]);
    assert_ne!(first, second);
    let challenge = j2["challenge"].as_str().unwrap();
    assert_eq!(verify(&env, challenge, &first).await.0, StatusCode::UNAUTHORIZED, "codice superato da uno più recente");
    let (s, h, _) = verify(&env, challenge, &second).await;
    assert_eq!((s, set_cookie_value(&h, "noct_refresh_token").is_some()), (StatusCode::OK, true));
    assert_eq!(verify(&env, challenge, &second).await.0, StatusCode::UNAUTHORIZED, "monouso");
    let _ = j;

    // scaduto
    login(&env, "anna").await;
    let third = code_in(&wait_for_mail(&env, 4).await[3]);
    sqlx::query("UPDATE mfa_email_codes SET expires_at = now() - interval '1 minute'")
        .execute(&env.pool)
        .await
        .unwrap();
    let (_, _, j3) = login(&env, "anna").await;
    let _ = wait_for_mail(&env, 5).await;
    assert_eq!(verify(&env, j3["challenge"].as_str().unwrap(), &third).await.0, StatusCode::UNAUTHORIZED);
    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn disabling_mfa_needs_a_valid_current_code_and_restores_plain_login() {
    let env = setup(&["auth_mfa"], &[]).await;
    let anna = register(&env, "anna").await;
    let secret = enable_totp(&env, anna).await;
    assert_eq!(env.post("/auth/mfa/disable", None, json!({})).await.0, StatusCode::UNAUTHORIZED);

    // primo passo: nessun codice → 202, e il secondo fattore resta
    let (s, j) = env.post("/auth/mfa/disable", Some(anna), json!({})).await;
    assert_eq!(
        (s, j["code_required"].as_bool(), j["method"].as_str()),
        (StatusCode::ACCEPTED, Some(true), Some("totp"))
    );
    assert_eq!(env.get("/auth/me", Some(anna)).await.1["mfa_enabled"], true);
    // codice sbagliato
    assert_eq!(env.post("/auth/mfa/disable", Some(anna), json!({"code": "000000"})).await.0, StatusCode::BAD_REQUEST);
    // il codice usato per attivare l'MFA (stesso intervallo) non si riusa
    let used = next_code(&secret, 0);
    assert_eq!(env.post("/auth/mfa/disable", Some(anna), json!({"code": used})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(env.get("/auth/me", Some(anna)).await.1["mfa_enabled"], true);

    // con il codice dell'intervallo successivo si disattiva
    let next = next_code(&secret, 1);
    assert_eq!(env.post("/auth/mfa/disable", Some(anna), json!({"code": next})).await.0, StatusCode::NO_CONTENT);
    assert_eq!(env.get("/auth/me", Some(anna)).await.1["mfa_enabled"], false);
    let (_, _, j) = login(&env, "anna").await;
    assert!(j["access_token"].is_string());
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM mfa_settings").fetch_one(&env.pool).await.unwrap();
    assert_eq!(rows, 0, "il segreto non resta");
    let actions: Vec<String> =
        sqlx::query_scalar("SELECT action FROM audit_log WHERE action LIKE 'auth.mfa%' ORDER BY occurred_at, id")
            .fetch_all(&env.pool)
            .await
            .unwrap();
    assert_eq!(actions, ["auth.mfa_enabled", "auth.mfa_disabled"]);
    // già disattivata: idempotente
    assert_eq!(env.post("/auth/mfa/disable", Some(anna), json!({})).await.0, StatusCode::NO_CONTENT);
    env.teardown().await;
}

#[tokio::test]
async fn disabling_email_mfa_sends_a_code_first() {
    let env = setup(&["auth_mfa"], &[]).await;
    let anna = register(&env, "anna").await;
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();
    env.post("/auth/mfa/email/setup", Some(anna), json!({})).await;
    let code = code_in(&wait_for_mail(&env, 1).await[0]);
    assert_eq!(env.post("/auth/mfa/email/confirm", Some(anna), json!({"code": code})).await.0, StatusCode::NO_CONTENT);

    let (s, j) = env.post("/auth/mfa/disable", Some(anna), json!({})).await;
    assert_eq!((s, j["method"].as_str()), (StatusCode::ACCEPTED, Some("email")));
    let disable_code = code_in(&wait_for_mail(&env, 2).await[1]);
    assert_eq!(env.post("/auth/mfa/disable", Some(anna), json!({"code": "123456"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(env.get("/auth/me", Some(anna)).await.1["mfa_enabled"], true);
    assert_eq!(
        env.post("/auth/mfa/disable", Some(anna), json!({"code": disable_code})).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(env.get("/auth/me", Some(anna)).await.1["mfa_enabled"], false);
    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn the_code_requirement_can_be_switched_off_for_v1_behaviour() {
    let env = setup(&["auth_mfa"], &[("NOCT_MFA_DISABLE_REQUIRES_CODE", "false")]).await;
    let anna = register(&env, "anna").await;
    enable_totp(&env, anna).await;
    assert_eq!(env.post("/auth/mfa/disable", Some(anna), json!({})).await.0, StatusCode::NO_CONTENT);
    assert_eq!(env.get("/auth/me", Some(anna)).await.1["mfa_enabled"], false);
    env.teardown().await;
}

#[tokio::test]
async fn totp_secrets_are_encrypted_at_rest_and_plaintext_rows_still_work() {
    let env = setup(&["auth_mfa"], &[]).await;
    let anna = register(&env, "anna").await;
    let secret = enable_totp(&env, anna).await;
    let stored: String = sqlx::query_scalar("SELECT totp_secret FROM mfa_settings").fetch_one(&env.pool).await.unwrap();
    assert!(stored.starts_with("enc:v1:") && !stored.contains(&secret), "{stored}");
    // il login con il codice funziona sul valore cifrato
    let (_, _, j) = login(&env, "anna").await;
    let challenge = j["challenge"].as_str().unwrap().to_string();
    assert_eq!(verify(&env, &challenge, &next_code(&secret, 1)).await.0, StatusCode::OK);
    // un valore in chiaro scritto prima della cifratura si legge ancora
    sqlx::query("UPDATE mfa_settings SET totp_secret = $1, totp_last_step = NULL")
        .bind(&secret)
        .execute(&env.pool)
        .await
        .unwrap();
    let (_, _, j) = login(&env, "anna").await;
    let challenge = j["challenge"].as_str().unwrap().to_string();
    assert_eq!(verify(&env, &challenge, &next_code(&secret, 0)).await.0, StatusCode::OK);
    // una chiave diversa non decifra: errore del server, mai un accesso
    let other = setup(&["auth_mfa"], &[("NOCT_SECRETS_KEY", "un'altra-chiave-lunga-e-casuale-0123456789")]).await;
    let bruno = register(&other, "bruno").await;
    let secret2 = enable_totp(&other, bruno).await;
    sqlx::query("UPDATE mfa_settings SET totp_secret = $1").bind(stored).execute(&other.pool).await.unwrap();
    let (_, _, j) = login(&other, "bruno").await;
    let (s, _, _) = verify(&other, j["challenge"].as_str().unwrap(), &next_code(&secret2, 1)).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    other.teardown().await;
    env.teardown().await;
}

#[tokio::test]
async fn mfa_routes_do_not_exist_when_the_module_is_disabled() {
    let env = setup(&[], &[]).await;
    let anna = register(&env, "anna").await;
    assert_eq!(env.post("/auth/mfa/totp/setup", Some(anna), json!({})).await.0, StatusCode::NOT_FOUND);
    let (_, _, j) = login(&env, "anna").await;
    assert!(j["access_token"].is_string());
    env.teardown().await;
}

// ---- SSO ----------------------------------------------------------------------------------------------------------

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// Provider OAuth finto. `/…/token` accetta i codici `good-<id>` (e pretende il verifier PKCE),
/// `/…/userinfo` restituisce il profilo registrato per `<id>`, `/github/emails` la lista delle email.
#[derive(Clone, Default)]
struct Mock {
    profiles: Arc<Mutex<HashMap<String, Value>>>,
    emails: Arc<Mutex<HashMap<String, Value>>>,
    tokens_requested: Arc<Mutex<Vec<String>>>,
}

impl Mock {
    fn profile(&self, id: &str, profile: Value) {
        self.profiles.lock().unwrap().insert(id.to_string(), profile);
    }
    fn emails(&self, id: &str, emails: Value) {
        self.emails.lock().unwrap().insert(id.to_string(), emails);
    }

    async fn serve(&self) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let mock = self.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else { return };
                let mock = mock.clone();
                tokio::spawn(async move {
                    let mut data = Vec::new();
                    let mut buf = vec![0u8; 8192];
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
                    let text = String::from_utf8_lossy(&data).to_string();
                    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
                    let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
                    let bearer = head
                        .lines()
                        .find_map(|l| {
                            l.to_lowercase()
                                .starts_with("authorization:")
                                .then(|| l.split_whitespace().last().unwrap_or("").to_string())
                        })
                        .unwrap_or_default();
                    let (status, json) = if path.ends_with("/token") {
                        let field = |k: &str| {
                            body.split('&').find_map(|p| p.strip_prefix(&format!("{k}=")).map(str::to_string))
                        };
                        match (field("code"), field("code_verifier"), field("client_secret")) {
                            (Some(code), Some(v), Some(secret))
                                if code.starts_with("good-") && v.len() >= 43 && secret == "s3cret" =>
                            {
                                mock.tokens_requested.lock().unwrap().push(code.clone());
                                (
                                    "200 OK",
                                    json!({"access_token": format!("tok-{}", &code[5..]), "token_type": "bearer"}),
                                )
                            }
                            _ => ("400 Bad Request", json!({"error": "invalid_grant"})),
                        }
                    } else if let Some(id) = bearer.strip_prefix("tok-") {
                        let store = if path.ends_with("/emails") { &mock.emails } else { &mock.profiles };
                        match store.lock().unwrap().get(id) {
                            Some(v) => ("200 OK", v.clone()),
                            None => ("404 Not Found", json!({})),
                        }
                    } else {
                        ("401 Unauthorized", json!({}))
                    };
                    let body = json.to_string();
                    let resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                });
            }
        });
        base
    }
}

async fn sso_env(extra: &[(&str, &str)]) -> (Env, Mock) {
    let mock = Mock::default();
    let base = mock.serve().await;
    let mut vars = vec![
        ("NOCT_OAUTH_ENDPOINT_BASE", base.as_str()),
        ("NOCT_OAUTH_GOOGLE_CLIENT_ID", "gid"),
        ("NOCT_OAUTH_GOOGLE_CLIENT_SECRET", "s3cret"),
        ("NOCT_OAUTH_GITHUB_CLIENT_ID", "ghid"),
        ("NOCT_OAUTH_GITHUB_CLIENT_SECRET", "s3cret"),
        ("NOCT_FRONTEND_URL", "http://front.test"),
        ("NOCT_OAUTH_REDIRECT_BASE_URL", "http://api.test"),
    ];
    vars.extend_from_slice(extra);
    (setup(&["auth_sso", "auth_mfa"], &vars).await, mock)
}

fn location(h: &HeaderMap) -> String {
    h.get(header::LOCATION).map(|v| v.to_str().unwrap().to_string()).unwrap_or_default()
}

/// Avvia il flusso come farebbe il browser: restituisce `(state, valore del cookie)`.
async fn sso_start(env: &Env, provider: &str) -> (String, String) {
    let req = Request::builder().uri(format!("/api/v1/auth/sso/{provider}/login")).body(Body::empty()).unwrap();
    let (s, h, _) = env.send(req).await;
    assert_eq!(s, StatusCode::SEE_OTHER);
    let loc = location(&h);
    let state = loc.split(['?', '&']).find_map(|p| p.strip_prefix("state=")).unwrap().to_string();
    let cookie = set_cookie_value(&h, "noct_sso_state").expect("cookie di stato");
    (state, cookie)
}

async fn sso_callback(env: &Env, provider: &str, query: &str, cookie: Option<&str>) -> (StatusCode, HeaderMap) {
    let mut b = Request::builder().uri(format!("/api/v1/auth/sso/{provider}/callback?{query}"));
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, format!("noct_sso_state={c}"));
    }
    let (s, h, _) = env.send(b.body(Body::empty()).unwrap()).await;
    (s, h)
}

/// Flusso completo con un profilo del provider.
async fn sso_flow(env: &Env, mock: &Mock, provider: &str, id: &str, profile: Value) -> (StatusCode, HeaderMap) {
    mock.profile(id, profile);
    let (state, cookie) = sso_start(env, provider).await;
    sso_callback(env, provider, &format!("code=good-{id}&state={state}"), Some(&cookie)).await
}

fn oidc(sub: &str, email: &str, verified: bool) -> Value {
    json!({"sub": sub, "email": email, "email_verified": verified})
}

async fn user_count(env: &Env) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(&env.pool).await.unwrap()
}

#[tokio::test]
async fn sso_login_redirects_with_state_and_pkce_bound_to_the_browser() {
    let (env, _) = sso_env(&[]).await;
    let req = Request::builder().uri("/api/v1/auth/sso/google/login").body(Body::empty()).unwrap();
    let (s, h, _) = env.send(req).await;
    assert_eq!(s, StatusCode::SEE_OTHER);
    let loc = location(&h);
    assert!(loc.contains("/google/authorize?response_type=code&client_id=gid&redirect_uri=http%3A%2F%2Fapi.test%2Fapi%2Fv1%2Fauth%2Fsso%2Fgoogle%2Fcallback"), "{loc}");
    assert!(loc.contains("code_challenge_method=S256") && loc.contains("&state="));
    let cookie = h
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .find(|c| c.starts_with("noct_sso_state="))
        .unwrap();
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("SameSite=Lax") && cookie.contains("Path=/api/v1/auth/sso"),
        "{cookie}"
    );
    // due avvii non condividono lo state
    assert_ne!(sso_start(&env, "google").await.0, sso_start(&env, "google").await.0);

    // provider sconosciuto o senza credenziali: 503
    for p in ["facebook", "microsoft", "linkedin"] {
        let req = Request::builder().uri(format!("/api/v1/auth/sso/{p}/login")).body(Body::empty()).unwrap();
        assert_eq!(env.send(req).await.0, StatusCode::SERVICE_UNAVAILABLE, "{p}");
    }
    // provider spento dall'amministratore
    sqlx::query("INSERT INTO sso_settings (id, enabled_providers) VALUES (1, '{github}')")
        .execute(&env.pool)
        .await
        .unwrap();
    let req = Request::builder().uri("/api/v1/auth/sso/google/login").body(Body::empty()).unwrap();
    assert_eq!(env.send(req).await.0, StatusCode::FORBIDDEN);
    env.teardown().await;
}

#[tokio::test]
async fn sso_creates_a_user_then_recognizes_the_identity() {
    let (env, mock) = sso_env(&[]).await;
    let (s, h) = sso_flow(&env, &mock, "google", "g1", oidc("g-1", "Anna.Rossi@Example.com", true)).await;
    assert_eq!((s, location(&h).as_str()), (StatusCode::SEE_OTHER, "http://front.test/dashboard"));
    assert!(set_cookie_value(&h, "noct_refresh_token").is_some() && set_cookie_value(&h, "noct_csrf_token").is_some());
    let (username, email, hash, role): (String, String, Option<String>, String) =
        sqlx::query_as("SELECT username, email, hashed_password, platform_role::text FROM users")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_eq!(
        (username.as_str(), email.as_str(), hash, role.as_str()),
        ("annarossi", "anna.rossi@example.com", None, "utente")
    );

    // di nuovo: stesso utente, nessun duplicato
    let (s, h) = sso_flow(&env, &mock, "google", "g1", oidc("g-1", "cambiata@example.com", true)).await;
    assert_eq!((s, location(&h).as_str()), (StatusCode::SEE_OTHER, "http://front.test/dashboard"));
    assert_eq!(user_count(&env).await, 1);
    let actions: Vec<String> =
        sqlx::query_scalar("SELECT action FROM audit_log ORDER BY occurred_at, id").fetch_all(&env.pool).await.unwrap();
    assert_eq!(actions, ["auth.register", "auth.login", "auth.login"]);

    // l'username si adatta se è occupato o riservato
    let (_, _) = sso_flow(&env, &mock, "google", "g2", oidc("g-2", "annarossi@altro.org", true)).await;
    let (_, _) = sso_flow(&env, &mock, "google", "g3", oidc("g-3", "admin@altro.org", true)).await;
    let mut names: Vec<String> = sqlx::query_scalar("SELECT username FROM users").fetch_all(&env.pool).await.unwrap();
    names.sort();
    assert_eq!(names, ["admin1", "annarossi", "annarossi1"]);
    env.teardown().await;
}

#[tokio::test]
async fn forged_or_broken_callbacks_create_nothing() {
    let (env, mock) = sso_env(&[]).await;
    mock.profile("x", oidc("x", "x@example.com", true));
    let (state, cookie) = sso_start(&env, "google").await;
    let denied = |h: &HeaderMap| location(h) == "http://front.test/login?sso_error=1";

    for (query, cookie) in [
        (format!("code=good-x&state={state}"), None), // senza cookie: callback forgiato
        ("code=good-x&state=altro".to_string(), Some(cookie.as_str())), // state diverso
        ("code=good-x".to_string(), Some(cookie.as_str())), // senza state
        (format!("state={state}"), Some(cookie.as_str())), // senza codice
        (format!("code=cattivo&state={state}"), Some(cookie.as_str())), // il provider rifiuta il codice
        (format!("code=good-x&state={state}"), Some("solo-stato-senza-verifier")), // cookie malformato
    ] {
        let (s, h) = sso_callback(&env, "google", &query, cookie).await;
        assert_eq!(s, StatusCode::SEE_OTHER, "{query}");
        assert!(denied(&h), "{query}: {}", location(&h));
        assert!(set_cookie_value(&h, "noct_refresh_token").is_none());
    }
    // provider non configurato o inesistente
    let (s, h) = sso_callback(&env, "microsoft", "code=a&state=b", None).await;
    assert!((s, denied(&h)) == (StatusCode::SEE_OTHER, true));
    // profilo senza email
    mock.profile("n", json!({"sub": "n"}));
    let (state, cookie) = sso_start(&env, "google").await;
    let (_, h) = sso_callback(&env, "google", &format!("code=good-n&state={state}"), Some(&cookie)).await;
    assert!(denied(&h));
    assert_eq!(user_count(&env).await, 0);
    env.teardown().await;
}

#[tokio::test]
async fn existing_accounts_link_only_with_a_provider_verified_email() {
    let (env, mock) = sso_env(&[]).await;
    let anna = register(&env, "anna").await;

    // email non verificata dal provider: niente collegamento (altrimenti si prende un account altrui)
    let (_, h) = sso_flow(&env, &mock, "google", "u1", oidc("g-9", "anna@example.com", false)).await;
    assert_eq!(location(&h), "http://front.test/login?sso_error=1");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM sso_identities").fetch_one(&env.pool).await.unwrap();
    assert_eq!(n, 0);

    // verificata: collegata, e il login è quello dell'utente esistente
    let (_, h) = sso_flow(&env, &mock, "google", "u2", oidc("g-9", "anna@example.com", true)).await;
    assert_eq!(location(&h), "http://front.test/dashboard");
    let owner: Uuid = sqlx::query_scalar("SELECT user_id FROM sso_identities").fetch_one(&env.pool).await.unwrap();
    assert_eq!((owner, user_count(&env).await), (anna, 1));
    // la password resta quella di prima
    let (_, _, j) = login(&env, "anna").await;
    assert!(j["access_token"].is_string());

    // un utente disattivato non entra, né da identità nota né da email
    sqlx::query("UPDATE users SET is_active = false").execute(&env.pool).await.unwrap();
    let (_, h) = sso_flow(&env, &mock, "google", "u3", oidc("g-9", "anna@example.com", true)).await;
    assert_eq!(location(&h), "http://front.test/login?sso_error=1");
    env.teardown().await;
}

#[tokio::test]
async fn linking_an_account_with_mfa_waits_for_the_second_factor() {
    let (env, mock) = sso_env(&[]).await;
    let anna = register(&env, "anna").await;
    let secret = enable_totp(&env, anna).await;

    let (s, h) = sso_flow(&env, &mock, "google", "m1", oidc("g-5", "anna@example.com", true)).await;
    assert_eq!(s, StatusCode::SEE_OTHER);
    let loc = location(&h);
    assert!(loc.starts_with("http://front.test/login?mfa_challenge=") && loc.ends_with("&mfa_method=totp"), "{loc}");
    assert!(set_cookie_value(&h, "noct_refresh_token").is_none(), "nessuna sessione prima del secondo fattore");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM sso_identities").fetch_one(&env.pool).await.unwrap();
    assert_eq!(n, 0, "nemmeno il collegamento");

    let challenge = loc.split("mfa_challenge=").nth(1).unwrap().split('&').next().unwrap().to_string();
    let (s, _, _) = verify(&env, &challenge, "000000").await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM sso_identities").fetch_one(&env.pool).await.unwrap();
    assert_eq!(n, 0, "un codice sbagliato non collega");

    let (s, h, j) = verify(&env, &challenge, &next_code(&secret, 1)).await;
    assert_eq!(
        (s, j["access_token"].is_string(), set_cookie_value(&h, "noct_refresh_token").is_some()),
        (StatusCode::OK, true, true),
        "{j}"
    );
    let linked: Uuid = sqlx::query_scalar("SELECT user_id FROM sso_identities WHERE provider_user_id = 'g-5'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(linked, anna);

    // da ora l'identità è nota: il prossimo accesso SSO entra direttamente
    let (_, h) = sso_flow(&env, &mock, "google", "m2", oidc("g-5", "anna@example.com", true)).await;
    assert_eq!(location(&h), "http://front.test/dashboard");
    env.teardown().await;
}

#[tokio::test]
async fn github_uses_the_primary_verified_email_and_registrations_must_be_open() {
    let (env, mock) = sso_env(&[]).await;
    mock.profile("h1", json!({"id": 777, "email": "pubblica@example.com"}));
    mock.emails("h1", json!([{"email": "privata@example.com", "primary": true, "verified": true}, {"email": "pubblica@example.com", "primary": false, "verified": true}]));
    let (state, cookie) = sso_start(&env, "github").await;
    let (_, h) = sso_callback(&env, "github", &format!("code=good-h1&state={state}"), Some(&cookie)).await;
    assert_eq!(location(&h), "http://front.test/dashboard");
    let email: String = sqlx::query_scalar("SELECT email FROM users").fetch_one(&env.pool).await.unwrap();
    assert_eq!(email, "privata@example.com", "l'email pubblica del profilo è libera: non si usa");

    mock.profile("h2", json!({"id": 778}));
    mock.emails("h2", json!([{"email": "nuova@example.com", "primary": true, "verified": false}]));
    let (state, cookie) = sso_start(&env, "github").await;
    let (_, h) = sso_callback(&env, "github", &format!("code=good-h2&state={state}"), Some(&cookie)).await;
    assert_eq!(location(&h), "http://front.test/login?sso_error=1");
    env.teardown().await;

    // registrazioni chiuse: l'SSO non è una porta secondaria per creare account
    let (env, mock) = sso_env(&[("NOCT_REGISTRATION_MODE", "closed")]).await;
    let (_, h) = sso_flow(&env, &mock, "google", "c1", oidc("g-1", "nuova@example.com", true)).await;
    assert_eq!(location(&h), "http://front.test/login?sso_error=1");
    assert_eq!(user_count(&env).await, 0);
    env.teardown().await;
}

#[tokio::test]
async fn sso_routes_do_not_exist_when_the_module_is_disabled() {
    let env = setup(&[], &[]).await;
    let req = Request::builder().uri("/api/v1/auth/sso/google/login").body(Body::empty()).unwrap();
    assert_eq!(env.send(req).await.0, StatusCode::NOT_FOUND);
    env.teardown().await;
}
