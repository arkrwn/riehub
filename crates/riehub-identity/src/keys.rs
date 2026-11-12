//! Hub keys (identity spec, section 4): the user's credential for
//! agents, SDKs and the CLI. The token `rh_<ID>_<secret>` is shown
//! once; the record holds the verifier, scopes, ACL prefixes and
//! expiry, and the label travels encrypted.

use crate::Service;
use riehub_core::error::{Error, Result};
use riehub_core::{Owner, UserId};
use riehub_crypto::hkdf;
use riehub_store::record::{Expect, Kind, Record};
use riehub_store::Store;
use std::sync::Arc;

pub const KEY_SLOT_INFO: &str = "riehub/v1/token/slot";
pub const KEY_AUTH_INFO: &str = "riehub/v1/token/auth";
const LABEL_INFO: &str = "riehub/v1/label/key";

/// The scopes a hub key can carry (identity spec, section 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyScope {
    Ai,
    Mcp,
    Manage,
}

impl KeyScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyScope::Ai => "ai",
            KeyScope::Mcp => "mcp",
            KeyScope::Manage => "manage",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ai" => Some(KeyScope::Ai),
            "mcp" => Some(KeyScope::Mcp),
            "manage" => Some(KeyScope::Manage),
            _ => None,
        }
    }
}

/// One hub key as its owner sees it. The token is never here.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct KeyInfo {
    pub id: String,
    pub scopes: Vec<String>,
    pub acl_prefixes: Vec<String>,
    pub expires_at: Option<i64>,
}

/// A created key with its one-time token.
#[derive(Clone)]
#[allow(missing_debug_implementations)]
pub struct NewKey {
    pub id: String,
    pub token: String,
    pub scopes: Vec<String>,
    pub acl_prefixes: Vec<String>,
    pub expires_at: Option<i64>,
}

/// An authenticated hub key call.
#[allow(missing_debug_implementations)]
pub struct AuthedKey {
    pub user_id: UserId,
    pub key_id: String,
    pub scopes: Vec<KeyScope>,
    pub acl_prefixes: Vec<String>,
    pub unlocked: Arc<riehub_vault::Unlocked>,
}

impl AuthedKey {
    pub fn has_scope(&self, scope: KeyScope) -> bool {
        self.scopes.contains(&scope)
    }

    /// The `riehub-vault` ACL this key may read through.
    pub fn vault_acl(&self) -> riehub_vault::Acl {
        riehub_vault::Acl {
            prefixes: self.acl_prefixes.clone(),
            permission: cofferak_core::acl::Permission::Read,
        }
    }
}

#[derive(Debug)]
pub struct KeyCrypto;

impl KeyCrypto {
    pub fn slot_key(secret: &[u8; 32]) -> riehub_crypto::Key32 {
        hkdf(secret, KEY_SLOT_INFO)
    }

    pub fn verifier(secret: &[u8; 32]) -> [u8; 32] {
        *hkdf(secret, KEY_AUTH_INFO).expose()
    }

    pub fn token(id: &str, secret: &[u8; 32]) -> String {
        format!("rh_{}_{}", id, riehub_crypto::base64url(secret))
    }
}

fn label_aad(user_id: &UserId, key_id: &str) -> Vec<u8> {
    let mut aad = Vec::new();
    aad.extend_from_slice(b"riehub/v1/label\x00");
    aad.extend_from_slice(user_id.as_str().as_bytes());
    aad.push(0x00);
    aad.extend_from_slice(key_id.as_bytes());
    aad
}

fn seal_label(dek: &[u8; 32], user_id: &UserId, key_id: &str, label: &str) -> Vec<u8> {
    let key = hkdf(dek, LABEL_INFO);
    let aad = label_aad(user_id, key_id);
    riehub_crypto::aead_seal(&key, &aad, label.as_bytes())
}

fn open_label(dek: &[u8; 32], user_id: &UserId, key_id: &str, blob: &[u8]) -> Result<String> {
    let key = hkdf(dek, LABEL_INFO);
    let aad = label_aad(user_id, key_id);
    let plain = riehub_crypto::aead_open(&key, &aad, blob).map_err(|_| Error::Internal(None))?;
    String::from_utf8(plain.as_slice().to_vec()).map_err(|_| Error::Internal(None))
}

impl Service {
    /// Creates a hub key: adds the vault slot `hk.<ID>`, stores the
    /// verifier, scopes, ACL prefixes and expiry. The label travels
    /// encrypted. The token is returned once.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_key(
        &self,
        user_id: &UserId,
        unlocked: &riehub_vault::Unlocked,
        vault: &riehub_vault::Vault,
        label: Option<&str>,
        scopes: &[KeyScope],
        acl_prefixes: &[String],
        expires_at: Option<i64>,
    ) -> Result<NewKey> {
        if scopes.is_empty() {
            return Err(Error::Invalid("a key needs at least one scope".to_owned()));
        }
        let id_record = riehub_core::HubKeyId::new();
        let bare_id = id_record.as_str().trim_start_matches("hkey_").to_owned();
        let secret = riehub_crypto::random_bytes::<32>();
        let slot_key = KeyCrypto::slot_key(&secret);

        let mut vault = vault.clone();
        riehub_vault::add_slot(
            &mut vault,
            unlocked,
            &format!("hk.{bare_id}"),
            slot_key.expose(),
        )?;
        riehub_vault::save(self.store.as_ref(), self, &vault, unlocked.generation).await?;

        let now = self.clock.now_millis();
        let verifier = KeyCrypto::verifier(&secret);
        let dek = unlocked.expose();
        let label_blob = match label {
            Some(label) => seal_label(dek, user_id, &bare_id, label),
            None => Vec::new(),
        };
        // The record data holds the verifier first, then the sealed
        // label; the verifier never sits in meta.
        let mut data = verifier.to_vec();
        data.extend_from_slice(&label_blob);
        let record = Record {
            owner: Owner::Instance,
            kind: Kind::HubKey,
            id: id_record.as_str().to_owned(),
            parent: None,
            lookup: Some(bare_id.clone()),
            status: Some("active".to_owned()),
            sort: 0,
            meta: serde_json::json!({
                "user": user_id.as_str(),
                "scopes": scopes.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                "acl_prefixes": acl_prefixes,
                "expires_at": expires_at,
            }),
            data: Some(data),
            version: 1,
            created_at: now,
            updated_at: now,
        };
        self.store.put(&record, Expect::New).await?;

        Ok(NewKey {
            id: bare_id.clone(),
            token: KeyCrypto::token(&bare_id, &secret),
            scopes: scopes.iter().map(|s| s.as_str().to_owned()).collect(),
            acl_prefixes: acl_prefixes.to_vec(),
            expires_at,
        })
    }

    /// Lists the keys of one user, never the tokens.
    pub async fn list_keys(&self, user_id: &UserId) -> Result<Vec<KeyInfo>> {
        let mut out = Vec::new();
        let mut cursor = None;
        loop {
            let page = self
                .store
                .list(
                    &Owner::Instance,
                    Kind::HubKey,
                    &Default::default(),
                    &riehub_store::record::Page {
                        after: cursor.clone(),
                        limit: 200,
                    },
                )
                .await?;
            for record in &page.items {
                if record.meta.get("user").and_then(|v| v.as_str()) != Some(user_id.as_str()) {
                    continue;
                }
                out.push(KeyInfo {
                    id: record.lookup.clone().unwrap_or_else(|| record.id.clone()),
                    scopes: record
                        .meta
                        .get("scopes")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(str::to_owned))
                                .collect()
                        })
                        .unwrap_or_default(),
                    acl_prefixes: record
                        .meta
                        .get("acl_prefixes")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(str::to_owned))
                                .collect()
                        })
                        .unwrap_or_default(),
                    expires_at: record.meta.get("expires_at").and_then(|v| v.as_i64()),
                });
            }
            match page.next {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        Ok(out)
    }

    /// Deletes a key: removes the record and the vault slot.
    pub async fn delete_key(
        &self,
        user_id: &UserId,
        bare_id: &str,
        unlocked: &riehub_vault::Unlocked,
    ) -> Result<()> {
        let record = self
            .store
            .get_by_lookup(&Owner::Instance, Kind::HubKey, bare_id)
            .await?
            .ok_or_else(|| Error::NotFound("no such key".to_owned()))?;
        if record.meta.get("user").and_then(|v| v.as_str()) != Some(user_id.as_str()) {
            return Err(Error::NotFound("no such key".to_owned()));
        }
        self.store
            .delete(&Owner::Instance, Kind::HubKey, &record.id)
            .await?;
        let stored = riehub_vault::load(self.store.as_ref(), user_id).await?;
        if let Some(mut vault) = stored {
            let expected = vault.file.generation;
            riehub_vault::remove_slot(&mut vault, unlocked, &format!("hk.{bare_id}"))?;
            riehub_vault::save(self.store.as_ref(), self, &vault, expected).await?;
        }
        self.cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&format!("hk.{bare_id}"));
        Ok(())
    }

    /// The label of one key, decrypted under the vault DEK.
    pub async fn key_label(
        &self,
        user_id: &UserId,
        bare_id: &str,
        unlocked: &riehub_vault::Unlocked,
    ) -> Result<Option<String>> {
        let record = self
            .store
            .get_by_lookup(&Owner::Instance, Kind::HubKey, bare_id)
            .await?
            .ok_or_else(|| Error::NotFound("no such key".to_owned()))?;
        match record.data {
            Some(data) if data.len() > 32 => Ok(Some(open_label(
                unlocked.expose(),
                user_id,
                bare_id,
                &data[32..],
            )?)),
            _ => Ok(None),
        }
    }

    /// Authenticates a hub key token: id lookup, `ct_eq` on the
    /// verifier, expiry, then the vault unlock through the cache.
    /// The scope check happens at the caller, which knows the route
    /// class.
    pub async fn auth_key(&self, token: &str, required: Option<KeyScope>) -> Result<AuthedKey> {
        // The base64url secret may hold dashes, so split at most twice.
        let parts: Vec<&str> = token.splitn(3, '_').collect();
        if parts.len() != 3 || parts[0] != "rh" {
            return Err(Error::Unauthorized("bad key token".to_owned()));
        }
        let bare_id = parts[1].to_owned();
        let secret = riehub_crypto::base64url_decode(parts[2])
            .ok_or_else(|| Error::Unauthorized("bad key token".to_owned()))?;
        let secret: [u8; 32] = secret
            .as_slice()
            .try_into()
            .map_err(|_| Error::Unauthorized("bad key token".to_owned()))?;

        let record = self
            .store
            .get_by_lookup(&Owner::Instance, Kind::HubKey, &bare_id)
            .await?
            .ok_or_else(|| Error::Unauthorized("no such key".to_owned()))?;
        // The record data holds the verifier, then the sealed label.
        let data = record
            .data
            .as_ref()
            .filter(|d| d.len() >= 32)
            .ok_or_else(|| Error::Unauthorized("bad key record".to_owned()))?;
        let verifier: [u8; 32] = data[..32].try_into().map_err(|_| Error::Internal(None))?;
        let computed = KeyCrypto::verifier(&secret);
        if !riehub_crypto::ct_eq(&verifier, &computed) {
            return Err(Error::Unauthorized("bad key token".to_owned()));
        }

        let now = self.clock.now_millis();
        if let Some(expires_at) = record.meta.get("expires_at").and_then(|v| v.as_i64()) {
            if now > expires_at {
                return Err(Error::Unauthorized("this key has expired".to_owned()));
            }
        }

        let scopes: Vec<KeyScope> = record
            .meta
            .get("scopes")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().and_then(KeyScope::parse))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(required) = required {
            if !scopes.contains(&required) {
                return Err(Error::Forbidden("this key lacks the scope".to_owned()));
            }
        }

        let user_id = UserId::parse(
            record
                .meta
                .get("user")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Internal(None))?,
        )?;

        let slot_key = KeyCrypto::slot_key(&secret);
        let unlocked = self
            .unlock_through_cache(
                user_id.clone(),
                &format!("hk.{bare_id}"),
                &format!("hk.{bare_id}"),
                &slot_key,
                now,
            )
            .await?;

        Ok(AuthedKey {
            user_id,
            key_id: bare_id,
            scopes,
            acl_prefixes: record
                .meta
                .get("acl_prefixes")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            unlocked,
        })
    }
}
