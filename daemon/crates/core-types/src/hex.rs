//! Hex-Kodierung ohne zusätzliche Abhängigkeit.

const DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Kodiert Bytes als Kleinbuchstaben-Hex.
pub(crate) fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::encode;

    #[test]
    fn encodes_lowercase() {
        assert_eq!(encode(&[0x00, 0x0f, 0xff, 0xa5]), "000fffa5");
        assert_eq!(encode(&[]), "");
    }
}
