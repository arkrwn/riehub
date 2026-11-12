//! Records (storage spec, section 2).

use riehub_core::error::{Error, Result};
use riehub_core::Owner;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    User,
    Vault,
    Session,
    HubKey,
    Connection,
    RoutingGroup,
    Connector,
    Skill,
    Chat,
    ChatMessage,
    Workflow,
    Run,
}

impl Kind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::User => "user",
            Kind::Vault => "vault",
            Kind::Session => "session",
            Kind::HubKey => "hub_key",
            Kind::Connection => "connection",
            Kind::RoutingGroup => "routing_group",
            Kind::Connector => "connector",
            Kind::Skill => "skill",
            Kind::Chat => "chat",
            Kind::ChatMessage => "chat_message",
            Kind::Workflow => "workflow",
            Kind::Run => "run",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "user" => Ok(Kind::User),
            "vault" => Ok(Kind::Vault),
            "session" => Ok(Kind::Session),
            "hub_key" => Ok(Kind::HubKey),
            "connection" => Ok(Kind::Connection),
            "routing_group" => Ok(Kind::RoutingGroup),
            "connector" => Ok(Kind::Connector),
            "skill" => Ok(Kind::Skill),
            "chat" => Ok(Kind::Chat),
            "chat_message" => Ok(Kind::ChatMessage),
            "workflow" => Ok(Kind::Workflow),
            "run" => Ok(Kind::Run),
            _ => Err(Error::Invalid(format!("unknown record kind: {s}"))),
        }
    }
}

/// What `put` expects about the stored version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    New,
    Version(u64),
    Any,
}

/// The only queryable fields (storage spec, section 3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    pub parent: Option<String>,
    pub status: Option<String>,
    pub sort_min: Option<i64>,
    pub sort_max: Option<i64>,
}

/// An opaque list position: the last returned (sort, id) pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub sort: i64,
    pub id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    pub after: Option<Cursor>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Listed {
    pub items: Vec<Record>,
    pub next: Option<Cursor>,
}

/// The one record shape (storage spec, section 2).
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub owner: Owner,
    pub kind: Kind,
    pub id: String,
    pub parent: Option<String>,
    pub lookup: Option<String>,
    pub status: Option<String>,
    pub sort: i64,
    /// A JSON object: ids, enums, numbers, booleans and timestamps only.
    pub meta: Value,
    /// AEAD ciphertext for user-owned kinds, plain JSON for instance
    /// configuration. Never free user text.
    pub data: Option<Vec<u8>>,
    pub version: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Record {
    pub fn new(owner: Owner, kind: Kind, id: impl Into<String>) -> Self {
        Self {
            owner,
            kind,
            id: id.into(),
            parent: None,
            lookup: None,
            status: None,
            sort: 0,
            meta: Value::Object(serde_json::Map::new()),
            data: None,
            version: 0,
            created_at: 0,
            updated_at: 0,
        }
    }
}
