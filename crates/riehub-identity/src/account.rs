//! Password change, recovery and the recovery key rotation
//! (identity spec, section 2). Everything here ends sessions or
//! re-wraps the vault, exactly as the spec lists.

use crate::roles::AuthUser;
use crate::{Credential, Service, UserStatus};
use riehub_core::clock::Clock;
use riehub_core::error::{Error, Result};
use riehub_core::{Owner, Secret, SystemClock, UserId};
use riehub_crypto::{hkdf, random_bytes};
use riehub_store::record::{Expect, Kind, Record};
use riehub_store::Store;

/// The pw slot derivations of security spec, section 3.2.
fn pw_slot_key(m: &riehub_crypto::Key32) -> riehub_crypto::Key32 {
    hkdf(m.expose(), crate::PW_SLOT_INFO)
}

fn pw_verifier(m: &riehub_crypto::Key32) -> [u8; 32] {
    *hkdf(m.expose(), crate::PW_AUTH_INFO).expose()
}

fn credential_record(user_id: &UserId, credential: &Credential, version: u64) -> Record {
    let now = SystemClock.now_millis();
    Record {
        owner: Owner::User(user_id.clone()),
        kind: Kind::Credential,
        id: user_id.as_str().to_owned(),
        parent: None,
        lookup: None,
        status: None,
        sort: 0,
        meta: serde_json::json!({}),
        data: Some(crate::encode_credential(credential)),
        version,
        created_at: now,
        updated_at: now,
    }
}

impl Service {
    /// Changes the password: verifies the current one, re-wraps slot
    /// `pw` with a new salt and verifier, and ends every other
    /// session. The caller's own session survives.
    pub async fn change_password(
        &self,
        caller: &AuthUser,
        current: &Secret<String>,
        new: &Secret<String>,
    ) -> Result<()> {
        crate::password::validate(&new.expose().as_bytes().into())?;
        let credential = self.load_credential(&caller.user_id).await?;
        let material = riehub_crypto::argon2id(
            current.expose().as_bytes(),
            &credential.salt,
            riehub_crypto::Argon2Params {
                m_kib: credential.m_kib,
                t: credential.t,
                p: credential.p,
            },
        );
        if !riehub_crypto::ct_eq(&pw_verifier(&material), &credential.verifier) {
            return Err(Error::Unauthorized(
                "the current password is wrong".to_owned(),
            ));
        }

        let new_salt: [u8; 16] = random_bytes::<16>();
        let params = riehub_crypto::Argon2Params::DEFAULT;
        let new_material = riehub_crypto::argon2id(new.expose().as_bytes(), &new_salt, params);
        let new_slot = pw_slot_key(&new_material);
        let new_verifier = pw_verifier(&new_material);

        let mut vault = riehub_vault::load(self.store.as_ref(), &caller.user_id)
            .await?
            .ok_or_else(|| Error::NotFound("no vault for this user".to_owned()))?;
        let expected = vault.file.generation;
        // add_slot replaces the existing `pw` slot in one seal, which
        // keeps the record version and the vault generation in step.
        riehub_vault::add_slot(&mut vault, &caller.unlocked, "pw", new_slot.expose())?;
        riehub_vault::save(self.store.as_ref(), self, &vault, expected).await?;

        let updated_credential = Credential {
            salt: new_salt,
            m_kib: params.m_kib,
            t: params.t,
            p: params.p,
            verifier: new_verifier,
        };
        let record = self
            .store
            .get(
                &Owner::User(caller.user_id.clone()),
                Kind::Credential,
                caller.user_id.as_str(),
            )
            .await?
            .ok_or_else(|| Error::Internal(None))?;
        self.store
            .put(
                &credential_record(&caller.user_id, &updated_credential, record.version + 1),
                Expect::Version(record.version),
            )
            .await?;

        // Every other session ends: the record goes and the slot goes
        // with it. The vault was just re-sealed, so the sweep reloads.
        let sessions = self.list_sessions(&caller.user_id).await?;
        for session in sessions {
            if session.session_id == caller.session_id {
                continue;
            }
            self.end_session(&caller.user_id, &session.sid, &caller.unlocked)
                .await?;
        }
        Ok(())
    }

    /// Recovery: the user presents the recovery key and a new
    /// password. The vault key is recovered, slot `pw` is replaced,
    /// and every session of the user ends.
    pub async fn recover(
        &self,
        email: &Secret<String>,
        recovery_armored: &str,
        new_password: &Secret<String>,
    ) -> Result<UserId> {
        crate::password::validate(&new_password.expose().as_bytes().into())?;
        let email = crate::normalize_email(email.expose())?;
        let user = self
            .user_by_email(&email)
            .await?
            .ok_or_else(|| Error::NotFound("no such user".to_owned()))?;
        let recovery = cofferak_core::crypto::RootPrivateKey::from_armored(recovery_armored)
            .map_err(|_| Error::Unauthorized("the recovery key does not fit".to_owned()))?;

        let vault = riehub_vault::load(self.store.as_ref(), &user.id)
            .await?
            .ok_or_else(|| Error::NotFound("no vault for this user".to_owned()))?;
        let unlocked = riehub_vault::recover(&vault, &recovery)?;

        let new_salt: [u8; 16] = random_bytes::<16>();
        let params = riehub_crypto::Argon2Params::DEFAULT;
        let new_material =
            riehub_crypto::argon2id(new_password.expose().as_bytes(), &new_salt, params);
        let new_slot = pw_slot_key(&new_material);
        let new_verifier = pw_verifier(&new_material);

        let mut vault = vault;
        let expected = vault.file.generation;
        riehub_vault::add_slot(&mut vault, &unlocked, "pw", new_slot.expose())?;
        riehub_vault::save(self.store.as_ref(), self, &vault, expected).await?;

        let record = self
            .store
            .get(
                &Owner::User(user.id.clone()),
                Kind::Credential,
                user.id.as_str(),
            )
            .await?;
        let version = record.map(|r| r.version).unwrap_or(0);
        let credential = Credential {
            salt: new_salt,
            m_kib: params.m_kib,
            t: params.t,
            p: params.p,
            verifier: new_verifier,
        };
        let cas = if version == 0 {
            Expect::New
        } else {
            Expect::Version(version)
        };
        self.store
            .put(
                &credential_record(&user.id, &credential, version.max(1)),
                cas,
            )
            .await?;

        // Every session of the user ends.
        let sessions = self.list_sessions(&user.id).await?;
        for session in sessions {
            self.end_session(&user.id, &session.sid, &unlocked).await?;
        }
        Ok(user.id)
    }

    /// Rotates the recovery key: a new root keypair, a new root wrap,
    /// the new armored key shown once.
    pub async fn rotate_recovery_key(&self, caller: &AuthUser) -> Result<String> {
        let (root_pub, root_priv) =
            cofferak_core::crypto::generate_root_keypair().map_err(|_| Error::Internal(None))?;
        let mut vault = riehub_vault::load(self.store.as_ref(), &caller.user_id)
            .await?
            .ok_or_else(|| Error::NotFound("no vault for this user".to_owned()))?;
        let expected = vault.file.generation;
        vault.file.root_pubkey = root_pub;
        vault.file.root_wrap = Some(
            cofferak_core::crypto::wrap_dek_for_root(
                caller.unlocked.expose(),
                &vault.file.root_pubkey,
            )
            .map_err(|_| Error::Internal(None))?,
        );
        vault.file.seal(caller.unlocked.expose());
        riehub_vault::save(self.store.as_ref(), self, &vault, expected).await?;
        let armored = root_priv.to_armored().as_str().to_owned();
        Ok(armored)
    }

    /// The admin reset: the user goes back to `pending` with a fresh
    /// one-time invite. The vault stays closed and untouched.
    pub async fn admin_reset(&self, admin: &AuthUser, target: &str) -> Result<String> {
        if !admin.has_permission(crate::roles::Permission::UsersManage) {
            return Err(Error::Forbidden("this needs users:manage".to_owned()));
        }
        let user_id = UserId::parse(target)?;
        let record = self
            .store
            .get(&Owner::Instance, Kind::User, user_id.as_str())
            .await?
            .ok_or_else(|| Error::NotFound("no such user".to_owned()))?;
        let token_bytes: [u8; 32] = random_bytes::<32>();
        let token: String = token_bytes.iter().map(|b| format!("{b:02x}")).collect();
        let mut meta = record.meta.clone();
        if let Some(obj) = meta.as_object_mut() {
            obj.insert(
                "invite_hash".to_owned(),
                serde_json::json!(crate::invite::invite_hash(&token)),
            );
            obj.insert(
                "invite_expires_at".to_owned(),
                serde_json::json!(
                    self.clock.now_millis() + crate::invite::INVITE_TTL.as_millis() as i64
                ),
            );
        }
        let updated = Record {
            status: Some(UserStatus::Pending.as_str().to_owned()),
            meta,
            version: record.version + 1,
            updated_at: self.clock.now_millis(),
            ..record
        };
        self.store
            .put(&updated, Expect::Version(record.version))
            .await?;
        // The user's sessions end here too: the account is locked out.
        for session in self.list_sessions(&user_id).await? {
            let _ = self
                .store
                .delete(&Owner::Instance, Kind::Session, &session.session_id)
                .await;
        }
        Ok(token)
    }

    /// Accepts an invite or a reset. With a recovery key the vault is
    /// recovered and re-wrapped; without one the old vault is deleted
    /// and a fresh one is created ("start empty"). Returns the user id
    /// and, for a fresh vault, the recovery key shown once.
    pub async fn accept_invite(
        &self,
        token: &str,
        password: &Secret<String>,
        recovery_armored: Option<&str>,
    ) -> Result<(UserId, Option<String>)> {
        crate::password::validate(&password.expose().as_bytes().into())?;
        let invite = self.invite_by_token(token).await?;
        let user_id = invite.user_id.clone();
        let user_record = self
            .store
            .get(&Owner::Instance, Kind::User, user_id.as_str())
            .await?
            .ok_or_else(|| Error::NotFound("no such invite".to_owned()))?;
        let had_vault = riehub_vault::load(self.store.as_ref(), &user_id)
            .await?
            .is_some();

        let new_salt: [u8; 16] = random_bytes::<16>();
        let params = riehub_crypto::Argon2Params::DEFAULT;
        let new_material = riehub_crypto::argon2id(password.expose().as_bytes(), &new_salt, params);
        let new_slot = pw_slot_key(&new_material);
        let new_verifier = pw_verifier(&new_material);

        let (armored, credential) = match (had_vault, recovery_armored) {
            (true, Some(armored)) => {
                // "I have my recovery key": the vault is recovered,
                // slot `pw` re-wrapped, entries stay.
                let recovery = cofferak_core::crypto::RootPrivateKey::from_armored(armored)
                    .map_err(|_| Error::Unauthorized("the recovery key does not fit".to_owned()))?;
                let vault = riehub_vault::load(self.store.as_ref(), &user_id)
                    .await?
                    .ok_or_else(|| Error::NotFound("no vault for this user".to_owned()))?;
                let unlocked = riehub_vault::recover(&vault, &recovery)?;
                let expected = vault.file.generation;
                let mut vault = vault;
                riehub_vault::add_slot(&mut vault, &unlocked, "pw", new_slot.expose())?;
                riehub_vault::save(self.store.as_ref(), self, &vault, expected).await?;
                let credential = Credential {
                    salt: new_salt,
                    m_kib: params.m_kib,
                    t: params.t,
                    p: params.p,
                    verifier: new_verifier,
                };
                (None, credential)
            }
            (true, None) => {
                // "Start empty": the old vault and its keys are gone.
                self.store
                    .delete(&Owner::User(user_id.clone()), Kind::Vault, user_id.as_str())
                    .await?;
                self.store
                    .delete(
                        &Owner::User(user_id.clone()),
                        Kind::Credential,
                        user_id.as_str(),
                    )
                    .await?;
                let (vault, recovery) =
                    riehub_vault::create(user_id.clone(), ("pw", new_slot.expose()))?;
                riehub_vault::save(self.store.as_ref(), self, &vault, 0).await?;
                let credential = Credential {
                    salt: new_salt,
                    m_kib: params.m_kib,
                    t: params.t,
                    p: params.p,
                    verifier: new_verifier,
                };
                (
                    Some(recovery.private.to_armored().as_str().to_owned()),
                    credential,
                )
            }
            (false, _) => {
                // First setup through the invite.
                let (vault, recovery) =
                    riehub_vault::create(user_id.clone(), ("pw", new_slot.expose()))?;
                riehub_vault::save(self.store.as_ref(), self, &vault, 0).await?;
                let credential = Credential {
                    salt: new_salt,
                    m_kib: params.m_kib,
                    t: params.t,
                    p: params.p,
                    verifier: new_verifier,
                };
                (
                    Some(recovery.private.to_armored().as_str().to_owned()),
                    credential,
                )
            }
        };

        if had_vault && recovery_armored.is_some() {
            let version = self
                .store
                .get(
                    &Owner::User(user_id.clone()),
                    Kind::Credential,
                    user_id.as_str(),
                )
                .await?
                .map(|r| r.version)
                .unwrap_or(0);
            self.store
                .put(
                    &credential_record(&user_id, &credential, version + 1),
                    Expect::Version(version),
                )
                .await?;
        } else {
            self.store
                .put(&credential_record(&user_id, &credential, 1), Expect::New)
                .await?;
        }

        let mut meta = user_record.meta.clone();
        if let Some(obj) = meta.as_object_mut() {
            obj.remove("invite_hash");
            obj.remove("invite_expires_at");
        }
        let updated = Record {
            status: Some(UserStatus::Active.as_str().to_owned()),
            meta,
            version: user_record.version + 1,
            updated_at: self.clock.now_millis(),
            ..user_record
        };
        self.store
            .put(&updated, Expect::Version(user_record.version))
            .await?;

        self.audit_event(
            riehub_audit::event::Actor::User(user_id.clone()),
            riehub_audit::event::AuditAction::UserCreate,
            riehub_audit::event::Outcome::Ok,
            Some(user_id.clone()),
            riehub_audit::event::AuditEvent::UserCreate {
                user: user_id.clone(),
            },
        )
        .await?;
        Ok((user_id, armored))
    }
}
