//! Encode and Decode: Base64, URL (percent) and HTML encoding, both ways.
//! Input that does not decode is left alone, with a message saying why,
//! never replaced by something half-decoded.

use crc_extension::{Input, Output};

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 Encode, standard alphabet with padding.
pub fn base64_encode(input: Input) -> Output {
    let bytes = input.text.as_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(BASE64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    Output::replace(out)
}

/// Base64 Decode. Whitespace is ignored, padding is optional, and the URL
/// alphabet (`-` and `_`) is accepted too.
pub fn base64_decode(input: Input) -> Output {
    let digits: Vec<u8> = input
        .text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let digits = digits
        .strip_suffix(b"==")
        .or_else(|| digits.strip_suffix(b"="))
        .unwrap_or(&digits);
    let value = |b: u8| -> Option<u32> {
        Some(match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    if digits.len() % 4 == 1 {
        return Output::message("Not Base64: the length is wrong");
    }
    let mut out = Vec::with_capacity(digits.len() * 3 / 4);
    for chunk in digits.chunks(4) {
        let mut n = 0u32;
        for (i, &b) in chunk.iter().enumerate() {
            let Some(v) = value(b) else {
                return Output::message(format!(
                    "Not Base64: {:?} is not a Base64 digit",
                    b as char
                ));
            };
            n |= v << (18 - 6 * i);
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..chunk.len() - 1]);
    }
    match String::from_utf8(out) {
        Ok(text) => Output::replace(text),
        Err(_) => Output::message("Decoded, but it is binary, not text; nothing changed"),
    }
}

/// URL Encode, as a component: everything but letters, digits and `-_.~`
/// becomes `%XX`.
pub fn url_encode(input: Input) -> Output {
    let mut out = String::with_capacity(input.text.len());
    for b in input.text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    Output::replace(out)
}

/// URL Decode: `%XX` escapes, and `+` as a space, as forms send it.
pub fn url_decode(input: Input) -> Output {
    let bytes = input.text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                // Two hex digits exactly: `from_str_radix` also takes `+F`.
                let hex = bytes
                    .get(i + 1..i + 3)
                    .filter(|h| h.iter().all(u8::is_ascii_hexdigit))
                    .and_then(|h| std::str::from_utf8(h).ok())
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                let Some(b) = hex else {
                    return Output::message(format!(
                        "Not URL-encoded: a % at {i} is not followed by two hex digits"
                    ));
                };
                out.push(b);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    match String::from_utf8(out) {
        Ok(text) => Output::replace(text),
        Err(_) => Output::message("Decoded, but it is not UTF-8 text; nothing changed"),
    }
}

/// HTML Escape: `& < > " '`.
pub fn html_escape(input: Input) -> Output {
    let mut out = String::with_capacity(input.text.len());
    for c in input.text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    Output::replace(out)
}

/// HTML Unescape: the common named entities and every numeric one. An
/// unknown entity is left as it is.
pub fn html_unescape(input: Input) -> Output {
    let text = &input.text;
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        // Entities are short: look for the `;` in the next few bytes only.
        // Searching the whole rest for each `&` made a long text of `&&`
        // quadratic, and the command ran out of time.
        let window = &rest.as_bytes()[..rest.len().min(13)];
        let entity = window
            .iter()
            .position(|&b| b == b';')
            .map(|end| (&rest[1..end], end));
        let decoded = entity.and_then(|(name, _)| match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            _ => {
                let number = name.strip_prefix('#')?;
                // Digits only: `parse` and `from_str_radix` also take a `+`.
                let code = match number.strip_prefix(['x', 'X']) {
                    Some(hex) if hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
                        u32::from_str_radix(hex, 16).ok()?
                    }
                    Some(_) => return None,
                    None if number.bytes().all(|b| b.is_ascii_digit()) => number.parse().ok()?,
                    None => return None,
                };
                char::from_u32(code)
            }
        });
        match (decoded, entity) {
            (Some(c), Some((_, end))) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    Output::replace(out)
}

crc_extension::commands! {
    "base64_encode" => base64_encode,
    "base64_decode" => base64_decode,
    "url_encode" => url_encode,
    "url_decode" => url_decode,
    "html_escape" => html_escape,
    "html_unescape" => html_unescape,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(f: fn(Input) -> Output, text: &str) -> Output {
        f(Input {
            text: text.into(),
            ..Input::default()
        })
    }

    fn replaced(f: fn(Input) -> Output, text: &str) -> String {
        run(f, text).replace.unwrap()
    }

    #[test]
    fn base64_round_trips_and_matches_the_rfc_vectors() {
        for (plain, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(replaced(base64_encode, plain), encoded);
            assert_eq!(replaced(base64_decode, encoded), plain);
        }
        assert_eq!(replaced(base64_decode, "Zm9v\nYmFy"), "foobar");
        assert_eq!(replaced(base64_decode, "w6k"), "é", "no padding");
        let bad = run(base64_decode, "Zm9v!A");
        assert!(bad.replace.is_none() && bad.message.unwrap().contains("'!'"));
        assert!(
            run(base64_decode, "/w==").replace.is_none(),
            "binary is not text"
        );
    }

    #[test]
    fn url_encoding_both_ways() {
        assert_eq!(replaced(url_encode, "a b&c=é/~"), "a%20b%26c%3D%C3%A9%2F~");
        assert_eq!(replaced(url_decode, "a%20b%26c%3D%C3%A9%2F~"), "a b&c=é/~");
        assert_eq!(replaced(url_decode, "a+b"), "a b");
        assert!(run(url_decode, "100%").replace.is_none());
    }

    #[test]
    fn html_both_ways() {
        assert_eq!(
            replaced(html_escape, "<a href=\"x\">R&D's</a>"),
            "&lt;a href=&quot;x&quot;&gt;R&amp;D&#39;s&lt;/a&gt;"
        );
        assert_eq!(
            replaced(
                html_unescape,
                "&lt;b&gt; &amp;amp; &#233;&#x1F600; &bogus; & x"
            ),
            "<b> &amp; é😀 &bogus; & x"
        );
    }

    #[test]
    fn signs_are_not_digits_and_long_text_is_fast() {
        assert!(run(url_decode, "%+F").replace.is_none());
        assert_eq!(replaced(html_unescape, "&#+65;"), "&#+65;");
        let long = "a && b\n".repeat(150_000);
        let started = std::time::Instant::now();
        assert_eq!(replaced(html_unescape, &long), long);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }
}
