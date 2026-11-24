//! Solo mode runs on `MemoryBus` with the inline worker. What happens to pending jobs across a restart?
//!
//! Verified semantics (crates/noct-bus/src/{memory,outbox}.rs):
//! * `noct_outbox` rows are written in the domain transaction. The relay publishes them in order and sets
//!   `published_at` in its own transaction right after `Bus::publish` returns.
//! * Rows NOT yet relayed stay in the database: the next process relays them (test A).
//! * Once a row is relayed, the message lives only in the memory of the process (the consumer channel, or
//!   `pending` when no consumer is registered yet). A restart loses it: the row is already marked
//!   published, so it is never replayed (test B).
//! * A job consumed and handled before the restart is done and not repeated (test C).
//!
//! Real Postgres, no NATS. The "restart" is a second `AppState` on the same database with a new, empty bus.

mod common;

use axum::http::StatusCode;
use common::*;
use noct_core::bus::outbox::Relay;
use noct_core::{AppState, Services};
use serde_json::json;
use std::{sync::Arc, time::Duration};

/// A second "process": same database and configuration, fresh in-memory services (empty bus).
fn restart(env: &Env) -> (AppState, Arc<noct_core::mail::MemoryMailer>) {
    let (services, mailer) = Services::in_memory(&env.storage_dir);
    let cfg = env.registry.module_configs(&env.enabled, &|_| None).unwrap();
    let state = env
        .registry
        .wire(&env.enabled, AppState::new(env.pool.clone(), (*env.state.config).clone(), services, cfg))
        .unwrap();
    (state, mailer)
}

async fn unpublished(env: &Env) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM noct_outbox WHERE published_at IS NULL AND subject = 'auth.password_reset.send'",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap()
}

async fn published(env: &Env) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM noct_outbox WHERE published_at IS NOT NULL AND subject = 'auth.password_reset.send'",
    )
    .fetch_one(&env.pool)
    .await
    .unwrap()
}

/// Registers a user and requests a password reset: this enqueues one email job in the outbox.
async fn enqueue_reset_job(env: &Env) {
    let body = json!({"username": "anna", "email": "anna@example.com", "password": "una-password-lunga-1"});
    assert_eq!(env.post("/auth/register", None, body).await.0, StatusCode::CREATED);
    let (s, _) = env.post("/auth/password/forgot", None, json!({"email": "anna@example.com"})).await;
    assert_eq!(s, StatusCode::ACCEPTED);
    assert_eq!(unpublished(env).await, 1);
}

async fn wait_mail(mailer: &noct_core::mail::MemoryMailer, want: usize) -> usize {
    for _ in 0..100 {
        if mailer.sent().len() >= want {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    mailer.sent().len()
}

#[tokio::test]
async fn a_row_not_yet_relayed_survives_the_restart_and_is_processed() {
    let env = setup(&["taxonomy"], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    // first process: job written, nobody relays it (crash before the relay ran)
    enqueue_reset_job(&env).await;
    assert!(env.mailer.sent().is_empty());

    // second process with the inline worker: relay + consumers
    let (state2, mailer2) = restart(&env);
    let background = env.registry.start_background(&env.enabled, &state2).await.unwrap();
    assert_eq!(wait_mail(&mailer2, 1).await, 1, "the job written before the restart is processed");
    assert_eq!(mailer2.sent()[0].to, ["anna@example.com"]);
    assert_eq!((unpublished(&env).await, published(&env).await), (0, 1));
    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn a_message_relayed_but_not_consumed_is_lost_with_the_process() {
    let env = setup(&["taxonomy"], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    enqueue_reset_job(&env).await;

    // first process: the relay publishes into its MemoryBus, but no consumer is running yet (job waits in `pending`)
    let relay = Relay::new(env.pool.clone(), env.state.services.bus.clone());
    assert_eq!(relay.run_once().await.unwrap(), 1);
    assert_eq!((unpublished(&env).await, published(&env).await), (0, 1), "the row is already marked published");
    assert!(env.mailer.sent().is_empty());
    // the process dies here: `env.state` and its bus are never used again

    // second process: relay and consumers start, but there is nothing to relay and the memory is empty
    let (state2, mailer2) = restart(&env);
    let background = env.registry.start_background(&env.enabled, &state2).await.unwrap();
    assert_eq!(wait_mail(&mailer2, 1).await.min(1), 0, "documented limit: the in-flight job is lost, no email");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(mailer2.sent().is_empty());
    assert_eq!((unpublished(&env).await, published(&env).await), (0, 1), "the outbox row state is unchanged");
    assert!(state2.services.bus.dead_letters().await.unwrap().is_empty(), "lost silently, not dead-lettered");
    background.stop().await;
    env.teardown().await;
}

#[tokio::test]
async fn a_job_consumed_before_the_restart_is_not_repeated() {
    let env = setup(&["taxonomy"], &[("NOCT_DEPLOYMENT_MODE", "solo")]).await;
    let background = env.registry.start_background(&env.enabled, &env.state).await.unwrap();
    enqueue_reset_job_running(&env).await;
    assert_eq!(wait_mail(&env.mailer, 1).await, 1);
    background.stop().await;

    let (state2, mailer2) = restart(&env);
    let background2 = env.registry.start_background(&env.enabled, &state2).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(mailer2.sent().is_empty(), "no redelivery after the restart");
    assert_eq!((unpublished(&env).await, published(&env).await), (0, 1));
    background2.stop().await;
    env.teardown().await;
}

/// Same as `enqueue_reset_job`, but the relay is already running, so the row may be published at once.
async fn enqueue_reset_job_running(env: &Env) {
    let body = json!({"username": "anna", "email": "anna@example.com", "password": "una-password-lunga-1"});
    assert_eq!(env.post("/auth/register", None, body).await.0, StatusCode::CREATED);
    let (s, _) = env.post("/auth/password/forgot", None, json!({"email": "anna@example.com"})).await;
    assert_eq!(s, StatusCode::ACCEPTED);
}
