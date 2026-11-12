//! riehub-vault: per-user coffers on `cofferak-core` (ADR 0005,
//! security spec section 3). One coffer per user: cofferak host slots
//! are unlock slots keyed by secrets the user presents, the root
//! keypair is the recovery key, and credentials are entries under the
//! coffer's DEK.

#![forbid(unsafe_code)]

use cofferak_core::acl::{path_allowed, validate_id, Permission};
use cofferak_core::crypto::{
    decrypt_entry, encrypt_entry, generate_root_keypair, unwrap_dek_for_host, unwrap_dek_for_root,
    wrap_dek_for_host, wrap_dek_for_root, Aad, RootPrivateKey, RootPublicKey,
};
use cofferak_core::format::{VaultEntry, VaultFile};
use riehub_core::clock::Clock;
use riehub_core::error::{Error, Result};
use riehub_core::{Owner, SystemClock, UserId};
use riehub_crypto::{aead_open, aead_seal, hkdf, hmac};
use riehub_store::record::{Expect, Kind, Record};
use riehub_store::Store;
use zeroize::Zeroizing;

const MAX_ENTRY_BYTES: usize = 1_048_576; // 1 MiB

/// One unlock slot: the id and the 32-byte key derived from the
/// secret the user presents.
pub type SlotKey = [u8; 32];

/// The recovery keypair, generated at create and shown once. No
/// `Debug`: the private half is key material.
#[allow(missing_debug_implementations)]
pub struct RecoveryKey {
    pub public: RootPublicKey,
    pub private: RootPrivateKey,
}

/// An unlocked vault: the DEK in a zeroizing buffer. No `Debug`.
#[allow(missing_debug_implementations)]
pub struct Unlocked {
    dek: Zeroizing<[u8; 32]>,
    pub generation: u64,
}

impl Unlocked {
    /// The DEK bytes. The name shows up in reviews and audits.
    pub fn expose(&self) -> &[u8; 32] {
        &self.dek
    }
}

// The spec forbids a Debug on unlocked key material, so this crate
// opts out of the workspace-wide Debug lint here.
#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct Vault {
    pub user_id: UserId,
    pub file: VaultFile,
}

/// What a vault entry path can hold. The path prefix must match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    ConnectionCredential,
    ConnectionOauth,
    ConnectorCredential,
    WorkflowWebhook,
    ToolKey,
}

impl EntryKind {
    pub fn path(id: &str, kind: EntryKind) -> String {
        match kind {
            EntryKind::ConnectionCredential => format!("conn/{id}/credential"),
            EntryKind::ConnectionOauth => format!("conn/{id}/oauth"),
            EntryKind::ConnectorCredential => format!("mcp/{id}/credential"),
            EntryKind::WorkflowWebhook => format!("wf/{id}/webhook"),
            EntryKind::ToolKey => format!("tool/{id}/key"),
        }
    }
}

/// The hub key scope: path prefixes and a permission. Reads go through
/// cofferak's prefix check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acl {
    pub prefixes: Vec<String>,
    pub permission: Permission,
}

/// The audit surface the vault needs. Implemented by the server over
/// the audit log; tests use a small in-memory sink.
pub trait VaultAudit: Send + Sync {
    /// The newest generation recorded for this user, 0 if none.
    fn newest_generation(&self, user_id: &UserId) -> Result<u64>;
    /// Appends the `vault.write` event for the new generation.
    fn vault_write(&self, user_id: &UserId, generation: u64) -> Result<()>;
}

/// Creates a fresh vault: new DEK, root wrap for the recovery key,
/// first unlock slot, sealed.
pub fn create(user_id: UserId, first_slot: (&str, &SlotKey)) -> Result<(Vault, RecoveryKey)> {
    validate_id("slot", first_slot.0).map_err(|why| Error::Invalid(format!("slot id: {why}")))?;
    let (root_pub, root_priv) = generate_root_keypair().map_err(|_| Error::Internal(None))?;
    let dek = riehub_crypto::random_bytes::<32>();
    let mut file = VaultFile::new(root_pub.clone());
    file.hosts.insert(
        first_slot.0.to_owned(),
        wrap_dek_for_host(&dek, first_slot.0, first_slot.1),
    );
    file.root_wrap = Some(wrap_dek_for_root(&dek, &root_pub).map_err(|_| Error::Internal(None))?);
    let unlocked = Unlocked {
        dek: Zeroizing::new(dek),
        generation: 0,
    };
    seal(&mut file, &unlocked);
    let vault = Vault { user_id, file };
    let recovery = RecoveryKey {
        public: root_pub,
        private: root_priv,
    };
    Ok((vault, recovery))
}

/// Verifies the MAC and the generation, then unwraps the DEK through
/// the slot. Refuses a vault older than the newest audited generation
/// (rollback).
pub fn unlock(
    vault: &Vault,
    slot_id: &str,
    slot_key: &SlotKey,
    audit: &dyn VaultAudit,
) -> Result<Unlocked> {
    validate_id("slot", slot_id).map_err(|why| Error::Invalid(format!("slot id: {why}")))?;
    let anchor = audit.newest_generation(&vault.user_id)?;
    let wrap = vault
        .file
        .hosts
        .get(slot_id)
        .ok_or_else(|| Error::NotFound("no such slot".to_owned()))?;
    let dek = unwrap_dek_for_host(wrap, slot_id, slot_key)
        .map_err(|_| Error::Unauthorized("the slot key does not fit".to_owned()))?;
    vault
        .file
        .verify(&dek, Some(anchor))
        .map_err(map_integrity)?;
    Ok(Unlocked {
        dek: Zeroizing::new(dek),
        generation: vault.file.generation,
    })
}

/// Unwraps the DEK through the recovery key.
pub fn recover(vault: &Vault, recovery: &RootPrivateKey) -> Result<Unlocked> {
    let wrap = vault
        .file
        .root_wrap
        .as_ref()
        .ok_or_else(|| Error::NotFound("no root wrap".to_owned()))?;
    let dek = unwrap_dek_for_root(wrap, &vault.file.root_pubkey, recovery)
        .map_err(|_| Error::Unauthorized("the recovery key does not fit".to_owned()))?;
    vault.file.verify(&dek, None).map_err(map_integrity)?;
    Ok(Unlocked {
        dek: Zeroizing::new(dek),
        generation: vault.file.generation,
    })
}

/// Adds an unlock slot and re-seals.
pub fn add_slot(
    vault: &mut Vault,
    unlocked: &Unlocked,
    slot_id: &str,
    slot_key: &SlotKey,
) -> Result<()> {
    validate_id("slot", slot_id).map_err(|why| Error::Invalid(format!("slot id: {why}")))?;
    vault.file.hosts.insert(
        slot_id.to_owned(),
        wrap_dek_for_host(unlocked.expose(), slot_id, slot_key),
    );
    seal(&mut vault.file, unlocked);
    Ok(())
}

/// Removes an unlock slot and re-seals.
pub fn remove_slot(vault: &mut Vault, unlocked: &Unlocked, slot_id: &str) -> Result<()> {
    validate_id("slot", slot_id).map_err(|why| Error::Invalid(format!("slot id: {why}")))?;
    if vault.file.hosts.remove(slot_id).is_none() {
        return Err(Error::NotFound("no such slot".to_owned()));
    }
    seal(&mut vault.file, unlocked);
    Ok(())
}

/// Stores the value at the path, encrypted under the DEK. Values stay
/// under 1 MiB.
pub fn set(
    vault: &mut Vault,
    unlocked: &Unlocked,
    path: &str,
    value: &riehub_core::Secret<Vec<u8>>,
) -> Result<()> {
    if value.expose().len() > MAX_ENTRY_BYTES {
        return Err(Error::Invalid(
            "vault entry: values stay under 1 MiB".to_owned(),
        ));
    }
    let entry = VaultEntry {
        ciphertext: encrypt_entry(unlocked.expose(), &Aad::entry(path), value.expose()),
        version: 1,
        updated_at: SystemClock.now_millis(),
    };
    vault.file.entries.insert(path.to_owned(), entry);
    seal(&mut vault.file, unlocked);
    Ok(())
}

/// Reads the value at the path through the ACL. A path outside the
/// prefixes is denied with a `Forbidden` that does not say whether the
/// entry exists.
pub fn get(
    vault: &Vault,
    unlocked: &Unlocked,
    scope: &Acl,
    path: &str,
) -> Result<riehub_core::Secret<Vec<u8>>> {
    if !path_allowed(&scope.prefixes, path) {
        return Err(Error::Forbidden("outside this key's scope".to_owned()));
    }
    if !scope.permission.allows_read() {
        return Err(Error::Forbidden("this key cannot read".to_owned()));
    }
    let entry = vault
        .file
        .entries
        .get(path)
        .ok_or_else(|| Error::NotFound("no such entry".to_owned()))?;
    let plain = decrypt_entry(unlocked.expose(), &Aad::entry(path), &entry.ciphertext)
        .map_err(|_| Error::Internal(None))?;
    Ok(riehub_core::Secret::new(plain))
}

/// Removes the entry at the path and re-seals.
pub fn remove(vault: &mut Vault, unlocked: &Unlocked, path: &str) -> Result<()> {
    if vault.file.entries.remove(path).is_none() {
        return Err(Error::NotFound("no such entry".to_owned()));
    }
    seal(&mut vault.file, unlocked);
    Ok(())
}

/// The kinds of bulk content, each mapped to a fixed, compile-time
/// checked info string (security spec, section 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    Chat,
    ChatMessage,
    Skill,
    Workflow,
}

impl ContentKind {
    pub fn info(&self) -> &'static str {
        match self {
            ContentKind::Chat => "riehub/v1/content/chat",
            ContentKind::ChatMessage => "riehub/v1/content/chat_message",
            ContentKind::Skill => "riehub/v1/content/skill",
            ContentKind::Workflow => "riehub/v1/content/workflow",
        }
    }
}

/// The content key for one kind of bulk content: derived from the DEK
/// with the fixed info string. The kind itself enters through the
/// HMAC, not the HKDF info, which stays a compile-time checked
/// literal.
pub fn content_key(unlocked: &Unlocked, kind: ContentKind) -> riehub_crypto::Key32 {
    let master = hkdf(unlocked.expose(), kind.info());
    let tag = hmac(&master, kind.info().as_bytes());
    let mut key = [0u8; 32];
    key.copy_from_slice(&tag[..32]);
    riehub_crypto::Key32::new(key)
}

/// The record AAD of security spec section 4:
/// `riehub/v1/rec 0x00 kind 0x00 user_id 0x00 record_id 0x00 version`.
fn record_aad(kind: &str, user_id: &UserId, record_id: &str, version: u64) -> Vec<u8> {
    let mut aad = Vec::new();
    aad.extend_from_slice(b"riehub/v1/rec\x00");
    aad.extend_from_slice(kind.as_bytes());
    aad.push(0x00);
    aad.extend_from_slice(user_id.as_str().as_bytes());
    aad.push(0x00);
    aad.extend_from_slice(record_id.as_bytes());
    aad.push(0x00);
    aad.extend_from_slice(&version.to_le_bytes());
    aad
}

/// Seals a record blob: `aead_seal(content_key(kind), aad, plaintext)`.
pub fn seal_record(
    content_key: &riehub_crypto::Key32,
    kind: &str,
    user_id: &UserId,
    record_id: &str,
    version: u64,
    plaintext: &[u8],
) -> Vec<u8> {
    let aad = record_aad(kind, user_id, record_id, version);
    aead_seal(content_key, &aad, plaintext)
}

/// Opens a record blob. Moving it to another record, user or version
/// makes this fail.
pub fn open_record(
    content_key: &riehub_crypto::Key32,
    kind: &str,
    user_id: &UserId,
    record_id: &str,
    version: u64,
    blob: &[u8],
) -> Result<Zeroizing<Vec<u8>>> {
    let aad = record_aad(kind, user_id, record_id, version);
    aead_open(content_key, &aad, blob)
}

/// Seals the vault file and bumps the generation. The caller reads
/// the new generation from `file.generation`.
fn seal(file: &mut VaultFile, unlocked: &Unlocked) {
    file.seal(unlocked.expose());
}

fn map_integrity(error: cofferak_core::format::IntegrityError) -> Error {
    match error {
        cofferak_core::format::IntegrityError::Rollback { .. } => {
            Error::Conflict("the vault is older than the audited generation".to_owned())
        }
        _ => Error::Unauthorized("the vault does not verify".to_owned()),
    }
}

/// The store record that carries the vault: one per user, id = user id.
fn vault_record(vault: &Vault, version: u64) -> Result<Record> {
    let bytes = vault.file.to_bytes().map_err(|_| Error::Internal(None))?;
    let now = SystemClock.now_millis();
    let mut meta = serde_json::Map::new();
    meta.insert(
        "generation".to_owned(),
        serde_json::json!(vault.file.generation),
    );
    Ok(Record {
        owner: Owner::User(vault.user_id.clone()),
        kind: Kind::Vault,
        id: vault.user_id.as_str().to_owned(),
        parent: None,
        lookup: None,
        status: None,
        sort: 0,
        meta: serde_json::Value::Object(meta),
        data: Some(bytes),
        version,
        created_at: now,
        updated_at: now,
    })
}

/// Loads the stored vault for a user, if any.
pub async fn load(store: &dyn Store, user_id: &UserId) -> Result<Option<Vault>> {
    let record = store
        .get(&Owner::User(user_id.clone()), Kind::Vault, user_id.as_str())
        .await?;
    match record {
        None => Ok(None),
        Some(record) => {
            let bytes = record.data.ok_or_else(|| Error::Internal(None))?;
            let file = VaultFile::from_bytes(&bytes).map_err(|_| Error::Internal(None))?;
            Ok(Some(Vault {
                user_id: user_id.clone(),
                file,
            }))
        }
    }
}

/// Writes the sealed vault with compare-and-swap on the generation and
/// appends the `vault.write` audit event. On a conflict the store is
/// reloaded: if it holds a newer generation than ours, another writer
/// won and the save conflicts; if it holds an older one, the write is
/// retried once against it.
pub async fn save(
    store: &dyn Store,
    audit: &dyn VaultAudit,
    vault: &Vault,
    expected_generation: u64,
) -> Result<u64> {
    let mut expected = expected_generation;
    let mut attempts = 0;
    loop {
        attempts += 1;
        let record = vault_record(vault, expected)?;
        // Generation 0 means the store has no vault yet.
        let expected_cas = if expected == 0 {
            Expect::New
        } else {
            Expect::Version(expected)
        };
        let put = store.put(&record, expected_cas).await;
        match put {
            Ok(version) => {
                audit.vault_write(&vault.user_id, vault.file.generation)?;
                return Ok(version);
            }
            Err(Error::Conflict(_)) if attempts == 1 => {
                let stored = load(store, &vault.user_id).await?;
                let stored_generation = stored.map(|v| v.file.generation).unwrap_or(0);
                if stored_generation >= vault.file.generation {
                    return Err(Error::Conflict(
                        "another writer saved a newer vault".to_owned(),
                    ));
                }
                expected = stored_generation;
            }
            Err(e) => return Err(e),
        }
        if attempts > 2 {
            return Err(Error::Conflict(
                "the vault did not save after the reload".to_owned(),
            ));
        }
    }
}
