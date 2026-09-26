//! Hash: MD5, SHA-1, SHA-256, SHA-512 and CRC32 of the selection, and decode
//! a JWT. Every digest is of the exact UTF-8 bytes of the selection, nothing
//! trimmed, so what you select is what gets hashed.

mod base64;
mod block;
mod crc32;
mod json;
mod md5;
mod sha1;
mod sha2;

use crc_extension::{Input, Output};

/// MD5.
pub fn md5(input: Input) -> Output {
    digest(
        &input,
        "MD5",
        &block::hex(&md5::digest(input.text.as_bytes())),
    )
}

/// SHA-1.
pub fn sha1(input: Input) -> Output {
    digest(
        &input,
        "SHA-1",
        &block::hex(&sha1::digest(input.text.as_bytes())),
    )
}

/// SHA-256.
pub fn sha256(input: Input) -> Output {
    digest(
        &input,
        "SHA-256",
        &block::hex(&sha2::sha256(input.text.as_bytes())),
    )
}

/// SHA-512.
pub fn sha512(input: Input) -> Output {
    digest(
        &input,
        "SHA-512",
        &block::hex(&sha2::sha512(input.text.as_bytes())),
    )
}

/// CRC32, as eight hex digits.
pub fn crc32(input: Input) -> Output {
    let value = crc32::crc32(input.text.as_bytes());
    digest(&input, "CRC32", &format!("{value:08x}"))
}

/// Replaces the selection with `hex` and says what was hashed, since the
/// text itself is gone (until undo).
fn digest(input: &Input, name: &str, hex: &str) -> Output {
    let bytes = input.text.len();
    let message = format!(
        "{name} of {} byte{}",
        thousands(bytes),
        if bytes == 1 { "" } else { "s" }
    );
    Output::replace(hex).with_message(message)
}

/// 1234567 as 1,234,567.
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Decode JWT: the header and payload as readable JSON. The signature is
/// not checked; there is no key here to check it with.
pub fn jwt_decode(input: Input) -> Output {
    match decode_jwt(input.text.trim()) {
        Ok((text, message)) => Output::replace(text).with_message(message),
        Err(why) => Output::message(format!("Hash: not a JWT: {why}")),
    }
}

fn decode_jwt(token: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 2 && parts.len() != 3 {
        return Err(format!(
            "expected header.payload.signature, found {} part{}",
            parts.len(),
            if parts.len() == 1 { "" } else { "s" }
        ));
    }
    let header = segment(parts.first().copied().unwrap_or(""), "header")?;
    let payload = segment(parts.get(1).copied().unwrap_or(""), "payload")?;
    let mut message = String::from("JWT decoded, signature not verified");
    if let Some(json::Value::Number(exp)) = payload.get("exp")
        && let Ok(seconds) = exp.parse::<f64>()
        && seconds.is_finite()
        && seconds.abs() < 1e15
        && let Some(date) = iso_date(seconds as i64)
    {
        message.push_str(", expires ");
        message.push_str(&date);
    }
    let text = format!(
        "// header\n{}\n\n// payload\n{}\n",
        json::pretty(&header),
        json::pretty(&payload)
    );
    Ok((text, message))
}

fn segment(text: &str, what: &str) -> Result<json::Value, String> {
    let bytes = base64::decode_url(text).map_err(|why| format!("{what}: {why}"))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{what} is not UTF-8"))?;
    let value = json::parse(&text).map_err(|why| format!("{what}: {why}"))?;
    match value {
        json::Value::Object(_) => Ok(value),
        _ => Err(format!("{what} is not a JSON object")),
    }
}

/// Seconds since the Unix epoch as `2024-09-26T14:40:00Z`. None when the
/// arithmetic would leave the range of dates anyone means.
fn iso_date(seconds: i64) -> Option<String> {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let (hour, minute, second) = (rest / 3600, (rest % 3600) / 60, rest % 60);
    // Days to a civil date, after Howard Hinnant's algorithm.
    let z = days.checked_add(719_468)?;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe.checked_add(era.checked_mul(400)?)?;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 {
        year.checked_add(1)?
    } else {
        year
    };
    if !(0..=9999).contains(&year) {
        return None;
    }
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

crc_extension::commands! {
    "md5" => md5,
    "sha1" => sha1,
    "sha256" => sha256,
    "sha512" => sha512,
    "crc32" => crc32,
    "jwt_decode" => jwt_decode,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(f: fn(Input) -> Output, text: &str) -> Output {
        f(Input {
            text: text.into(),
            selection: true,
            ..Input::default()
        })
    }

    fn hex_of(f: fn(Input) -> Output, text: &str) -> String {
        run(f, text).replace.unwrap_or_default()
    }

    const FOX: &str = "The quick brown fox jumps over the lazy dog";

    #[test]
    fn md5_vectors() {
        assert_eq!(hex_of(md5, ""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hex_of(md5, "abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(hex_of(md5, FOX), "9e107d9d372bb6826bd81d3542a419d6");
    }

    #[test]
    fn sha1_vectors() {
        assert_eq!(hex_of(sha1, ""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex_of(sha1, "abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hex_of(sha1, FOX),
            "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"
        );
    }

    #[test]
    fn sha256_vectors() {
        assert_eq!(
            hex_of(sha256, ""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex_of(sha256, "abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex_of(sha256, FOX),
            "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592"
        );
    }

    #[test]
    fn sha512_vectors() {
        assert_eq!(
            hex_of(sha512, ""),
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
        );
        assert_eq!(
            hex_of(sha512, "abc"),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
        assert_eq!(
            hex_of(sha512, FOX),
            "07e547d9586f6a73f73fbac0435ed76951218fb7d0c8d788a309d785436bbb642e93a252a954f23912547d1e8a3b5ed6e1bfd7097821233fa0538f3db854fee6"
        );
    }

    #[test]
    fn crc32_vectors() {
        assert_eq!(hex_of(crc32, ""), "00000000");
        assert_eq!(hex_of(crc32, "abc"), "352441c2");
        assert_eq!(hex_of(crc32, "123456789"), "cbf43926");
        assert_eq!(hex_of(crc32, FOX), "414fa339");
    }

    /// Lengths around the block and padding boundaries: 55 bytes still fit
    /// their padding in one block, 56 and up need two, 64 is a full block.
    #[test]
    fn padding_boundaries() {
        let a = |n: usize| "a".repeat(n);
        assert_eq!(
            hex_of(sha256, &a(55)),
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"
        );
        assert_eq!(
            hex_of(sha256, &a(56)),
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"
        );
        assert_eq!(
            hex_of(sha256, &a(64)),
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"
        );
        assert_eq!(hex_of(md5, &a(64)), "014842d480b571495a4a0363793f7367");
        assert_eq!(
            hex_of(sha512, &a(112)),
            "c01d080efd492776a1c43bd23dd99d0a2e626d481e16782e75d54c2503b5dc32bd05f0f1ba33e568b88fd2d970929b719ecbb152f58f130a407c8830604b70ca"
        );
    }

    /// A million 'a', the classic long vector. Slow in debug, so it runs
    /// with `cargo test -p hash --release -- --ignored`.
    #[test]
    #[ignore]
    fn million_a() {
        let text = "a".repeat(1_000_000);
        assert_eq!(hex_of(md5, &text), "7707d6ae4e027c70eea2a935c2296f21");
        assert_eq!(
            hex_of(sha1, &text),
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
        );
        assert_eq!(
            hex_of(sha256, &text),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        assert_eq!(
            hex_of(sha512, &text),
            "e718483d0ce769644e2e42c7bc15b4638e1f98b13b2044285632a803afa973ebde0ff244877ea60a4cb0432ce577c31beb009c5c2c49aa2e4eadb217ad8cc09b"
        );
        assert_eq!(hex_of(crc32, &text), "dc25bfbc");
    }

    #[test]
    fn hashes_the_bytes_as_given_and_says_so() {
        let out = run(sha256, " hi\n");
        assert_ne!(out.replace, run(sha256, "hi").replace);
        assert_eq!(out.message.as_deref(), Some("SHA-256 of 4 bytes"));
        assert_eq!(run(md5, "a").message.as_deref(), Some("MD5 of 1 byte"));
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
    }

    #[test]
    fn decodes_a_jwt() {
        // {"alg":"HS256","typ":"JWT"} . {"sub":"1234567890","name":"John Doe","iat":1516239022}
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let out = run(jwt_decode, &format!("  {token}\n"));
        assert_eq!(
            out.replace.as_deref(),
            Some(
                "// header\n{\n  \"alg\": \"HS256\",\n  \"typ\": \"JWT\"\n}\n\n// payload\n{\n  \"sub\": \"1234567890\",\n  \"name\": \"John Doe\",\n  \"iat\": 1516239022\n}\n"
            )
        );
        assert_eq!(
            out.message.as_deref(),
            Some("JWT decoded, signature not verified")
        );
    }

    #[test]
    fn says_when_a_jwt_expires() {
        // {"alg":"none"} . {"exp":1727361600}
        let token = "eyJhbGciOiJub25lIn0.eyJleHAiOjE3MjczNjE2MDB9.";
        let out = run(jwt_decode, token);
        assert!(out.replace.is_some());
        assert_eq!(
            out.message.as_deref(),
            Some("JWT decoded, signature not verified, expires 2024-09-26T14:40:00Z")
        );
    }

    #[test]
    fn refuses_what_is_not_a_jwt() {
        let out = run(jwt_decode, "hello world");
        assert_eq!(out.replace, None);
        assert_eq!(
            out.message.as_deref(),
            Some("Hash: not a JWT: expected header.payload.signature, found 1 part")
        );
        let out = run(jwt_decode, "a.b.c");
        assert!(
            out.message
                .unwrap_or_default()
                .starts_with("Hash: not a JWT: header")
        );
        // A payload that is an array, not an object.
        let out = run(jwt_decode, "eyJhbGciOiJub25lIn0.W10.");
        assert_eq!(
            out.message.as_deref(),
            Some("Hash: not a JWT: payload is not a JSON object")
        );
        // Bad Base64 and bad JSON both come back as messages, never a panic.
        assert!(run(jwt_decode, "!!.!!").replace.is_none());
        assert!(run(jwt_decode, "eyJ.eyJ").replace.is_none());
        assert!(run(jwt_decode, "").replace.is_none());
    }

    #[test]
    fn dates() {
        assert_eq!(iso_date(0).as_deref(), Some("1970-01-01T00:00:00Z"));
        assert_eq!(iso_date(-1).as_deref(), Some("1969-12-31T23:59:59Z"));
        assert_eq!(
            iso_date(951_782_400).as_deref(),
            Some("2000-02-29T00:00:00Z")
        );
        assert_eq!(
            iso_date(1_727_361_600).as_deref(),
            Some("2024-09-26T14:40:00Z")
        );
        assert_eq!(iso_date(i64::MAX), None);
        assert_eq!(iso_date(i64::MIN), None);
    }

    #[test]
    fn base64_url_and_standard_alphabets() {
        assert_eq!(base64::decode_url("aGk").ok(), Some(b"hi".to_vec()));
        assert_eq!(base64::decode_url("aGk=").ok(), Some(b"hi".to_vec()));
        assert_eq!(base64::decode_url("-_8").ok(), Some(vec![0xfb, 0xff]));
        assert_eq!(base64::decode_url("+/8=").ok(), Some(vec![0xfb, 0xff]));
        assert!(base64::decode_url("a b").is_err());
        assert!(base64::decode_url("a").is_err());
    }
}
