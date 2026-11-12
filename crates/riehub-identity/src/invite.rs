//! Invites (identity spec, section 1): an admin creates a pending
//! user and a one-time invite link valid for 72 hours. The invite
//! token is high-entropy; the record holds only its hash.

use crate::roles::Role;
use crate::{AuthKind, Credential, Service, User, UserStatus};
use riehub_core::error::{Error, Result};
use riehub_core::{Owner, Secret, UserId};
use riehub_crypto::random_bytes;
use riehub_store::record::{Expect, Kind, Record};
use riehub_store::Store;
use std::time::Duration;

pub const INVITE_TTL: Duration = Duration::from_secs(72 * 3600);

fn invite_hash(token: &str) -> String {
    // The token is 256 bits of randomness, so a keyed hash with a
    // fixed key is enough to keep it out of the record in the clear.
    let tag = riehub_crypto::hmac(&riehub_crypto::Key32::new([0u8; 32]), token.as_bytes());
    tag.iter().map(|b| format!("{b:02x}")).collect()
}

/// One pending invite as the admin sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    pub user_id: UserId,
    pub email: String,
    pub display_name: String,
    pub expires_at: i64,
}

fn invite_of_record(record: &Record) -> Option<Invite> {
    let meta = record.meta.as_object()?;
    meta.get("invite_hash").and_then(|v| v.as_str())?;
    Some(Invite {
        user_id: UserId::parse(&record.id).ok()?,
        email: record.lookup.clone()?,
        display_name: meta
            .get("display_name")
            .and_then(|v| v.as_str())
            .unwrap_or("unnamed")
            .to_owned(),
        expires_at: meta.get("invite_expires_at").and_then(|v| v.as_i64())?,
    })
}

impl Service {
    /// Creates a pending user with a one-time invite token. Admin
    /// only.
    pub async fn create_invite(
        &self,
        email: &str,
        display_name: Option<&str>,
    ) -> Result<(User, String)> {
        let email = crate::normalize_email(email)?;
        if self.user_by_email(&email).await?.is_some() {
            return Err(Error::Conflict("this email already exists".to_owned()));
        }
        let user_id = UserId::new();
        let token_bytes = random_bytes::<32>();
        let token: String = token_bytes.iter().map(|b| format!("{b:02x}")).collect();
        let now = self.clock.now_millis();
        let user = User {
            id: user_id.clone(),
            email: email.clone(),
            display_name: display_name.unwrap_or("unnamed").chars().take(80).collect(),
            status: UserStatus::Pending,
            roles: vec![Role::Member.as_str().to_owned()],
            auth: AuthKind::Password,
        };
        let record = Record {
            owner: Owner::Instance,
            kind: Kind::User,
            id: user.id.as_str().to_owned(),
            parent: None,
            lookup: Some(user.email.clone()),
            status: Some(user.status.as_str().to_owned()),
            sort: 0,
            meta: serde_json::json!({
                "display_name": user.display_name,
                "roles": user.roles.iter().map(|r| r.as_str()).collect::<Vec<_>>(),
                "auth": user.auth.as_str(),
                "invite_hash": invite_hash(&token),
                "invite_expires_at": now + INVITE_TTL.as_millis() as i64,
            }),
            data: None,
            version: 1,
            created_at: now,
            updated_at: now,
        };
        self.store.put(&record, Expect::New).await?;
        Ok((user, token))
    }

    /// Looks up a pending invite by its token.
    pub async fn invite_by_token(&self, token: &str) -> Result<Invite> {
        let hash = invite_hash(token);
        let pending = self.pending_invites().await?;
        let invite = pending
            .into_iter()
            .find(|invite| {
                // The record holds the hash; compare in constant time.
                let record_hash = invite.1.clone();
                riehub_crypto::ct_eq(record_hash.as_bytes(), hash.as_bytes())
            })
            .map(|(invite, _)| invite)
            .ok_or_else(|| Error::NotFound("no such invite".to_owned()))?;
        let now = self.clock.now_millis();
        if now > invite.expires_at {
            return Err(Error::Invalid("this invite has expired".to_owned()));
        }
        Ok(invite)
    }

    /// The pending invites with their record hash, for the lookup.
    async fn pending_invites(&self) -> Result<Vec<(Invite, String)>> {
        let mut out = Vec::new();
        let mut cursor = None;
        loop {
            let page = self
                .store
                .list(
                    &Owner::Instance,
                    Kind::User,
                    &riehub_store::record::Filter {
                        status: Some(UserStatus::Pending.as_str().to_owned()),
                        ..Default::default()
                    },
                    &riehub_store::record::Page {
                        after: cursor.clone(),
                        limit: 200,
                    },
                )
                .await?;
            for record in &page.items {
                let meta = record
                    .meta
                    .as_object()
                    .ok_or_else(|| Error::Internal(None))?;
                let Some(hash) = meta.get("invite_hash").and_then(|v| v.as_str()) else {
                    continue;
                };
                if let Some(invite) = invite_of_record(record) {
                    out.push((invite, hash.to_owned()));
                }
            }
            match page.next {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        Ok(out)
    }

    /// Accepts an invite: sets the password, creates the vault with
    /// slot `pw` and the credential, and activates the user. Single
    /// use: the hash is removed on success.
    pub async fn accept_invite(
        &self,
        token: &str,
        password: &Secret<String>,
    ) -> Result<(UserId, String)> {
        crate::password::validate(&password.expose().as_bytes().into())?;
        let invite = self.invite_by_token(token).await?;
        let record = self
            .store
            .get(&Owner::Instance, Kind::User, invite.user_id.as_str())
            .await?
            .ok_or_else(|| Error::NotFound("no such invite".to_owned()))?;

        let salt: [u8; 16] = random_bytes::<16>();
        let params = riehub_crypto::Argon2Params::DEFAULT;
        let pw_material = riehub_crypto::argon2id(password.expose().as_bytes(), &salt, params);
        let pw_key = crate::pw_slot_key(&pw_material);
        let verifier = crate::pw_verifier(&pw_material);

        let (vault, recovery) =
            riehub_vault::create(invite.user_id.clone(), ("pw", pw_key.expose()))?;
        riehub_vault::save(self.store.as_ref(), self, &vault, 0).await?;
        let armored = recovery.private.to_armored().as_str().to_owned();
        let credential = Credential {
            salt,
            m_kib: params.m_kib,
            t: params.t,
            p: params.p,
            verifier,
        };
        let credential_record = Record {
            owner: Owner::User(invite.user_id.clone()),
            kind: Kind::Credential,
            id: invite.user_id.as_str().to_owned(),
            parent: None,
            lookup: None,
            status: None,
            sort: 0,
            meta: serde_json::json!({}),
            data: Some(crate::encode_credential(&credential)),
            version: 1,
            created_at: self.clock.now_millis(),
            updated_at: self.clock.now_millis(),
        };
        self.store.put(&credential_record, Expect::New).await?;

        let mut meta = record.meta.clone();
        if let Some(obj) = meta.as_object_mut() {
            obj.remove("invite_hash");
            obj.remove("invite_expires_at");
        }
        let updated = Record {
            status: Some(UserStatus::Active.as_str().to_owned()),
            meta,
            version: record.version + 1,
            updated_at: self.clock.now_millis(),
            ..record
        };
        self.store
            .put(&updated, Expect::Version(record.version))
            .await?;

        self.audit_event(
            riehub_audit::event::Actor::User(invite.user_id.clone()),
            riehub_audit::event::AuditAction::UserCreate,
            riehub_audit::event::Outcome::Ok,
            Some(invite.user_id.clone()),
            riehub_audit::event::AuditEvent::UserCreate {
                user: invite.user_id.clone(),
            },
        )
        .await?;

        Ok((invite.user_id, armored))
    }
}
