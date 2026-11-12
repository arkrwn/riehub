//! The invite acceptance tests (identity spec, section 1).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use riehub_core::{Secret, UserId};
use riehub_identity::Service;
use riehub_store::sqlite::SqliteStore;
use riehub_store::Store;
use std::sync::Arc;

fn temp_url(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "riehub-invite-{}-{}",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    format!("sqlite://{}/riehub.db", dir.display())
}

async fn service(tag: &str) -> Arc<Service> {
    let store = Arc::new(SqliteStore::open(&temp_url(tag)).await.unwrap());
    store.migrate().await.unwrap();
    let mac = riehub_audit::AuditChainMac::new(riehub_crypto::Key32::new([7u8; 32]));
    let audit = Arc::new(riehub_audit::AuditLog::new(store.clone(), mac));
    Arc::new(Service::new(store, audit))
}

#[tokio::test]
async fn invite_is_single_use() {
    let service = service("single").await;
    let (_user, token) = service
        .create_invite("i@example.com", Some("Invited"))
        .await
        .unwrap();
    let (user_id, _recovery) = service
        .accept_invite(&token, &Secret::new("a-very-good-passphrase-42".to_owned()))
        .await
        .unwrap();
    assert!(!user_id.as_str().is_empty());
    let second = service
        .accept_invite(&token, &Secret::new("another-good-passphrase-3".to_owned()))
        .await;
    assert!(second.is_err(), "an invite must be single use");
    let info = service.invite_by_token(&token).await;
    assert!(info.is_err(), "the accepted invite is gone");
}

#[tokio::test]
async fn invite_inside_72h_is_accepted() {
    let store = Arc::new(SqliteStore::open(&temp_url("expire")).await.unwrap());
    store.migrate().await.unwrap();
    let mac = riehub_audit::AuditChainMac::new(riehub_crypto::Key32::new([7u8; 32]));
    let audit = Arc::new(riehub_audit::AuditLog::new(store.clone(), mac));
    let clock = Arc::new(riehub_core::TestClock::new(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
    ));
    let service = Arc::new(Service::new(store, audit).with_test_clock(clock.clone()));
    let (_user, token) = service.create_invite("e@example.com", None).await.unwrap();

    clock.advance(71 * 3600 * 1000);
    let (_uid, _recovery) = service
        .accept_invite(&token, &Secret::new("a-very-good-passphrase-42".to_owned()))
        .await
        .unwrap();
}

#[tokio::test]
async fn invite_expired_after_73h_is_refused() {
    let store = Arc::new(SqliteStore::open(&temp_url("expired")).await.unwrap());
    store.migrate().await.unwrap();
    let mac = riehub_audit::AuditChainMac::new(riehub_crypto::Key32::new([7u8; 32]));
    let audit = Arc::new(riehub_audit::AuditLog::new(store.clone(), mac));
    let clock = Arc::new(riehub_core::TestClock::new(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
    ));
    let service = Arc::new(Service::new(store, audit).with_test_clock(clock.clone()));
    let (_user, token) = service.create_invite("l@example.com", None).await.unwrap();

    clock.advance(73 * 3600 * 1000);
    let result = service
        .accept_invite(&token, &Secret::new("a-very-good-passphrase-42".to_owned()))
        .await;
    assert!(result.is_err(), "an expired invite is refused");
    let info = service.invite_by_token(&token).await;
    assert!(info.is_err(), "an expired invite lookup is refused");
    let _ = UserId::new();
}
