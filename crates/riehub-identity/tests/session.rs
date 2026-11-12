//! The session acceptance tests (identity spec, section 3).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use riehub_core::{Secret, UserId};
use riehub_identity::session::SessionCrypto;
use riehub_identity::Service;
use riehub_store::sqlite::SqliteStore;
use riehub_store::Store;
use std::sync::Arc;

fn temp_url(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "riehub-session-{}-{}",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    format!("sqlite://{}/riehub.db", dir.display())
}

async fn service(tag: &str) -> (Arc<Service>, Arc<SqliteStore>, Arc<riehub_core::TestClock>) {
    let store = SqliteStore::open(&temp_url(tag)).await.unwrap();
    store.migrate().await.unwrap();
    let store = Arc::new(store);
    let mac = riehub_audit::AuditChainMac::new(riehub_crypto::Key32::new([7u8; 32]));
    let audit = Arc::new(riehub_audit::AuditLog::new(store.clone(), mac));
    let clock = Arc::new(riehub_core::TestClock::new(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
    ));
    let service = Service::new(store.clone(), audit).with_test_clock(clock.clone());
    (Arc::new(service), store, clock)
}

const COOKIE_KEY: &[u8; 32] = &[1u8; 32];

async fn logged_in(service: &Service) -> (UserId, riehub_identity::session::Session) {
    service
        .setup_owner(
            "s@example.com",
            &Secret::new("a-very-good-passphrase-42".to_owned()),
            None,
        )
        .await
        .unwrap();
    service
        .login(
            &Secret::new("s@example.com".to_owned()),
            &Secret::new("a-very-good-passphrase-42".to_owned()),
            None,
            COOKIE_KEY,
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn idle_timeout_ends_session() {
    let (service, _store, clock) = service("idle").await;
    let (_user, session) = logged_in(&service).await;
    service.validate_session(&session.token).await.unwrap();

    clock.advance(riehub_identity::session::IDLE_TIMEOUT + 1_000);
    let result = service.validate_session(&session.token).await;
    assert!(result.is_err(), "an idle session must end");
}

#[tokio::test]
async fn absolute_timeout_ends_session() {
    let (service, _store, clock) = service("absolute").await;
    let (_user, session) = logged_in(&service).await;

    // Activity keeps the session inside the idle window, but the
    // absolute lifetime still ends it.
    for _ in 0..5 {
        clock.advance(10 * 60_000);
        service.validate_session(&session.token).await.unwrap();
    }
    clock.advance(riehub_identity::session::ABSOLUTE_TIMEOUT);
    let result = service.validate_session(&session.token).await;
    assert!(result.is_err(), "an absolute expired session must end");
}

#[tokio::test]
async fn stolen_session_record_from_db_cannot_unlock() {
    let (service, store, _clock) = service("stolen").await;
    let (user, session) = logged_in(&service).await;

    // The attacker reads the session record from the database and
    // rebuilds a token around the stored verifier.
    let record = store
        .get_by_lookup(
            &riehub_core::Owner::Instance,
            riehub_store::record::Kind::Session,
            &session.sid,
        )
        .await
        .unwrap()
        .unwrap();
    let verifier_bytes = record.data.unwrap();
    let mut verifier = [0u8; 32];
    verifier.copy_from_slice(&verifier_bytes);

    // The verifier is hkdf(secret, token/auth), the slot key is
    // hkdf(secret, token/slot): one does not give the other.
    let slot_key = riehub_crypto::Key32::new(verifier);
    let vault = riehub_vault::load(store.as_ref(), &user)
        .await
        .unwrap()
        .unwrap();
    let result = riehub_vault::unlock(
        &vault,
        &format!("ws.{}", session.sid),
        slot_key.expose(),
        service.as_ref(),
    )
    .await;
    assert!(
        result.is_err(),
        "a token built from the stored verifier must not unlock"
    );
}

#[tokio::test]
async fn unlock_cache_evicts_after_idle() {
    let (service, _store, clock) = service("cache").await;
    let (_user, session) = logged_in(&service).await;
    service.validate_session(&session.token).await.unwrap();
    assert_eq!(service.unlock_cache_len(), 1);

    clock.advance(riehub_identity::session::IDLE_TIMEOUT / 2);
    service.validate_session(&session.token).await.unwrap();
    assert_eq!(service.unlock_cache_len(), 1, "recent use keeps the entry");

    clock.advance(riehub_identity::session::IDLE_TIMEOUT + 1_000);
    // After the idle window the old entry has been evicted and the
    // session itself is gone; a fresh login repopulates the cache.
    let _ = service.validate_session(&session.token).await;
    let (_user2, session2) = {
        service
            .login(
                &Secret::new("s@example.com".to_owned()),
                &Secret::new("a-very-good-passphrase-42".to_owned()),
                None,
                COOKIE_KEY,
            )
            .await
            .unwrap()
    };
    service.validate_session(&session2.token).await.unwrap();
    assert_eq!(
        service.unlock_cache_len(),
        1,
        "the cache holds only live entries"
    );
}

#[tokio::test]
async fn eleventh_session_ends_the_oldest() {
    let (service, store, _clock) = service("cap").await;
    let (user, first_session) = logged_in(&service).await;

    // Ten more logins: the first session must be the one that ends.
    for _ in 0..10 {
        service
            .login(
                &Secret::new("s@example.com".to_owned()),
                &Secret::new("a-very-good-passphrase-42".to_owned()),
                None,
                COOKIE_KEY,
            )
            .await
            .unwrap();
    }
    let sessions = service.list_sessions(&user).await.unwrap();
    assert_eq!(sessions.len(), 10, "the cap holds");
    assert!(
        !sessions.iter().any(|s| s.sid == first_session.sid),
        "the oldest session is gone"
    );
    let first_record = store
        .get_by_lookup(
            &riehub_core::Owner::Instance,
            riehub_store::record::Kind::Session,
            &first_session.sid,
        )
        .await
        .unwrap();
    assert!(
        first_record.is_none(),
        "the ended session record is deleted"
    );
}

#[tokio::test]
async fn the_session_verifier_check_rejects_a_bad_secret() {
    let (service, _store, _clock) = service("badsecret").await;
    let (_user, session) = logged_in(&service).await;
    // Tamper with the secret part of the token.
    let parts: Vec<&str> = session.token.split('_').collect();
    let bad_token = format!("rs_{}_{}", parts[1], "A".repeat(43));
    let result = service.validate_session(&bad_token).await;
    assert!(result.is_err(), "a forged secret must fail");
    let _ = SessionCrypto::verifier(&[0u8; 32]);
}
