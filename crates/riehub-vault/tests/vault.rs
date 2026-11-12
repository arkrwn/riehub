//! The vault acceptance tests (security spec, section 3).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use riehub_core::error::Result;
use riehub_core::{Secret, UserId};
use riehub_store::record::{Expect, Kind, Record};
use riehub_store::sqlite::SqliteStore;
use riehub_store::Store;
use riehub_vault::{
    add_slot, content_key, create, get, load, open_record, recover, remove, remove_slot, save,
    seal_record, set, unlock, Acl, ContentKind, EntryKind, Vault, VaultAudit,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

fn temp_url(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "riehub-vault-{}-{}",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    format!("sqlite://{}/riehub.db", dir.display())
}

async fn fresh_store(tag: &str) -> Arc<SqliteStore> {
    let store = SqliteStore::open(&temp_url(tag)).await.unwrap();
    store.migrate().await.unwrap();
    Arc::new(store)
}

/// The in-memory audit sink the tests use: it keeps the newest
/// generation per user, like the audit log does for real.
#[derive(Default)]
struct Sink {
    newest: Mutex<BTreeMap<String, u64>>,
}

impl VaultAudit for Sink {
    fn newest_generation(&self, user_id: &UserId) -> Result<u64> {
        Ok(*self
            .newest
            .lock()
            .unwrap()
            .get(user_id.as_str())
            .unwrap_or(&0))
    }

    fn vault_write(&self, user_id: &UserId, generation: u64) -> Result<()> {
        self.newest
            .lock()
            .unwrap()
            .insert(user_id.as_str().to_owned(), generation);
        Ok(())
    }
}

const SLOT_PW: &[u8; 32] = b"password derived key bytes 00001";
const SLOT_PW2: &[u8; 32] = b"second slot key bytes 0000000001";

fn user(n: u8) -> UserId {
    // The Crockford alphabet excludes I, L, O and U.
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut raw = String::from("user_01ARZ3NDEKTSV4RRFFQ69G5FA");
    raw.push(ALPHABET[n as usize] as char);
    UserId::parse(&raw).unwrap()
}

#[tokio::test]
async fn create_then_unlock_with_slot() {
    let u = user(1);
    let (vault, _recovery) = create(u.clone(), ("pw", SLOT_PW)).unwrap();
    let sink = Sink::default();
    let unlocked = unlock(&vault, "pw", SLOT_PW, &sink).unwrap();
    assert_eq!(unlocked.generation, 1);
    let wrong_slot = unlock(&vault, "nope", SLOT_PW, &sink);
    assert!(wrong_slot.is_err());
}

#[tokio::test]
async fn unlock_with_wrong_key_fails() {
    let u = user(2);
    let (vault, _recovery) = create(u.clone(), ("pw", SLOT_PW)).unwrap();
    let sink = Sink::default();
    let result = unlock(&vault, "pw", SLOT_PW2, &sink);
    assert!(result.is_err(), "a wrong slot key must not unlock");
}

#[tokio::test]
async fn recover_with_recovery_key() {
    let u = user(3);
    let (vault, recovery) = create(u.clone(), ("pw", SLOT_PW)).unwrap();
    let unlocked = recover(&vault, &recovery.private).unwrap();
    assert_eq!(unlocked.generation, 1);
}

#[tokio::test]
async fn recover_with_other_users_key_fails() {
    let u = user(4);
    let (_vault_a, recovery_a) = create(user(4), ("pw", SLOT_PW)).unwrap();
    let (vault_b, _recovery_b) = create(u.clone(), ("pw", SLOT_PW2)).unwrap();
    let result = recover(&vault_b, &recovery_a.private);
    assert!(result.is_err(), "another user's recovery key must not fit");
}

#[tokio::test]
async fn add_and_remove_slot() {
    let u = user(5);
    let (mut vault, _recovery) = create(u.clone(), ("pw", SLOT_PW)).unwrap();
    let sink = Sink::default();
    let unlocked = unlock(&vault, "pw", SLOT_PW, &sink).unwrap();
    add_slot(&mut vault, &unlocked, "pw2", SLOT_PW2).unwrap();
    let with_new = unlock(&vault, "pw2", SLOT_PW2, &sink).unwrap();
    assert!(
        with_new.generation > unlocked.generation,
        "the vault re-sealed"
    );
    remove_slot(&mut vault, &with_new, "pw2").unwrap();
    let gone = unlock(&vault, "pw2", SLOT_PW2, &sink);
    assert!(gone.is_err(), "the removed slot must no longer unlock");
}

#[tokio::test]
async fn removed_slot_no_longer_unlocks() {
    let u = user(6);
    let (mut vault, _recovery) = create(u.clone(), ("pw", SLOT_PW)).unwrap();
    let sink = Sink::default();
    let unlocked = unlock(&vault, "pw", SLOT_PW, &sink).unwrap();
    add_slot(&mut vault, &unlocked, "pw2", SLOT_PW2).unwrap();
    let with_new = unlock(&vault, "pw2", SLOT_PW2, &sink).unwrap();
    remove_slot(&mut vault, &with_new, "pw").unwrap();
    assert!(unlock(&vault, "pw", SLOT_PW, &sink).is_err());
}

#[tokio::test]
async fn set_get_remove_entry() {
    let u = user(7);
    let (mut vault, _recovery) = create(u.clone(), ("pw", SLOT_PW)).unwrap();
    let sink = Sink::default();
    let unlocked = unlock(&vault, "pw", SLOT_PW, &sink).unwrap();
    let path = EntryKind::path(
        "conn_01ARZ3NDEKTSV4RRFFQ69G5FAV",
        EntryKind::ConnectionCredential,
    );
    set(
        &mut vault,
        &unlocked,
        &path,
        &Secret::new(b"sk-provider-key".to_vec()),
    )
    .unwrap();

    let acl = Acl {
        prefixes: vec!["conn".to_owned()],
        permission: Permission::ReadWrite,
    };
    let value = get(&vault, &unlocked, &acl, &path).unwrap();
    assert_eq!(value.expose(), b"sk-provider-key");

    remove(&mut vault, &unlocked, &path).unwrap();
    let gone = get(&vault, &unlocked, &acl, &path);
    assert!(gone.is_err(), "the removed entry must be gone");
}

#[tokio::test]
async fn entry_over_1mib_is_rejected() {
    let u = user(8);
    let (mut vault, _recovery) = create(u.clone(), ("pw", SLOT_PW)).unwrap();
    let sink = Sink::default();
    let unlocked = unlock(&vault, "pw", SLOT_PW, &sink).unwrap();
    let path = EntryKind::path("conn_big", EntryKind::ConnectionCredential);
    let big = Secret::new(vec![0u8; 1_048_577]);
    let result = set(&mut vault, &unlocked, &path, &big);
    assert!(result.is_err(), "a value over 1 MiB must be rejected");
}

use cofferak_core::acl::Permission;

#[tokio::test]
async fn get_outside_acl_is_denied() {
    let u = user(9);
    let (mut vault, _recovery) = create(u.clone(), ("pw", SLOT_PW)).unwrap();
    let sink = Sink::default();
    let unlocked = unlock(&vault, "pw", SLOT_PW, &sink).unwrap();
    let path_a = EntryKind::path("con_A", EntryKind::ConnectionCredential);
    let path_b = EntryKind::path("con_B", EntryKind::ConnectionCredential);
    set(
        &mut vault,
        &unlocked,
        &path_a,
        &Secret::new(b"key-a".to_vec()),
    )
    .unwrap();
    set(
        &mut vault,
        &unlocked,
        &path_b,
        &Secret::new(b"key-b".to_vec()),
    )
    .unwrap();

    let scoped = Acl {
        prefixes: vec![EntryKind::path("con_A", EntryKind::ConnectionCredential)],
        permission: Permission::ReadWrite,
    };
    let denied = get(&vault, &unlocked, &scoped, &path_b);
    assert!(
        matches!(denied, Err(riehub_core::Error::Forbidden(_))),
        "a prefix for con_A must not read con_B: {denied:?}"
    );
    let allowed = get(&vault, &unlocked, &scoped, &path_a).unwrap();
    assert_eq!(allowed.expose(), b"key-a");
}

#[tokio::test]
async fn tampered_vault_bytes_fail_to_unlock() {
    let u = user(10);
    let (mut vault, _recovery) = create(u.clone(), ("pw", SLOT_PW)).unwrap();
    let sink = Sink::default();
    let unlocked = unlock(&vault, "pw", SLOT_PW, &sink).unwrap();
    let path = EntryKind::path("con_A", EntryKind::ConnectionCredential);
    set(
        &mut vault,
        &unlocked,
        &path,
        &Secret::new(b"key-a".to_vec()),
    )
    .unwrap();
    let mut bytes = vault.file.to_bytes().unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    let tampered = Vault {
        user_id: u.clone(),
        file: cofferak_core::format::VaultFile::from_bytes(&bytes).unwrap(),
    };
    let result = unlock(&tampered, "pw", SLOT_PW, &sink);
    assert!(result.is_err(), "tampered vault bytes must fail to unlock");
}

#[tokio::test]
async fn rolled_back_vault_is_refused() {
    let store = fresh_store("rollback").await;
    let u = user(11);
    let sink = Sink::default();
    let (mut vault, _recovery) = create(u.clone(), ("pw", SLOT_PW)).unwrap();
    let generation_one_bytes = vault.file.to_bytes().unwrap();
    save(store.as_ref(), &sink, &vault, 0).await.unwrap();

    // Advance to generation 2, then put the generation 1 bytes back
    // under the newest record version, the way a rollback attack does.
    let unlocked = unlock(&vault, "pw", SLOT_PW, &sink).unwrap();
    let path = EntryKind::path("con_A", EntryKind::ConnectionCredential);
    set(&mut vault, &unlocked, &path, &Secret::new(b"v2".to_vec())).unwrap();
    save(store.as_ref(), &sink, &vault, 1).await.unwrap();

    let mut rollback = vault_record_for(u.clone(), 2, generation_one_bytes);
    let stored = store
        .get(
            &riehub_core::Owner::User(u.clone()),
            Kind::Vault,
            u.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    rollback.created_at = stored.created_at;
    store.put(&rollback, Expect::Any).await.unwrap();

    let loaded = load(store.as_ref(), &u).await.unwrap().unwrap();
    let refused = unlock(&loaded, "pw", SLOT_PW, &sink);
    assert!(
        matches!(refused, Err(riehub_core::Error::Conflict(_))),
        "a rolled back vault must be refused"
    );
}

fn vault_record_for(user_id: UserId, version: u64, bytes: Vec<u8>) -> Record {
    Record {
        owner: riehub_core::Owner::User(user_id.clone()),
        kind: Kind::Vault,
        id: user_id.as_str().to_owned(),
        parent: None,
        lookup: None,
        status: None,
        sort: 0,
        meta: serde_json::json!({}).as_object().unwrap().clone().into(),
        data: Some(bytes),
        version,
        created_at: 0,
        updated_at: 0,
    }
}

#[tokio::test]
async fn concurrent_saves_one_wins_one_conflicts() {
    let store = fresh_store("concurrent").await;
    let u = user(12);
    let sink = Sink::default();
    let (vault, _recovery) = create(u.clone(), ("pw", SLOT_PW)).unwrap();
    // Both writers start from the same stored generation 1.
    save(store.as_ref(), &sink, &vault, 0).await.unwrap();

    let writer_a = vault.clone();
    let writer_b = vault.clone();
    let (a, b) = tokio::join!(
        save(store.as_ref(), &sink, &writer_a, 1),
        save(store.as_ref(), &sink, &writer_b, 1),
    );
    let results = [a.is_ok(), b.is_ok()];
    assert_eq!(
        results.iter().filter(|ok| **ok).count(),
        1,
        "one writer wins, one conflicts: {a:?} vs {b:?}"
    );
}

#[tokio::test]
async fn record_blob_fails_to_open_for_other_user_or_id_or_version() {
    let u = user(13);
    let (vault, _recovery) = create(u.clone(), ("pw", SLOT_PW)).unwrap();
    let sink = Sink::default();
    let unlocked = unlock(&vault, "pw", SLOT_PW, &sink).unwrap();
    let key = content_key(&unlocked, ContentKind::Chat);
    let blob = seal_record(
        &key,
        "chat",
        &u,
        "chat_01ARZ3NDEKTSV4RRFFQ69G5FAV",
        1,
        b"hello",
    );

    let opened = open_record(
        &key,
        "chat",
        &u,
        "chat_01ARZ3NDEKTSV4RRFFQ69G5FAV",
        1,
        &blob,
    );
    assert!(opened.is_ok());
    assert_eq!(opened.unwrap().as_slice(), b"hello");

    let other_user = user(14);
    assert!(open_record(
        &key,
        "chat",
        &other_user,
        "chat_01ARZ3NDEKTSV4RRFFQ69G5FAV",
        1,
        &blob
    )
    .is_err());
    assert!(open_record(
        &key,
        "chat",
        &u,
        "chat_01ARZ3NDEKTSV4RRFFQ69G5FAW",
        1,
        &blob
    )
    .is_err());
    assert!(open_record(
        &key,
        "chat",
        &u,
        "chat_01ARZ3NDEKTSV4RRFFQ69G5FAV",
        2,
        &blob
    )
    .is_err());
    assert!(open_record(
        &key,
        "chat_message",
        &u,
        "chat_01ARZ3NDEKTSV4RRFFQ69G5FAV",
        1,
        &blob
    )
    .is_err());
}

#[tokio::test]
async fn stored_vault_bytes_do_not_contain_the_entry_plaintext() {
    let store = fresh_store("plaintext").await;
    let u = user(15);
    let sink = Sink::default();
    let (mut vault, _recovery) = create(u.clone(), ("pw", SLOT_PW)).unwrap();
    let unlocked = unlock(&vault, "pw", SLOT_PW, &sink).unwrap();
    let path = EntryKind::path("con_A", EntryKind::ConnectionCredential);
    let plaintext = b"RIEHUB-CANARY-plaintext-check";
    set(
        &mut vault,
        &unlocked,
        &path,
        &Secret::new(plaintext.to_vec()),
    )
    .unwrap();
    save(store.as_ref(), &sink, &vault, 0).await.unwrap();

    let record = store
        .get(
            &riehub_core::Owner::User(u.clone()),
            Kind::Vault,
            u.as_str(),
        )
        .await
        .unwrap()
        .unwrap();
    let bytes = record.data.unwrap();
    assert!(
        !find(bytes.as_slice(), plaintext),
        "the stored vault bytes must not hold the entry plaintext"
    );
}

fn find(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
