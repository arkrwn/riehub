//! The hub key acceptance tests at the service level (identity spec,
//! section 4, security spec 3.2).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use riehub_core::Secret;
use riehub_identity::keys::KeyScope;
use riehub_identity::Service;
use riehub_store::sqlite::SqliteStore;
use riehub_store::Store;
use std::sync::Arc;

fn temp_url(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "riehub-keys-{}-{}",
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

const COOKIE_KEY: &[u8; 32] = &[3u8; 32];

#[tokio::test]
async fn db_contents_cannot_rebuild_a_key() {
    let service = service("rebuild").await;
    service
        .setup_owner(
            "k@example.com",
            &Secret::new("a-very-good-passphrase-42".to_owned()),
            None,
        )
        .await
        .unwrap();
    let (_user_id, session) = service
        .login(
            &Secret::new("k@example.com".to_owned()),
            &Secret::new("a-very-good-passphrase-42".to_owned()),
            None,
            COOKIE_KEY,
        )
        .await
        .unwrap();
    let authed = service.validate_session(&session.token).await.unwrap();
    let user_id = authed.auth_user.user_id.clone();
    let vault = riehub_vault::load(service.store_ref(), &user_id)
        .await
        .unwrap()
        .unwrap();

    let new_key = service
        .create_key(
            &user_id,
            &authed.auth_user.unlocked,
            &vault,
            None,
            &[KeyScope::Ai],
            &[],
            None,
        )
        .await
        .unwrap();

    // The attacker reads the key record from the database and tries
    // the stored verifier as the slot key. The verifier is
    // hkdf(secret, token/auth) and the slot key is
    // hkdf(secret, token/slot): one does not give the other.
    let record = service
        .store_ref()
        .get_by_lookup(
            &riehub_core::Owner::Instance,
            riehub_store::record::Kind::HubKey,
            &new_key.id,
        )
        .await
        .unwrap()
        .unwrap();
    let data = record.data.unwrap();
    assert!(data.len() >= 32);
    let mut verifier = [0u8; 32];
    verifier.copy_from_slice(&data[..32]);

    let as_slot_key = riehub_crypto::Key32::new(verifier);
    let result = riehub_vault::unlock(
        &vault,
        &format!("hk.{}", new_key.id),
        as_slot_key.expose(),
        service.as_ref(),
    )
    .await;
    assert!(
        result.is_err(),
        "the stored verifier must not work as the slot key"
    );
}
