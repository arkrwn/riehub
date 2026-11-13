//! riehub-identity: users, setup and password login (identity spec,
//! sections 1 and 2, security spec section 3.1). Passwords run
//! through Argon2id with per-user parameters, verifiers are HKDF
//! outputs, the vault unlocks through the `pw` slot, and login
//! throttles through store counters.

#![forbid(unsafe_code)]

pub mod account;
pub mod admin;
pub mod invite;
pub mod keys;

pub use roles::AdminSafe;
pub mod password;
pub mod roles;
pub mod session;

use riehub_audit::event::{
    Actor, AuditAction, AuditEvent, Event, LoginFailReason, Outcome, Target, ThrottleScope,
};
use riehub_core::clock::Clock;
use riehub_core::error::{Error, Result};
use riehub_core::{Owner, Secret, SystemClock, UserId};
use riehub_crypto::{hkdf, random_bytes};
use riehub_store::record::{Expect, Kind, Record};
use riehub_store::sqlite::SqliteStore;
use riehub_store::{CounterKey, Store};
use riehub_vault::{self as vault, VaultAudit};
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const ARGON2_CONCURRENCY: usize = 4;
const ARGON2_WAIT: std::time::Duration = std::time::Duration::from_secs(5);
const THROTTLE_WINDOW_MS: i64 = 900_000; // 15 minutes
const ACCOUNT_FAIL_LIMIT: i64 = 5;
const IP_FAIL_LIMIT: i64 = 20;
pub(crate) const PW_SLOT_INFO: &str = "riehub/v1/pw/slot";
pub(crate) const PW_AUTH_INFO: &str = "riehub/v1/pw/auth";
const DUMMY_SALT: [u8; 16] = [0x5a; 16];

/// One user of the hub (identity spec, section 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    pub email: String,
    pub display_name: String,
    pub status: UserStatus,
    pub roles: Vec<String>,
    pub auth: AuthKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserStatus {
    Active,
    Disabled,
    Pending,
}

impl UserStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserStatus::Active => "active",
            UserStatus::Disabled => "disabled",
            UserStatus::Pending => "pending",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "active" => Ok(UserStatus::Active),
            "disabled" => Ok(UserStatus::Disabled),
            "pending" => Ok(UserStatus::Pending),
            _ => Err(Error::Invalid("unknown user status".to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    Password,
}

impl AuthKind {
    fn as_str(&self) -> &'static str {
        match self {
            AuthKind::Password => "password",
        }
    }
}

/// The stored per-user credential: salt, Argon2 parameters, verifier.
pub(crate) struct Credential {
    salt: [u8; 16],
    m_kib: u32,
    t: u32,
    p: u32,
    verifier: [u8; 32],
}

/// The identity service over one store and one audit log. No
/// `Debug`: it holds credential material in flight.
#[derive(Clone)]
#[allow(missing_debug_implementations)]
pub struct Service {
    store: Arc<SqliteStore>,
    audit: Arc<riehub_audit::AuditLog<SqliteStore>>,
    argon: Arc<Semaphore>,
    cache: Arc<Mutex<std::collections::HashMap<String, session::CacheEntry>>>,
    clock: Arc<dyn Clock>,
    test_clock: Option<Arc<riehub_core::TestClock>>,
}

impl Service {
    pub fn new(store: Arc<SqliteStore>, audit: Arc<riehub_audit::AuditLog<SqliteStore>>) -> Self {
        Self {
            store,
            audit,
            argon: Arc::new(Semaphore::new(ARGON2_CONCURRENCY)),
            cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            clock: Arc::new(riehub_core::SystemClock),
            test_clock: None,
        }
    }

    /// Swaps in a test clock for the session timeouts. Tests only.
    pub fn with_test_clock(mut self, clock: Arc<riehub_core::TestClock>) -> Self {
        self.clock = clock.clone();
        self.test_clock = Some(clock);
        self
    }

    /// One Argon2 slot. At most four run at once; the rest wait up to
    /// five seconds, then the request fails closed with a 503.
    pub async fn acquire_argon(&self) -> Result<OwnedSemaphorePermit> {
        match tokio::time::timeout(ARGON2_WAIT, self.argon.clone().acquire_owned()).await {
            Ok(Ok(permit)) => Ok(permit),
            _ => Err(Error::Unavailable(
                "the argon2 concurrency limit is saturated".to_owned(),
            )),
        }
    }

    fn window(now_ms: i64) -> (i64, i64) {
        let window = now_ms.div_euclid(THROTTLE_WINDOW_MS);
        (window, (window + 1) * THROTTLE_WINDOW_MS)
    }

    /// The throttle state: failures for the account and the ip this
    /// window, plus the seconds until the window closes. The handler
    /// turns that into `Retry-After`.
    pub async fn throttle_state(&self, email: &str, ip: Option<&str>) -> (i64, i64, i64) {
        let now = SystemClock.now_millis();
        let (window, window_end) = Self::window(now);
        let account = self
            .store
            .counter_get(&account_key(email), window)
            .await
            .unwrap_or(0);
        let ip_fails = match ip {
            Some(ip) => self
                .store
                .counter_get(&ip_key(ip), window)
                .await
                .unwrap_or(0),
            None => 0,
        };
        let retry_after = ((window_end - now) / 1000).max(1);
        (account, ip_fails, retry_after)
    }

    async fn count_failure(&self, email: &str, ip: Option<&str>) -> Result<()> {
        let now = SystemClock.now_millis();
        let (window, window_end) = Self::window(now);
        self.store
            .counter_add(&account_key(email), window, 1, window_end)
            .await?;
        if let Some(ip) = ip {
            self.store
                .counter_add(&ip_key(ip), window, 1, window_end)
                .await?;
        }
        Ok(())
    }

    /// Creates the first user with the owner role. Works only while
    /// no user exists.
    pub async fn setup_owner(
        &self,
        email: &str,
        password: &Secret<String>,
        display_name: Option<&str>,
    ) -> std::result::Result<(User, String), riehub_core::Error> {
        let email = normalize_email(email)?;
        password::validate(&password.expose().as_bytes().into())?;
        if self.any_user().await? {
            return Err(Error::Conflict(
                "setup: the instance already has users".to_owned(),
            ));
        }
        let (user, credential, recovery) = self
            .make_user(&email, password, display_name, &["owner"])
            .await?;
        self.write_new_user(&user, &credential).await?;
        self.audit_event(
            Actor::System,
            AuditAction::UserCreate,
            Outcome::Ok,
            Some(user.id.clone()),
            AuditEvent::UserCreate {
                user: user.id.clone(),
            },
        )
        .await?;
        Ok((user, recovery))
    }

    /// Password login. An unknown email runs the same Argon2 work on
    /// a dummy record, and every failure looks the same.
    pub async fn login(
        &self,
        email: &Secret<String>,
        password: &Secret<String>,
        ip: Option<&str>,
        cookie_key: &[u8; 32],
    ) -> Result<(UserId, session::Session)> {
        let email_raw = normalize_email(email.expose())?;
        let now = SystemClock.now_millis();
        let (window, _end) = Self::window(now);
        let account_fails = self
            .store
            .counter_get(&account_key(&email_raw), window)
            .await
            .unwrap_or(0);
        if account_fails >= ACCOUNT_FAIL_LIMIT {
            self.audit_event(
                Actor::Anonymous,
                AuditAction::LoginThrottled,
                Outcome::Denied,
                None,
                AuditEvent::LoginThrottled {
                    scope: ThrottleScope::Account,
                },
            )
            .await?;
            return Err(Error::RateLimited("too many failed logins".to_owned()));
        }
        if let Some(ip) = ip {
            let ip_fails = self
                .store
                .counter_get(&ip_key(ip), window)
                .await
                .unwrap_or(0);
            if ip_fails >= IP_FAIL_LIMIT {
                self.audit_event(
                    Actor::Anonymous,
                    AuditAction::LoginThrottled,
                    Outcome::Denied,
                    None,
                    AuditEvent::LoginThrottled {
                        scope: ThrottleScope::Ip,
                    },
                )
                .await?;
                return Err(Error::RateLimited("too many failed logins".to_owned()));
            }
        }

        let user = self.user_by_email(&email_raw).await?;
        let _permit = self.acquire_argon().await?;

        let Some(user) = user else {
            // Same work for an unknown email so timing does not reveal
            // which emails exist.
            let _dummy = riehub_crypto::argon2id(
                password.expose().as_bytes(),
                &DUMMY_SALT,
                riehub_crypto::Argon2Params::DEFAULT,
            );
            self.count_failure(&email_raw, ip).await?;
            self.audit_event(
                Actor::Anonymous,
                AuditAction::LoginFailed,
                Outcome::Denied,
                None,
                AuditEvent::LoginFailed {
                    reason: LoginFailReason::UnknownEmail,
                },
            )
            .await?;
            return Err(invalid_login());
        };

        let credential = self.load_credential(&user.id).await?;
        let pw_material = riehub_crypto::argon2id(
            password.expose().as_bytes(),
            &credential.salt,
            riehub_crypto::Argon2Params {
                m_kib: credential.m_kib,
                t: credential.t,
                p: credential.p,
            },
        );
        let pw_key = pw_slot_key(&pw_material);
        let computed = pw_verifier(&pw_material);
        if !riehub_crypto::ct_eq(&computed, &credential.verifier) {
            self.count_failure(&email_raw, ip).await?;
            self.audit_event(
                Actor::User(user.id.clone()),
                AuditAction::LoginFailed,
                Outcome::Denied,
                None,
                AuditEvent::LoginFailed {
                    reason: LoginFailReason::WrongPassword,
                },
            )
            .await?;
            return Err(invalid_login());
        }
        if user.status != UserStatus::Active {
            self.audit_event(
                Actor::User(user.id.clone()),
                AuditAction::LoginFailed,
                Outcome::Denied,
                None,
                AuditEvent::LoginFailed {
                    reason: LoginFailReason::Disabled,
                },
            )
            .await?;
            return Err(Error::Forbidden("this account is disabled".to_owned()));
        }

        // Unlock the vault through the pw slot, then create the
        // session: slot `ws.<SID>`, session record, cookie token.
        let stored = vault::load(self.store.as_ref(), &user.id).await?;
        let Some(stored) = stored else {
            return Err(Error::Internal(None));
        };
        let unlocked = Arc::new(vault::unlock(&stored, "pw", pw_key.expose(), self).await?);
        let session = self
            .create_session(&user.id, &stored, unlocked, cookie_key)
            .await?;

        self.audit_event(
            Actor::User(user.id.clone()),
            AuditAction::LoginOk,
            Outcome::Ok,
            None,
            AuditEvent::LoginOk {
                user: user.id.clone(),
            },
        )
        .await?;
        Ok((user.id, session))
    }

    async fn any_user(&self) -> Result<bool> {
        let listed = self
            .store
            .list(
                &Owner::Instance,
                Kind::User,
                &Default::default(),
                &riehub_store::record::Page {
                    after: None,
                    limit: 1,
                },
            )
            .await?;
        Ok(!listed.items.is_empty())
    }

    /// The status string of one user's record, None if the record is
    /// gone.
    pub async fn user_record_status(&self, user_id: &UserId) -> Result<Option<String>> {
        let record = self
            .store
            .get(&Owner::Instance, Kind::User, user_id.as_str())
            .await?;
        Ok(record.and_then(|r| r.status))
    }

    /// The roles of one user, from the instance-owned user record.
    pub async fn roles_of(&self, user_id: &UserId) -> Result<Vec<crate::roles::Role>> {
        let record = self
            .store
            .get(&Owner::Instance, Kind::User, user_id.as_str())
            .await?;
        match record {
            None => Ok(vec![crate::roles::Role::Member]),
            Some(record) => Ok(record
                .meta
                .get("roles")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().and_then(crate::roles::Role::parse))
                        .collect()
                })
                .unwrap_or_default()),
        }
    }

    pub async fn user_by_email(&self, email: &str) -> Result<Option<User>> {
        let record = self
            .store
            .get_by_lookup(&Owner::Instance, Kind::User, email)
            .await?;
        match record {
            None => Ok(None),
            Some(record) => Ok(Some(user_from_record(&record)?)),
        }
    }

    async fn load_credential(&self, user_id: &UserId) -> Result<Credential> {
        let record = self
            .store
            .get(
                &Owner::User(user_id.clone()),
                Kind::Credential,
                user_id.as_str(),
            )
            .await?
            .ok_or_else(|| Error::NotFound("no credential for this user".to_owned()))?;
        let data = record.data.ok_or_else(|| Error::Internal(None))?;
        decode_credential(&data).ok_or_else(|| Error::Internal(None))
    }

    /// Creates a user record set: the user, its vault with slot `pw`,
    /// and its credential. Returns the armored recovery key, shown
    /// once.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn make_user(
        &self,
        email: &str,
        password: &Secret<String>,
        display_name: Option<&str>,
        roles: &[&str],
    ) -> Result<(User, Credential, String)> {
        let user_id = UserId::new();
        let salt: [u8; 16] = random_bytes::<16>();
        let params = riehub_crypto::Argon2Params::DEFAULT;
        let pw_material = riehub_crypto::argon2id(password.expose().as_bytes(), &salt, params);
        let pw_key = pw_slot_key(&pw_material);
        let verifier = pw_verifier(&pw_material);

        let (vault, recovery) = vault::create(user_id.clone(), ("pw", pw_key.expose()))?;
        let armored = recovery.private.to_armored().as_str().to_owned();
        let _recovery = recovery;
        let version = vault::save(self.store.as_ref(), self, &vault, 0).await?;

        let user = User {
            id: user_id.clone(),
            email: email.to_owned(),
            display_name: display_name.unwrap_or("unnamed").chars().take(80).collect(),
            status: UserStatus::Active,
            roles: roles.iter().map(|s| s.to_string()).collect(),
            auth: AuthKind::Password,
        };
        let credential = Credential {
            salt,
            m_kib: params.m_kib,
            t: params.t,
            p: params.p,
            verifier,
        };
        let _ = version;
        Ok((user, credential, armored))
    }

    async fn write_new_user(&self, user: &User, credential: &Credential) -> Result<()> {
        let now = SystemClock.now_millis();
        let meta = serde_json::json!({
            "email": user.email,
            "display_name": user.display_name,
            "roles": user.roles,
            "auth": user.auth.as_str(),
        });
        let user_record = Record {
            owner: Owner::Instance,
            kind: Kind::User,
            id: user.id.as_str().to_owned(),
            parent: None,
            lookup: Some(user.email.clone()),
            status: Some(user.status.as_str().to_owned()),
            sort: 0,
            meta,
            data: None,
            version: 1,
            created_at: now,
            updated_at: now,
        };
        let credential_record = Record {
            owner: Owner::User(user.id.clone()),
            kind: Kind::Credential,
            id: user.id.as_str().to_owned(),
            parent: None,
            lookup: None,
            status: None,
            sort: 0,
            meta: serde_json::json!({}),
            data: Some(encode_credential(credential)),
            version: 1,
            created_at: now,
            updated_at: now,
        };
        self.store.put(&user_record, Expect::New).await?;
        self.store.put(&credential_record, Expect::New).await?;
        Ok(())
    }

    pub(crate) async fn audit_event(
        &self,
        actor: Actor,
        action: AuditAction,
        outcome: Outcome,
        target: Option<UserId>,
        event: AuditEvent,
    ) -> Result<()> {
        let event = Event {
            ts: SystemClock.now_millis(),
            actor,
            action,
            outcome,
            request_id: None,
            target: target.map(Target::User),
            ip: None,
            event,
        };
        self.audit.append(vec![event]).await?;
        Ok(())
    }
}

fn invalid_login() -> Error {
    Error::Unauthorized("invalid email or password".to_owned())
}

fn counter_tag(value: &str) -> String {
    // The caller text never lands in a counter name: it is hashed
    // first (security spec, section 6).
    let tag = riehub_crypto::hmac(&riehub_crypto::Key32::new([0u8; 32]), value.as_bytes());
    tag.iter().map(|b| format!("{b:02x}")).collect()
}

fn account_key(email: &str) -> CounterKey {
    CounterKey::new(
        Owner::Instance,
        format!("login:user:{}", counter_tag(email)),
    )
}

fn ip_key(ip: &str) -> CounterKey {
    CounterKey::new(Owner::Instance, format!("login:ip:{}", counter_tag(ip)))
}

/// The slot key: `hkdf(m, riehub/v1/pw/slot)` (security spec, 3.2).
pub(crate) fn pw_slot_key(m: &riehub_crypto::Key32) -> riehub_crypto::Key32 {
    hkdf(m.expose(), PW_SLOT_INFO)
}

/// The stored verifier: `hkdf(m, riehub/v1/pw/auth)`.
pub(crate) fn pw_verifier(m: &riehub_crypto::Key32) -> [u8; 32] {
    *hkdf(m.expose(), PW_AUTH_INFO).expose()
}

pub(crate) fn encode_credential(c: &Credential) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + 12 + 32);
    out.extend_from_slice(&c.salt);
    out.extend_from_slice(&c.m_kib.to_le_bytes());
    out.extend_from_slice(&c.t.to_le_bytes());
    out.extend_from_slice(&c.p.to_le_bytes());
    out.extend_from_slice(&c.verifier);
    out
}

fn decode_credential(data: &[u8]) -> Option<Credential> {
    if data.len() != 16 + 12 + 32 {
        return None;
    }
    let mut salt = [0u8; 16];
    salt.copy_from_slice(&data[..16]);
    let m_kib = u32::from_le_bytes(data[16..20].try_into().ok()?);
    let t = u32::from_le_bytes(data[20..24].try_into().ok()?);
    let p = u32::from_le_bytes(data[24..28].try_into().ok()?);
    let mut verifier = [0u8; 32];
    verifier.copy_from_slice(&data[28..]);
    Some(Credential {
        salt,
        m_kib,
        t,
        p,
        verifier,
    })
}

pub(crate) fn normalize_email(email: &str) -> Result<String> {
    let trimmed = email.trim().to_lowercase();
    if trimmed.is_empty() || trimmed.len() > 254 || !trimmed.contains('@') {
        return Err(Error::Invalid("email: not a usable address".to_owned()));
    }
    Ok(trimmed)
}

pub(crate) fn user_from_record(record: &Record) -> Result<User> {
    let id = UserId::parse(&record.id)?;
    let email = record.lookup.clone().ok_or_else(|| Error::Internal(None))?;
    let meta = record
        .meta
        .as_object()
        .ok_or_else(|| Error::Internal(None))?;
    let display_name = meta
        .get("display_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unnamed")
        .to_owned();
    let roles = meta
        .get("roles")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let status = record
        .status
        .as_deref()
        .map(UserStatus::parse)
        .transpose()?
        .unwrap_or(UserStatus::Pending);
    Ok(User {
        id,
        email,
        display_name,
        status,
        roles,
        auth: AuthKind::Password,
    })
}

// The vault audit sink: the identity service writes vault.write
// events through the audit log it holds, and reads the newest
// audited generation back for the rollback anchor.
#[async_trait::async_trait]
impl VaultAudit for Service {
    async fn newest_generation(&self, user_id: &UserId) -> Result<u64> {
        let mut next = 1u64;
        let mut newest = 0u64;
        loop {
            let rows = self
                .store
                .audit_read(next, 1000, &Default::default())
                .await?;
            if rows.is_empty() {
                break;
            }
            for row in &rows {
                if row.kind == "vault.write" {
                    if let Some(user) = row.event.get("user") {
                        if user.as_str() == Some(user_id.as_str()) {
                            if let Some(gen) = row.event.get("generation").and_then(|g| g.as_u64())
                            {
                                newest = newest.max(gen);
                            }
                        }
                    }
                }
            }
            next = rows.last().map(|r| r.seq + 1).unwrap_or(next);
            if rows.len() < 1000 {
                break;
            }
        }
        Ok(newest)
    }

    async fn vault_write(&self, user_id: &UserId, generation: u64) -> Result<()> {
        let event = Event {
            ts: SystemClock.now_millis(),
            actor: Actor::User(user_id.clone()),
            action: AuditAction::VaultWrite,
            outcome: Outcome::Ok,
            request_id: None,
            target: None,
            ip: None,
            event: AuditEvent::VaultWrite {
                user: user_id.clone(),
                generation,
            },
        };
        self.audit.append(vec![event]).await?;
        Ok(())
    }
}
