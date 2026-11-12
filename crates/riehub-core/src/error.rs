//! The one error type every crate returns. Display never carries a
//! source error's text (S2, security spec section 2).

use std::fmt;

type BoxedSource = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug)]
pub enum Error {
    NotFound(String),
    Conflict(String),
    Unauthorized(String),
    Forbidden(String),
    Invalid(String),
    RateLimited(String),
    Unavailable(String),
    Internal(Option<BoxedSource>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound(m) => write!(f, "not found: {m}"),
            Error::Conflict(m) => write!(f, "conflict: {m}"),
            Error::Unauthorized(m) => write!(f, "unauthorized: {m}"),
            Error::Forbidden(m) => write!(f, "forbidden: {m}"),
            Error::Invalid(m) => write!(f, "invalid: {m}"),
            Error::RateLimited(m) => write!(f, "rate limited: {m}"),
            Error::Unavailable(m) => write!(f, "unavailable: {m}"),
            // The source text never reaches Display (S2).
            Error::Internal(_) => f.write_str("internal error"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Internal(Some(source)) => Some(source.as_ref()),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_error_display_hides_source() {
        let error = Error::Internal(Some(Box::new(std::io::Error::other("password=hunter2"))));
        let display = error.to_string();
        assert_eq!(display, "internal error");
        assert!(
            !display.contains("hunter2"),
            "Display leaked the source: {display}"
        );
    }

    #[test]
    fn error_classes_display_their_message() {
        assert_eq!(
            Error::NotFound("user_01".to_owned()).to_string(),
            "not found: user_01"
        );
        assert_eq!(
            Error::Invalid("bad id".to_owned()).to_string(),
            "invalid: bad id"
        );
        assert_eq!(Error::Internal(None).to_string(), "internal error");
    }
}
