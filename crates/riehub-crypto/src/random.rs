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

/// Base64url without padding, for token secrets.
pub fn base64url(bytes: &[u8]) -> String {
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).map(|b| *b as u32).unwrap_or(0);
        let b2 = chunk.get(2).map(|b| *b as u32).unwrap_or(0);
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHA[((n >> 18) & 63) as usize] as char);
        out.push(ALPHA[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHA[((n >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHA[(n & 63) as usize] as char);
        }
    }
    out
}

/// Decodes unpadded base64url. Returns None on any bad character.
pub fn base64url_decode(text: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut values = Vec::with_capacity(text.len());
    for c in text.bytes() {
        let v = ALPHABET.iter().position(|a| *a == c)?;
        values.push(v as u32);
    }
    let mut out = Vec::with_capacity(values.len() * 3 / 4);
    for chunk in values.chunks(4) {
        let n = match chunk.len() {
            2 => (chunk[0] << 18) | (chunk[1] << 12),
            3 => (chunk[0] << 18) | (chunk[1] << 12) | (chunk[2] << 6),
            4 => (chunk[0] << 18) | (chunk[1] << 12) | (chunk[2] << 6) | chunk[3],
            _ => return None,
        };
        out.push(((n >> 16) & 0xff) as u8);
        if chunk.len() > 2 {
            out.push(((n >> 8) & 0xff) as u8);
        }
        if chunk.len() > 3 {
            out.push((n & 0xff) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn base64url_round_trips() {
        let bytes = random_bytes::<32>();
        let text = base64url(&bytes);
        assert!(!text.contains('+'));
        assert!(!text.contains('/'));
        assert!(!text.contains('='));
        assert_eq!(base64url_decode(&text).unwrap(), bytes.to_vec());
    }

    #[test]
    fn random_bytes_differ_between_calls() {
        let a = random_bytes::<32>();
        let b = random_bytes::<32>();
        assert_ne!(a, b);
    }
}
