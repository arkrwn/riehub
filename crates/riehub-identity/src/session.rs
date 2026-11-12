//! Web sessions (identity spec, section 3): the token
//! `rs_<SID>_<secret>`, the cookie, the session record and the unlock
//! cache (security spec, sections 3.2 and 3.4).

use crate::Service;
use riehub_core::error::{Error, Result};
use riehub_core::{Owner, UserId};
use riehub_crypto::hkdf;
use riehub_store::record::{Expect, Kind, Record};
use riehub_store::sqlite::SqliteStore;
use riehub_store::Store;
use std::collections::HashMap;
use std::sync::Arc;

pub const TOKEN_SLOT_INFO: &str = "riehub/v1/token/slot";
pub const TOKEN_AUTH_INFO: &str = "riehub/v1/token/auth";
pub const COOKIE_NAME_HOST: &str = "__Host-rs";
pub const COOKIE_NAME_DEV: &str = "rs";
pub const IDLE_TIMEOUT: i64 = 30 * 60_000;
pub const ABSOLUTE_TIMEOUT: i64 = 12 * 60 * 60_000;
pub const MAX_SESSIONS: usize = 10;
const UNLOCK_CACHE_IDLE: i64 = 10 * 60_000;
const UNLOCK_CACHE_MAX: usize = 10_000;

/// One authenticated web session, created at login. The token inside
/// is credential material, so no `Debug`.
#[derive(Clone)]
#[allow(missing_debug_implementations)]
pub struct Session {
    /// The session record id: `sess_<ULID>`.
    pub id: String,
    /// The bare sid that goes into the token.
    pub sid: String,
    pub user_id: UserId,
    /// The token `rs_<SID>_<secret>`.
    pub token: String,
    /// The csrf token: `hmac(cookie_key, sid)` in hex.
    pub csrf_token: String,
}

/// A validated session: the `AuthUser` handlers use. Ready to touch
/// the vault.
#[allow(missing_debug_implementations)]
pub struct AuthedSession {
    pub auth_user: crate::roles::AuthUser,
}

impl std::ops::Deref for AuthedSession {
    type Target = crate::roles::AuthUser;
    fn deref(&self) -> &crate::roles::AuthUser {
        &self.auth_user
    }
}

impl AuthedSession {
    /// The bare sid of this session, for the csrf token and the slot
    /// name.
    pub fn sid(&self) -> &str {
        self.session_id.trim_start_matches("sess_")
    }
}

#[allow(missing_debug_implementations)]
pub struct CacheEntry {
    unlocked: Arc<riehub_vault::Unlocked>,
    last_used_ms: i64,
}

/// The token derivations of security spec, section 3.2.
#[derive(Debug)]
pub struct SessionCrypto;

impl SessionCrypto {
    pub fn slot_key(secret: &[u8; 32]) -> riehub_crypto::Key32 {
        hkdf(secret, TOKEN_SLOT_INFO)
    }

    pub fn verifier(secret: &[u8; 32]) -> [u8; 32] {
        *hkdf(secret, TOKEN_AUTH_INFO).expose()
    }

    pub fn token(sid: &str, secret: &[u8; 32]) -> String {
        format!("rs_{}_{}", sid, riehub_crypto::base64url(secret))
    }
}

pub fn csrf_token(cookie_key: &[u8; 32], sid: &str) -> String {
    let tag = riehub_crypto::hmac(&riehub_crypto::Key32::new(*cookie_key), sid.as_bytes());
    tag.iter().map(|b| format!("{b:02x}")).collect()
}

impl Service {
    /// Creates a session for a logged-in user: adds the vault slot
    /// `ws.<SID>`, stores the session record and ends the oldest
    /// sessions beyond the cap. The unlocked vault handle goes into
    /// the cache.
    pub async fn create_session(
        &self,
        user_id: &UserId,
        vault: &riehub_vault::Vault,
        unlocked: Arc<riehub_vault::Unlocked>,
        cookie_key: &[u8; 32],
    ) -> Result<Session> {
        let sid_record = riehub_core::SessionId::new();
        let sid = sid_record.as_str().trim_start_matches("sess_").to_owned();
        let secret = riehub_crypto::random_bytes::<32>();
        let slot_key = SessionCrypto::slot_key(&secret);

        let mut vault = vault.clone();
        riehub_vault::add_slot(
            &mut vault,
            &unlocked,
            &format!("ws.{sid}"),
            slot_key.expose(),
        )?;
        riehub_vault::save(self.store.as_ref(), self, &vault, unlocked.generation).await?;

        let now = self.clock.now_millis();
        let verifier = SessionCrypto::verifier(&secret);
        let record = session_record(
            user_id,
            sid_record.as_str(),
            &verifier,
            now,
            now + ABSOLUTE_TIMEOUT,
            now,
        );
        self.store.put(&record, Expect::New).await?;
        self.enforce_session_cap(user_id, &unlocked).await?;

        self.cache.lock().unwrap_or_else(|p| p.into_inner()).insert(
            sid.clone(),
            CacheEntry {
                unlocked,
                last_used_ms: now,
            },
        );

        let token = SessionCrypto::token(&sid, &secret);
        let csrf = csrf_token(cookie_key, &sid);
        Ok(Session {
            id: sid_record.as_str().to_owned(),
            sid,
            user_id: user_id.clone(),
            token,
            csrf_token: csrf,
        })
    }

    /// Ends the oldest sessions once the user holds more than the cap.
    async fn enforce_session_cap(
        &self,
        user_id: &UserId,
        unlocked: &riehub_vault::Unlocked,
    ) -> Result<()> {
        let mut mine = self.list_sessions(user_id).await?;
        if mine.len() <= MAX_SESSIONS {
            return Ok(());
        }
        mine.sort_by_key(|s| s.created_at);
        while mine.len() > MAX_SESSIONS {
            let oldest = mine.remove(0);
            self.end_session(user_id, &oldest.sid, unlocked).await?;
        }
        Ok(())
    }

    /// Lists the active sessions of one user.
    pub async fn list_sessions(&self, user_id: &UserId) -> Result<Vec<SessionInfo>> {
        let mut out = Vec::new();
        let mut cursor = None;
        loop {
            let page = self
                .store
                .list(
                    &Owner::Instance,
                    Kind::Session,
                    &Default::default(),
                    &riehub_store::record::Page {
                        after: cursor.clone(),
                        limit: 200,
                    },
                )
                .await?;
            for record in &page.items {
                if record.meta.get("user").and_then(|v| v.as_str()) == Some(user_id.as_str()) {
                    out.push(SessionInfo {
                        session_id: record.id.clone(),
                        sid: record
                            .meta
                            .get("sid")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_owned(),
                        created_at: record.created_at,
                        last_seen: record.updated_at,
                        expires_at: record
                            .meta
                            .get("expires_at")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0),
                    });
                }
            }
            match page.next {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        Ok(out)
    }

    /// Ends one session: deletes the record and the vault slot.
    pub async fn end_session(
        &self,
        user_id: &UserId,
        sid: &str,
        unlocked: &riehub_vault::Unlocked,
    ) -> Result<()> {
        self.store
            .delete(&Owner::Instance, Kind::Session, &format!("sess_{sid}"))
            .await?;
        let stored = riehub_vault::load(self.store.as_ref(), user_id).await?;
        if let Some(mut vault) = stored {
            let expected = vault.file.generation;
            riehub_vault::remove_slot(&mut vault, unlocked, &format!("ws.{sid}"))?;
            riehub_vault::save(self.store.as_ref(), self, &vault, expected).await?;
        }
        self.cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(sid);
        Ok(())
    }

    /// Validates a token into an authenticated session: parses the
    /// cookie value, checks the verifier with `ct_eq`, the idle and
    /// absolute timeouts, and unlocks the vault through the cache.
    pub async fn validate_session(&self, token: &str) -> Result<AuthedSession> {
        // The base64url secret may contain '-', so split at most twice.
        let parts: Vec<&str> = token.splitn(3, '_').collect();
        if parts.len() != 3 || parts[0] != "rs" {
            return Err(Error::Unauthorized("bad session token".to_owned()));
        }
        let sid = parts[1].to_owned();
        let secret = riehub_crypto::base64url_decode(parts[2])
            .ok_or_else(|| Error::Unauthorized("bad session token".to_owned()))?;
        if secret.len() != 32 {
            return Err(Error::Unauthorized("bad session token".to_owned()));
        }
        let secret: [u8; 32] = secret
            .as_slice()
            .try_into()
            .map_err(|_| Error::Unauthorized("bad session token".to_owned()))?;

        let record = self
            .store
            .get_by_lookup(&Owner::Instance, Kind::Session, &sid)
            .await?
            .ok_or_else(|| Error::Unauthorized("no such session".to_owned()))?;
        let stored = record
            .data
            .as_ref()
            .filter(|d| d.len() == 32)
            .ok_or_else(|| Error::Unauthorized("bad session record".to_owned()))?;
        let computed = SessionCrypto::verifier(&secret);
        if !riehub_crypto::ct_eq(stored, &computed) {
            return Err(Error::Unauthorized("bad session token".to_owned()));
        }

        let now = self.clock.now_millis();
        let absolute_deadline = record
            .meta
            .get("expires_at")
            .and_then(|v| v.as_i64())
            .unwrap_or(record.created_at + ABSOLUTE_TIMEOUT);
        if now > absolute_deadline {
            self.delete_session_record(&sid).await?;
            return Err(Error::Unauthorized("the session expired".to_owned()));
        }
        if now > record.updated_at + IDLE_TIMEOUT {
            self.delete_session_record(&sid).await?;
            return Err(Error::Unauthorized("the session went idle".to_owned()));
        }

        let user_id = UserId::parse(
            record
                .meta
                .get("user")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Internal(None))?,
        )?;

        let slot_key = SessionCrypto::slot_key(&secret);
        let unlocked = self
            .unlock_through_cache(user_id.clone(), &sid, &format!("ws.{sid}"), &slot_key, now)
            .await?;

        // Update last seen at most once per minute.
        if now - record.updated_at >= 60_000 {
            let mut updated = record.clone();
            updated.updated_at = now;
            let _ = self
                .store
                .put(&updated, Expect::Version(record.version))
                .await;
        }

        // A disabled user never holds a working session, even one
        // created before the disable (defense in depth).
        match self.user_record_status(&user_id).await {
            Ok(Some(status)) if status == "active" => {}
            Ok(_) => {
                self.delete_session_record(&sid).await?;
                return Err(Error::Forbidden("this account is disabled".to_owned()));
            }
            Err(_) => {}
        }
        let roles = self.roles_of(&user_id).await.unwrap_or_default();
        Ok(AuthedSession {
            auth_user: crate::roles::AuthUser {
                user_id,
                roles,
                session_id: record.id.clone(),
                unlocked,
            },
        })
    }

    pub async fn unlock_through_cache(
        &self,
        user_id: UserId,
        cache_key: &str,
        slot_name: &str,
        slot_key: &riehub_crypto::Key32,
        now: i64,
    ) -> Result<Arc<riehub_vault::Unlocked>> {
        {
            let mut cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(entry) = cache.get_mut(cache_key) {
                entry.last_used_ms = now;
                return Ok(entry.unlocked.clone());
            }
        }
        // Cache miss: unlock the vault through the token slot.
        let stored = riehub_vault::load(self.store.as_ref(), &user_id)
            .await?
            .ok_or_else(|| Error::Unauthorized("no vault for this user".to_owned()))?;
        let unlocked = riehub_vault::unlock(&stored, slot_name, slot_key.expose(), self).await?;
        let unlocked = Arc::new(unlocked);
        let cached = unlocked.clone();
        let mut cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        self.evict_cache(&mut cache, now);
        cache.insert(
            cache_key.to_owned(),
            CacheEntry {
                unlocked,
                last_used_ms: now,
            },
        );
        Ok(cached)
    }

    fn evict_cache(&self, cache: &mut HashMap<String, CacheEntry>, now: i64) {
        cache.retain(|_, entry| now - entry.last_used_ms < UNLOCK_CACHE_IDLE);
        while cache.len() >= UNLOCK_CACHE_MAX {
            let oldest = cache
                .iter()
                .min_by_key(|(_, entry)| entry.last_used_ms)
                .map(|(sid, _)| sid.clone());
            match oldest {
                Some(sid) => {
                    cache.remove(&sid);
                }
                None => break,
            }
        }
    }

    /// The store behind this service, for the CLI readers.
    pub fn store_ref(&self) -> &SqliteStore {
        &self.store
    }

    /// The unlock cache length, for tests.
    pub fn unlock_cache_len(&self) -> usize {
        self.cache.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    async fn delete_session_record(&self, sid: &str) -> Result<()> {
        self.store
            .delete(&Owner::Instance, Kind::Session, &format!("sess_{sid}"))
            .await?;
        self.cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(sid);
        Ok(())
    }
}

/// One session as shown to its user.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub sid: String,
    pub created_at: i64,
    pub last_seen: i64,
    pub expires_at: i64,
}

fn session_record(
    user_id: &UserId,
    full_sid: &str,
    verifier: &[u8; 32],
    created_at: i64,
    expires_at: i64,
    last_seen: i64,
) -> Record {
    let bare = full_sid.trim_start_matches("sess_");
    Record {
        owner: Owner::Instance,
        kind: Kind::Session,
        id: full_sid.to_owned(),
        parent: None,
        lookup: Some(bare.to_owned()),
        status: Some("active".to_owned()),
        sort: 0,
        meta: serde_json::json!({
            "user": user_id.as_str(),
            "sid": bare,
            "expires_at": expires_at,
        }),
        data: Some(verifier.to_vec()),
        version: 1,
        created_at,
        updated_at: last_seen,
    }
}
