//! The tenant of a record (storage spec, section 2).

use crate::ids::UserId;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Owner {
    Instance,
    User(UserId),
}

impl Owner {
    pub fn user_id(&self) -> Option<&UserId> {
        match self {
            Owner::Instance => None,
            Owner::User(id) => Some(id),
        }
    }
}

impl fmt::Display for Owner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Owner::Instance => f.write_str("instance"),
            Owner::User(id) => write!(f, "user:{id}"),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn owner_displays_the_storage_key_shape() {
        assert_eq!(Owner::Instance.to_string(), "instance");
        let id = UserId::new();
        assert_eq!(Owner::User(id.clone()).to_string(), format!("user:{}", id));
    }

    #[test]
    fn user_id_is_visible_only_for_user_owners() {
        assert!(Owner::Instance.user_id().is_none());
        let id = UserId::new();
        assert_eq!(Owner::User(id.clone()).user_id(), Some(&id));
    }
}
