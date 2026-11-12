//! `Secret<T>`: a credential that never leaks through traits (S2,
//! security spec section 2).

use std::fmt;
use zeroize::Zeroize;

pub struct Secret<T: Zeroize>(T);

impl<T: Zeroize> Secret<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// The only way in. The name shows up in reviews and audits.
    pub fn expose(&self) -> &T {
        &self.0
    }

    pub fn expose_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: Zeroize> Drop for Secret<T> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl<T: Zeroize> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[secret]")
    }
}

impl<T: Zeroize> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[secret]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_debug_and_display_are_redacted() {
        let secret = Secret::new(String::from("hunter2"));
        let debug = format!("{:?}", secret);
        let display = format!("{}", secret);
        assert!(
            !debug.contains("hunter2"),
            "Debug leaked the value: {debug}"
        );
        assert!(
            !display.contains("hunter2"),
            "Display leaked the value: {display}"
        );
        assert_eq!(debug, "[secret]");
        assert_eq!(display, "[secret]");
        assert_eq!(secret.expose(), "hunter2");
    }
}
