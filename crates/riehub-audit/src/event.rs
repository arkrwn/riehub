//! The typed audit event (security spec, section 6, ADR 0010). No
//! variant has a `String` or bytes field filled from a request: the
//! fields are ids, enums, numbers and booleans only.

use riehub_core::config::Profile;
use riehub_core::{ConnectionId, HubKeyId, RequestId, UserId};

/// A mac value. Serialised as an array of numbers, never a string.
pub type Mac = [u8; 48];

/// Who acted. Serialises as one of the fixed strings below or a typed
/// id, never caller text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Actor {
    User(UserId),
    Key(HubKeyId),
    System,
    Anonymous,
}

impl Actor {
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            Actor::User(id) => serde_json::Value::String(format!("user:{id}")),
            Actor::Key(id) => serde_json::Value::String(format!("key:{id}")),
            Actor::System => serde_json::Value::String("system".to_owned()),
            Actor::Anonymous => serde_json::Value::String("anonymous".to_owned()),
        }
    }
}

/// What the actor did. The variant names are the fixed action strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    SystemStart,
    ConfigLoad,
    AuditPrune,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditAction::SystemStart => "system.start",
            AuditAction::ConfigLoad => "config.load",
            AuditAction::AuditPrune => "audit.prune",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "system.start" => Some(AuditAction::SystemStart),
            "config.load" => Some(AuditAction::ConfigLoad),
            "audit.prune" => Some(AuditAction::AuditPrune),
            _ => None,
        }
    }
}

/// How the action ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    Denied,
    Error,
}

/// The record an action points at, when there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    User(UserId),
    Key(HubKeyId),
    Connection(ConnectionId),
}

/// The client ip as raw octets, after the configured truncation.
/// Octets are numbers in the JSON, never a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpField {
    V4([u8; 4]),
    V6([u8; 16]),
}

impl IpField {
    /// The default truncation: /24 for IPv4, /48 for IPv6.
    pub fn truncated(self) -> Self {
        match self {
            IpField::V4(mut o) => {
                o[3] = 0;
                IpField::V4(o)
            }
            IpField::V6(mut o) => {
                o[6..].fill(0);
                IpField::V6(o)
            }
        }
    }
}

/// The typed payload of one audit event. One variant per action.
#[derive(Debug, Clone, PartialEq)]
pub enum AuditEvent {
    SystemStart {},
    ConfigLoad { profile: Profile },
    AuditPrune { through_seq: u64, last_mac: Mac },
}

impl AuditEvent {
    /// The canonical bytes: a hand-written, length-prefixed encoding
    /// with the field order fixed here, not by JSON key order. Bump
    /// the format tag when the layout changes.
    pub fn canonical_bytes(&self, action: AuditAction) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(0x01); // format tag
        out.push(action_tag(action));
        match self {
            AuditEvent::SystemStart {} => {}
            AuditEvent::ConfigLoad { profile } => {
                let tag: u8 = match profile {
                    Profile::Dev => 1,
                    Profile::Production => 2,
                };
                out.push(tag);
            }
            AuditEvent::AuditPrune {
                through_seq,
                last_mac,
            } => {
                out.extend_from_slice(&through_seq.to_le_bytes());
                out.extend_from_slice(last_mac);
            }
        }
        out
    }

    /// The JSON body stored with the row. Null fields are omitted.
    pub fn to_json(&self, action: AuditAction) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "type".to_owned(),
            serde_json::Value::String(action.as_str().to_owned()),
        );
        match self {
            AuditEvent::SystemStart {} => {}
            AuditEvent::ConfigLoad { profile } => {
                map.insert(
                    "profile".to_owned(),
                    serde_json::Value::String(profile.as_str().to_owned()),
                );
            }
            AuditEvent::AuditPrune {
                through_seq,
                last_mac,
            } => {
                map.insert("through_seq".to_owned(), serde_json::json!(through_seq));
                let mac_array = serde_json::Value::Array(
                    last_mac.iter().map(|b| serde_json::json!(b)).collect(),
                );
                map.insert("last_mac".to_owned(), mac_array);
            }
        }
        serde_json::Value::Object(map)
    }
}

fn action_tag(action: AuditAction) -> u8 {
    match action {
        AuditAction::SystemStart => 1,
        AuditAction::ConfigLoad => 2,
        AuditAction::AuditPrune => 3,
    }
}

/// One event ready for the chain: the envelope plus the typed payload.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub ts: i64,
    pub actor: Actor,
    pub action: AuditAction,
    pub outcome: Outcome,
    pub request_id: Option<RequestId>,
    pub target: Option<Target>,
    pub ip: Option<IpField>,
    pub event: AuditEvent,
}

impl Event {
    /// The JSON body stored with the row: envelope plus payload. Field
    /// presence is fixed by code, not by input.
    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert("actor".to_owned(), self.actor.to_json_value());
        map.insert(
            "outcome".to_owned(),
            serde_json::Value::String(
                match self.outcome {
                    Outcome::Ok => "ok",
                    Outcome::Denied => "denied",
                    Outcome::Error => "error",
                }
                .to_owned(),
            ),
        );
        if let Some(request_id) = &self.request_id {
            map.insert(
                "request_id".to_owned(),
                serde_json::Value::String(request_id.to_string()),
            );
        }
        if let Some(target) = &self.target {
            let value = match target {
                Target::User(id) => serde_json::Value::String(format!("user:{id}")),
                Target::Key(id) => serde_json::Value::String(format!("key:{id}")),
                Target::Connection(id) => serde_json::Value::String(format!("conn:{id}")),
            };
            map.insert("target".to_owned(), value);
        }
        if let Some(ip) = &self.ip {
            let value = match ip {
                IpField::V4(o) => serde_json::json!({"v4": o}),
                IpField::V6(o) => serde_json::json!({"v6": o}),
            };
            map.insert("ip".to_owned(), value);
        }
        let payload = self.event.to_json(self.action);
        // to_json always returns an object by construction.
        if let serde_json::Value::Object(fields) = payload {
            for (key, value) in fields {
                map.insert(key, value);
            }
        }
        serde_json::Value::Object(map)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.ts.to_le_bytes());
        // length-prefixed string fields from the envelope
        put_str(
            &mut out,
            &match &self.actor {
                Actor::User(id) => format!("user:{id}"),
                Actor::Key(id) => format!("key:{id}"),
                Actor::System => "system".to_owned(),
                Actor::Anonymous => "anonymous".to_owned(),
            },
        );
        put_str(&mut out, self.action.as_str());
        out.push(match self.outcome {
            Outcome::Ok => 1,
            Outcome::Denied => 2,
            Outcome::Error => 3,
        });
        match &self.request_id {
            Some(id) => put_str(&mut out, id.as_str()),
            None => put_len(&mut out, 0),
        }
        match &self.target {
            Some(target) => put_str(
                &mut out,
                &match target {
                    Target::User(id) => format!("user:{id}"),
                    Target::Key(id) => format!("key:{id}"),
                    Target::Connection(id) => format!("conn:{id}"),
                },
            ),
            None => put_len(&mut out, 0),
        }
        match &self.ip {
            Some(IpField::V4(o)) => {
                put_len(&mut out, 4);
                out.extend_from_slice(o);
            }
            Some(IpField::V6(o)) => {
                put_len(&mut out, 16);
                out.extend_from_slice(o);
            }
            None => put_len(&mut out, 0),
        }
        out.extend_from_slice(&self.event.canonical_bytes(self.action));
        out
    }
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_len(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

fn put_len(out: &mut Vec<u8>, len: u64) {
    out.extend_from_slice(&len.to_le_bytes());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// The known enum strings and typed-id forms the JSON may carry.
    /// Anything else is a leak of caller text (security spec, section 6).
    #[test]
    fn event_enum_has_no_string_fields() {
        let id = riehub_core::UserId::parse("user_01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
        let key_id = riehub_core::HubKeyId::parse("hkey_01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap();
        let request = riehub_core::RequestId::parse("req_01ARZ3NDEKTSV4RRFFQ69G5FAX").unwrap();
        let samples = vec![
            Event {
                ts: 1,
                actor: Actor::System,
                action: AuditAction::SystemStart,
                outcome: Outcome::Ok,
                request_id: None,
                target: None,
                ip: None,
                event: AuditEvent::SystemStart {},
            },
            Event {
                ts: 2,
                actor: Actor::User(id.clone()),
                action: AuditAction::ConfigLoad,
                outcome: Outcome::Ok,
                request_id: Some(request),
                target: Some(Target::Key(key_id.clone())),
                ip: Some(IpField::V4([203, 0, 113, 9]).truncated()),
                event: AuditEvent::ConfigLoad {
                    profile: Profile::Production,
                },
            },
            Event {
                ts: 3,
                actor: Actor::System,
                action: AuditAction::AuditPrune,
                outcome: Outcome::Ok,
                request_id: None,
                target: None,
                ip: Some(IpField::V6([0x20; 16])),
                event: AuditEvent::AuditPrune {
                    through_seq: 41,
                    last_mac: [7u8; 48],
                },
            },
        ];

        let known = [
            "system",
            "anonymous",
            "ok",
            "denied",
            "error",
            "system.start",
            "config.load",
            "audit.prune",
            "dev",
            "production",
        ];
        for event in &samples {
            let json = event.to_json();
            check_strings(&json, &known);
        }
    }

    fn check_strings(value: &serde_json::Value, known: &[&str]) {
        match value {
            serde_json::Value::String(s) => {
                let parses_as_id = s
                    .split_once(':')
                    .map(|(prefix, rest)| match prefix {
                        "user" => riehub_core::UserId::parse(rest).is_ok(),
                        "key" => riehub_core::HubKeyId::parse(rest).is_ok(),
                        "conn" => riehub_core::ConnectionId::parse(rest).is_ok(),
                        _ => false,
                    })
                    .unwrap_or(false)
                    || riehub_core::RequestId::parse(s).is_ok();
                assert!(
                    parses_as_id || known.contains(&s.as_str()),
                    "unexpected free string in the audit event: {s}"
                );
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    check_strings(item, known);
                }
            }
            serde_json::Value::Object(map) => {
                for item in map.values() {
                    check_strings(item, known);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn canonical_bytes_are_stable() {
        let event = Event {
            ts: 42,
            actor: Actor::System,
            action: AuditAction::ConfigLoad,
            outcome: Outcome::Ok,
            request_id: None,
            target: None,
            ip: None,
            event: AuditEvent::ConfigLoad {
                profile: Profile::Dev,
            },
        };
        let first = event.canonical_bytes();
        let second = event.canonical_bytes();
        assert_eq!(first, second, "the encoding must be deterministic");
        assert_eq!(&first[..8], &42i64.to_le_bytes(), "the ts comes first");
        let payload = AuditEvent::ConfigLoad {
            profile: Profile::Dev,
        }
        .canonical_bytes(AuditAction::ConfigLoad);
        assert!(
            first.ends_with(&payload),
            "the typed payload closes the encoding with its format tag"
        );
    }

    #[test]
    fn ip_truncation_zeroes_the_host_bits() {
        let v4 = IpField::V4([203, 0, 113, 9]).truncated();
        assert_eq!(v4, IpField::V4([203, 0, 113, 0]));
        let v6 = IpField::V6([0x20; 16]).truncated();
        assert_eq!(
            v6,
            IpField::V6([0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
        );
    }
}
