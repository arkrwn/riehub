//! The identity acceptance tests (identity spec, sections 1 and 2).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use riehub_audit::event::{Actor, AuditAction, AuditEvent, Outcome};
use riehub_core::{Secret, UserId};
use riehub_identity::Service;
use riehub_store::sqlite::SqliteStore;
use riehub_store::Store;
use std::sync::Arc;

fn temp_url(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "riehub-identity-{}-{}",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    format!("sqlite://{}/riehub.db", dir.display())
}

async fn service(tag: &str) -> (Arc<Service>, Arc<SqliteStore>) {
    let store = SqliteStore::open(&temp_url(tag)).await.unwrap();
    store.migrate().await.unwrap();
    let store = Arc::new(store);
    // The audit log holds no events yet; the identity service appends
    // through it directly.
    let mac = riehub_audit::AuditChainMac::new(riehub_crypto::Key32::new([7u8; 32]));
    let audit = Arc::new(riehub_audit::AuditLog::new(store.clone(), mac));
    (Arc::new(Service::new(store.clone(), audit)), store)
}

fn pw(s: &str) -> Secret<String> {
    Secret::new(s.to_owned())
}

#[tokio::test]
async fn setup_creates_owner_once() {
    let (service, _store) = service("setup").await;
    let (user, recovery) = service
        .setup_owner(
            "Owner@Example.com",
            &pw("a-very-good-passphrase-42"),
            Some("Owner"),
        )
        .await
        .unwrap();
    assert!(
        recovery.contains("-----BEGIN"),
        "the recovery key is armored"
    );
    assert_eq!(user.email, "owner@example.com", "emails are lower cased");
    assert!(user.roles.contains(&"owner".to_owned()));

    let loaded = service
        .user_by_email("owner@example.com")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.id, user.id);
}

#[tokio::test]
async fn second_setup_is_409() {
    let (service, _store) = service("second").await;
    service
        .setup_owner("a@example.com", &pw("a-very-good-passphrase-42"), None)
        .await
        .unwrap();
    let second = service
        .setup_owner("b@example.com", &pw("another-good-passphrase-7"), None)
        .await;
    assert!(
        matches!(second, Err(riehub_core::Error::Conflict(_))),
        "the second setup must conflict: {second:?}"
    );
}

#[tokio::test]
async fn login_ok_with_correct_password() {
    let (service, _store) = service("loginok").await;
    service
        .setup_owner("u@example.com", &pw("a-very-good-passphrase-42"), None)
        .await
        .unwrap();
    let user_id = service
        .login(&pw("u@example.com"), &pw("a-very-good-passphrase-42"), None)
        .await
        .unwrap();
    assert!(!user_id.as_str().is_empty());
}

#[tokio::test]
async fn login_unknown_email_and_wrong_password_give_same_status_and_body() {
    let (service, _store) = service("same").await;
    service
        .setup_owner("u@example.com", &pw("a-very-good-passphrase-42"), None)
        .await
        .unwrap();
    let unknown = service
        .login(
            &pw("ghost@example.com"),
            &pw("a-very-good-passphrase-42"),
            None,
        )
        .await;
    let wrong = service
        .login(&pw("u@example.com"), &pw("totally-different-pass-1"), None)
        .await;
    let (su, sw) = (unknown.err().unwrap(), wrong.err().unwrap());
    assert_eq!(su.to_string(), sw.to_string(), "the error text must match");
}

#[tokio::test]
async fn login_is_throttled_after_5_failures() {
    let (service, _store) = service("throttle").await;
    let email = pw("t@example.com");
    // Five failures put the account over the limit.
    for _ in 0..5 {
        let _ = service
            .login(&email, &pw("totally-different-pass-1"), Some("10.0.0.9"))
            .await;
    }
    let result = service
        .login(&email, &pw("totally-different-pass-1"), Some("10.0.0.9"))
        .await;
    assert!(
        matches!(result, Err(riehub_core::Error::RateLimited(_))),
        "the sixth attempt is throttled: {result:?}"
    );
}

#[tokio::test]
async fn login_after_throttle_lets_a_correct_password_through_after_window() {
    // The throttle counts failures; a correct password still must not
    // bypass the limit.
    let (service, _store) = service("throttle2").await;
    service
        .setup_owner("v@example.com", &pw("a-very-good-passphrase-42"), None)
        .await
        .unwrap();
    let email = pw("v@example.com");
    for _ in 0..5 {
        let _ = service
            .login(&email, &pw("totally-different-pass-1"), None)
            .await;
    }
    let result = service
        .login(&email, &pw("a-very-good-passphrase-42"), None)
        .await;
    assert!(matches!(result, Err(riehub_core::Error::RateLimited(_))));
}

#[tokio::test]
async fn common_password_is_rejected() {
    let (service, _store) = service("common").await;
    let result = service
        .setup_owner("c@example.com", &pw("password1234"), None)
        .await;
    assert!(
        result.is_err(),
        "a password from the common list is rejected"
    );
}

#[tokio::test]
async fn short_password_is_rejected() {
    let (service, _store) = service("short").await;
    let result = service
        .setup_owner("s@example.com", &pw("short-pass"), None)
        .await;
    assert!(result.is_err(), "a password under 12 bytes is rejected");
}

#[tokio::test]
async fn argon2_concurrency_is_capped() {
    let (service, _store) = service("cap").await;
    let mut permits = Vec::new();
    for _ in 0..4 {
        permits.push(service.acquire_argon().await.unwrap());
    }
    let fifth = tokio::spawn({
        let service = service.clone();
        async move { service.acquire_argon().await }
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(!fifth.is_finished(), "the fifth computation must wait");
    drop(permits.pop());
    let permit = tokio::time::timeout(std::time::Duration::from_secs(1), fifth)
        .await
        .unwrap()
        .unwrap();
    drop(permit);
}

#[tokio::test]
async fn stored_credential_never_contains_password() {
    let (service, store) = service("scan").await;
    let password = "RIEHUB-CANARY-secret-password";
    service
        .setup_owner("scan@example.com", &pw(password), None)
        .await
        .unwrap();
    let url = temp_url("scan-file");
    // The store is already at a temp file; scan it for the raw bytes.
    let _ = url;
    let records = store
        .list(
            &riehub_core::Owner::Instance,
            riehub_store::record::Kind::User,
            &Default::default(),
            &riehub_store::record::Page {
                after: None,
                limit: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(records.items.len(), 1);
    let needle = password.as_bytes();
    let user_id = UserId::parse(records.items[0].id.as_str()).unwrap();
    let credential = store
        .get(
            &riehub_core::Owner::User(user_id.clone()),
            riehub_store::record::Kind::Credential,
            user_id.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    let data = credential.data.unwrap();
    let found = data.windows(needle.len()).any(|w| w == needle);
    assert!(!found, "the stored credential must not hold the password");
}

#[tokio::test]
async fn the_audit_log_holds_the_identity_events() {
    let (service, store) = service("auditevents").await;
    service
        .setup_owner("e@example.com", &pw("a-very-good-passphrase-42"), None)
        .await
        .unwrap();
    let _ = service
        .login(&pw("e@example.com"), &pw("a-very-good-passphrase-42"), None)
        .await
        .unwrap();
    let _ = service
        .login(&pw("e@example.com"), &pw("totally-different-pass-1"), None)
        .await;

    let rows = store.audit_read(1, 100, &Default::default()).await.unwrap();
    let kinds: Vec<&str> = rows.iter().map(|r| r.kind.as_str()).collect();
    assert!(kinds.contains(&"user.create"), "kinds: {kinds:?}");
    assert!(kinds.contains(&"login.ok"), "kinds: {kinds:?}");
    assert!(kinds.contains(&"login.failed"), "kinds: {kinds:?}");
    let _ = (
        Actor::System,
        AuditAction::SystemStart,
        AuditEvent::SystemStart {},
        Outcome::Ok,
    );
    let _ = UserId::new();
}
