//! Base64 decoding for JWTs: the URL-safe alphabet (RFC 4648 section 5),
//! padding optional. The standard alphabet is accepted too, since tokens get
//! pasted from all sorts of places.

/// Decodes `text`, or says what is wrong with it.
pub fn decode_url(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for (i, c) in text.chars().enumerate() {
        let value = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '-' | '+' => 62,
            '_' | '/' => 63,
            '=' => break,
            '\r' | '\n' => continue,
            _ => {
                return Err(format!(
                    "character {c:?} at position {} is not Base64",
                    i + 1
                ));
            }
        };
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            // The top 8 of `bits + 8` bits, the rest stays in the buffer.
            out.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    if bits >= 6 {
        return Err("a Base64 segment ends with a stray character".into());
    }
    Ok(out)
}
