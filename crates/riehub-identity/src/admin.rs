//! Admin user administration (identity spec, section 5): patch roles
//! and status, delete users, and the last-owner protection.

use crate::roles::{AuthUser, Permission, Role};
use crate::{Service, User, UserStatus};
use riehub_core::clock::Clock;
use riehub_core::error::{Error, Result};
use riehub_core::{Owner, SystemClock, UserId};
use riehub_store::record::{Expect, Kind, Record};
use riehub_store::Store;
use serde_json::Value;

#[derive(Debug)]
pub struct UserPatch {
    pub roles: Option<Vec<String>>,
    pub status: Option<String>,
}

impl Service {
    /// Patches roles and status of one user. Admin only. The last
    /// owner cannot be demoted or disabled.
    pub async fn patch_user(
        &self,
        admin: &AuthUser,
        target: &str,
        patch: &UserPatch,
    ) -> Result<()> {
        if !admin.has_permission(Permission::UsersManage) {
            return Err(Error::Forbidden("this needs users:manage".to_owned()));
        }
        // Granting the owner role is an owner-only decision (identity
        // spec, section 5): an admin escalates nobody.
        let grants_owner = patch
            .roles
            .as_ref()
            .map(|roles| roles.iter().any(|r| Role::parse(r) == Some(Role::Owner)))
            .unwrap_or(false);
        if grants_owner && !admin.is_owner() {
            return Err(Error::Forbidden(
                "only an owner assigns the owner role".to_owned(),
            ));
        }
        let user_id = UserId::parse(target)?;
        let record = self
            .store
            .get(&Owner::Instance, Kind::User, user_id.as_str())
            .await?
            .ok_or_else(|| Error::NotFound("no such user".to_owned()))?;

        let mut meta = record.meta.clone();
        let mut new_roles: Vec<String> = match &patch.roles {
            Some(roles) => {
                let mut mapped = Vec::new();
                for r in roles {
                    let role =
                        Role::parse(r).ok_or_else(|| Error::Invalid("unknown role".to_owned()))?;
                    mapped.push(role.as_str().to_owned());
                }
                mapped
            }
            None => meta
                .get("roles")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        };
        let new_status = match &patch.status {
            Some(status) => match status.as_str() {
                "active" => UserStatus::Active,
                "disabled" => UserStatus::Disabled,
                "pending" => UserStatus::Pending,
                _ => return Err(Error::Invalid("unknown status".to_owned())),
            },
            None => record
                .status
                .as_deref()
                .map(|s| match s {
                    "active" => UserStatus::Active,
                    "disabled" => UserStatus::Disabled,
                    _ => UserStatus::Pending,
                })
                .unwrap_or(UserStatus::Pending),
        };

        // The owner check reads the roles as they are stored now: a
        // patch that removes the owner role must hit the guard.
        let currently_owner = record
            .meta
            .get("roles")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().any(|r| r.as_str() == Some("owner")))
            .unwrap_or(false);
        let stays_owner = currently_owner
            && new_roles.iter().any(|r| r == "owner")
            && !matches!(new_status, UserStatus::Disabled);
        if currently_owner && !stays_owner {
            let owners = self.active_owners().await?;
            if owners <= 1 {
                return Err(Error::Conflict(
                    "the last owner cannot be demoted or disabled".to_owned(),
                ));
            }
        }
        if new_roles.is_empty() {
            new_roles = vec![Role::Member.as_str().to_owned()];
        }
        if let Some(obj) = meta.as_object_mut() {
            obj.insert("roles".to_owned(), serde_json::json!(new_roles));
        }

        // A newly disabled user loses every live session here.
        if matches!(new_status, UserStatus::Disabled) {
            for session in self.list_sessions(&user_id).await? {
                let _ = self
                    .store
                    .delete(&Owner::Instance, Kind::Session, &session.session_id)
                    .await;
            }
        }

        let updated = Record {
            status: Some(new_status.as_str().to_owned()),
            meta,
            version: record.version + 1,
            updated_at: SystemClock.now_millis(),
            ..record
        };
        self.store
            .put(&updated, Expect::Version(record.version))
            .await?;
        Ok(())
    }

    /// Deletes a user: every record the user owns (the vault with its
    /// keys, the credential, sessions metadata) plus the instance-side
    /// user record. The last owner cannot be deleted.
    pub async fn delete_user(&self, admin: &AuthUser, target: &str) -> Result<()> {
        if !admin.has_permission(Permission::UsersManage) {
            return Err(Error::Forbidden("this needs users:manage".to_owned()));
        }
        let user_id = UserId::parse(target)?;
        let record = self
            .store
            .get(&Owner::Instance, Kind::User, user_id.as_str())
            .await?
            .ok_or_else(|| Error::NotFound("no such user".to_owned()))?;
        let is_owner = record
            .meta
            .get("roles")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().any(|r| r.as_str() == Some("owner")))
            .unwrap_or(false);
        if is_owner {
            let owners = self.active_owners().await?;
            if owners <= 1 {
                return Err(Error::Conflict(
                    "the last owner cannot be removed".to_owned(),
                ));
            }
        }

        // The user-owned records go first: the vault with its keys,
        // the credential, everything (identity spec, section 1).
        self.store
            .delete_owner(&Owner::User(user_id.clone()))
            .await?;
        self.store
            .delete(&Owner::Instance, Kind::User, user_id.as_str())
            .await?;
        // Sessions are instance-owned records with meta.user; end them
        // so the token list does not show a ghost.
        let sessions = self.list_sessions(&user_id).await?;
        for session in sessions {
            let _ = self
                .store
                .delete(&Owner::Instance, Kind::Session, &session.session_id)
                .await;
        }
        Ok(())
    }

    /// How many active owners the instance has.
    pub async fn active_owners(&self) -> Result<usize> {
        let mut owners = 0;
        let mut cursor = None;
        loop {
            let page = self
                .store
                .list(
                    &Owner::Instance,
                    Kind::User,
                    &Default::default(),
                    &riehub_store::record::Page {
                        after: cursor.clone(),
                        limit: 200,
                    },
                )
                .await?;
            for record in &page.items {
                let active = record.status.as_deref() == Some("active");
                let has_owner = record
                    .meta
                    .get("roles")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().any(|r| r.as_str() == Some("owner")))
                    .unwrap_or(false);
                if active && has_owner {
                    owners += 1;
                }
            }
            match page.next {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        Ok(owners)
    }

    /// The user record for one id, if it exists.
    pub async fn user_by_id(&self, user_id: &UserId) -> Result<Option<User>> {
        let record = self
            .store
            .get(&Owner::Instance, Kind::User, user_id.as_str())
            .await?;
        match record {
            None => Ok(None),
            Some(record) => Ok(Some(
                crate::user_from_record(&record).map_err(|_| Error::Internal(None))?,
            )),
        }
    }

    /// Whether the patch body carries only admin-visible fields. Used
    /// by the tests to pin that admin routes never take content.
    pub fn patch_fields_are_admin_safe(patch: &Value) -> bool {
        patch
            .as_object()
            .map(|obj| obj.keys().all(|k| k == "roles" || k == "status"))
            .unwrap_or(false)
    }
}
