//! Random bytes from the operating system generator (security spec,
//! section 1).

// A failure of the OS generator is not recoverable in-process.
#[allow(clippy::panic)]
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    if aws_lc_rs::rand::fill(&mut buf).is_err() {
        panic!("the operating system random generator failed");
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_bytes_differ_between_calls() {
        let a = random_bytes::<32>();
        let b = random_bytes::<32>();
        assert_ne!(a, b);
    }
}
