//! Roles and permissions (identity spec, section 5) and the
//! `UserScope`, whose constructor is private to this crate and takes
//! an `AuthUser` (S5: no route can build a scope for someone else).

use riehub_core::Owner;
use riehub_core::UserId;
use riehub_vault::Unlocked;
use std::sync::Arc;

/// The built-in roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    Admin,
    Member,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Admin => "admin",
            Role::Member => "member",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "owner" => Some(Role::Owner),
            "admin" => Some(Role::Admin),
            "member" => Some(Role::Member),
            _ => None,
        }
    }

    /// The permissions each role carries (identity spec, section 5).
    pub fn permissions(&self) -> &'static [Permission] {
        match self {
            Role::Owner => &[
                Permission::UsersManage,
                Permission::TeamsManage,
                Permission::CatalogManage,
                Permission::PolicyManage,
                Permission::UsageReadAll,
                Permission::AuditRead,
                Permission::UsageReadOwn,
                Permission::InstanceSettings,
            ],
            Role::Admin => &[
                Permission::UsersManage,
                Permission::TeamsManage,
                Permission::CatalogManage,
                Permission::PolicyManage,
                Permission::UsageReadAll,
                Permission::AuditRead,
                Permission::UsageReadOwn,
            ],
            Role::Member => &[Permission::UsageReadOwn],
        }
    }
}

/// The permission strings. No permission grants reading content or
/// secrets of another user (S3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    UsersManage,
    TeamsManage,
    CatalogManage,
    PolicyManage,
    UsageReadAll,
    AuditRead,
    UsageReadOwn,
    InstanceSettings,
}

impl Permission {
    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::UsersManage => "users:manage",
            Permission::TeamsManage => "teams:manage",
            Permission::CatalogManage => "catalog:manage",
            Permission::PolicyManage => "policy:manage",
            Permission::UsageReadAll => "usage:read:all",
            Permission::AuditRead => "audit:read",
            Permission::UsageReadOwn => "usage:read:own",
            Permission::InstanceSettings => "instance:settings",
        }
    }
}

/// The authenticated caller: session identity plus the unlocked vault
/// handle. The only way handlers get at user-owned data.
#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct AuthUser {
    pub user_id: UserId,
    pub roles: Vec<Role>,
    pub session_id: String,
    pub unlocked: Arc<Unlocked>,
}

impl AuthUser {
    /// The `UserScope` for this caller. The constructor of
    /// `UserScope` is private to this crate, so this is the only way.
    pub fn scope(&self) -> UserScope {
        UserScope {
            user_id: self.user_id.clone(),
        }
    }

    pub fn has_permission(&self, permission: Permission) -> bool {
        self.roles
            .iter()
            .any(|role| role.permissions().contains(&permission))
    }

    pub fn is_owner(&self) -> bool {
        self.roles.contains(&Role::Owner)
    }

    pub fn is_admin(&self) -> bool {
        self.roles.contains(&Role::Admin) || self.is_owner()
    }
}

/// Marker trait for response types of `AdminMeta` routes. It is
/// implemented by hand only for types without `Content` or `Secret`
/// fields, so admin responses cannot smuggle either through.
pub trait AdminSafe {}

/// The tenant of a user-owned record. Built only from an `AuthUser`.
#[derive(Debug, Clone)]
pub struct UserScope {
    user_id: UserId,
}

impl UserScope {
    pub fn owner(&self) -> Owner {
        Owner::User(self.user_id.clone())
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }
}
