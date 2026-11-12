//! The instance coffer acceptance tests (security spec, section 5).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use riehub_store::Store;
use riehub_vault::instance::AUDIT_KEY_PATH;
use riehub_vault::instance::{create, get_secret, open, recover, save, SealBackend};

fn temp_url(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "riehub-instance-{}-{}",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    format!("sqlite://{}/riehub.db", dir.display())
}

fn file_backend(tag: &str) -> SealBackend {
    let dir = std::env::temp_dir().join(format!(
        "riehub-instance-key-{}-{}",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    SealBackend::File {
        path: dir.join("host.key"),
    }
}

async fn fresh_store(tag: &str) -> std::sync::Arc<riehub_store::sqlite::SqliteStore> {
    let store = riehub_store::sqlite::SqliteStore::open(&temp_url(tag))
        .await
        .unwrap();
    store.migrate().await.unwrap();
    std::sync::Arc::new(store)
}

#[tokio::test]
async fn instance_recovery_key_reopens_the_coffer() {
    let store = fresh_store("recover").await;
    let backend = file_backend("recover").host_seal().unwrap();
    let (coffer, armored) = create(store.as_ref(), backend.as_ref()).await.unwrap();

    // The armored recovery key is a valid armored key: reopening the
    // coffer through it must work.
    let recovery = parse_armored(&armored);
    let opened = recover(store.as_ref(), &recovery).await.unwrap();
    let entries = vec![(AUDIT_KEY_PATH.to_owned(), vec![9u8; 32])];
    save(
        store.as_ref(),
        &coffer,
        &opened,
        &entries,
        coffer.file.generation,
    )
    .await
    .unwrap();

    let reopened = open(store.as_ref(), backend.as_ref()).await.unwrap();
    let reloaded = riehub_vault::instance::load(store.as_ref())
        .await
        .unwrap()
        .unwrap();
    let secret = get_secret(&reloaded.file, &reopened, AUDIT_KEY_PATH).unwrap();
    assert_eq!(secret, vec![9u8; 32]);
}

fn parse_armored(text: &str) -> cofferak_core::crypto::RootPrivateKey {
    cofferak_core::crypto::RootPrivateKey::from_armored(text).unwrap()
}

#[tokio::test]
async fn open_with_a_missing_coffer_fails() {
    let store = fresh_store("missing").await;
    let backend = file_backend("missing").host_seal().unwrap();
    let result = open(store.as_ref(), backend.as_ref()).await;
    assert!(result.is_err(), "no coffer means no open");
}

#[tokio::test]
async fn open_with_a_wrong_host_key_fails() {
    use std::os::unix::fs::PermissionsExt;
    let store = fresh_store("wronghost").await;
    let backend = file_backend("wronghost");
    let sealed = backend.host_seal().unwrap();
    create(store.as_ref(), sealed.as_ref()).await.unwrap();

    // Overwrite the key file with a different 32-byte key, mode 0600:
    // the host key no longer fits the coffer.
    let path = match &backend {
        SealBackend::File { path } => path.clone(),
        _ => unreachable!(),
    };
    std::fs::write(&path, b"wrong key bytes wrong key bytes 0").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

    let result = open(store.as_ref(), sealed.as_ref()).await;
    assert!(result.is_err(), "a wrong host key must fail closed");
}

#[tokio::test]
async fn key_file_with_loose_mode_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let store = fresh_store("loose").await;
    let backend = file_backend("loose");
    let sealed = backend.host_seal().unwrap();
    create(store.as_ref(), sealed.as_ref()).await.unwrap();

    let path = match &backend {
        SealBackend::File { path } => path.clone(),
        _ => unreachable!(),
    };
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let result = open(store.as_ref(), sealed.as_ref()).await;
    assert!(
        result.is_err(),
        "a group or world readable key file is refused"
    );
}
