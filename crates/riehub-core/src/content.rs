//! `Content`: user content bytes that never leak through traits (S2,
//! security spec section 2).

use serde::de::{Deserialize, Deserializer, Visitor};
use std::fmt;
use zeroize::Zeroize;

pub struct Content(Vec<u8>);

impl Content {
    /// The bytes. The name shows up in reviews and audits.
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for Content {
    fn from(value: String) -> Self {
        Self(value.into_bytes())
    }
}

impl From<Vec<u8>> for Content {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl Drop for Content {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for Content {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[content]")
    }
}

impl fmt::Display for Content {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[content]")
    }
}

impl<'de> Deserialize<'de> for Content {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct ContentVisitor;

        impl Visitor<'_> for ContentVisitor {
            type Value = Content;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a string of user content")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Content, E> {
                Ok(Content::from(v.to_owned()))
            }
        }

        d.deserialize_string(ContentVisitor)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn content_debug_is_redacted() {
        let content = Content::from(String::from("hunter2"));
        let debug = format!("{:?}", content);
        let display = format!("{}", content);
        assert!(
            !debug.contains("hunter2"),
            "Debug leaked the content: {debug}"
        );
        assert!(
            !display.contains("hunter2"),
            "Display leaked the content: {display}"
        );
        assert_eq!(debug, "[content]");
        assert_eq!(display, "[content]");
        assert_eq!(content.expose(), b"hunter2");
        assert_eq!(content.len(), 7);
    }

    #[test]
    fn content_deserializes_from_a_json_string() {
        let content: Content = serde_json::from_str("\"hello\"").unwrap();
        assert_eq!(content.expose(), b"hello");
    }
}
