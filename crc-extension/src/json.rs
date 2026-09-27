//! The two JSON shapes crossing the boundary, by hand: an extension should
//! not need serde to say "replace this text".

use crate::{Input, Output};

/// Reads the input object. Unknown keys are skipped, so crc can add fields
/// without breaking extensions built against this version.
pub fn read_input(bytes: &[u8]) -> Option<Input> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut p = Parser {
        s: text.as_bytes(),
        at: 0,
    };
    let mut input = Input::default();
    p.ws();
    p.eat(b'{')?;
    p.ws();
    if p.peek() == Some(b'}') {
        return Some(input);
    }
    loop {
        p.ws();
        let key = p.string()?;
        p.ws();
        p.eat(b':')?;
        p.ws();
        match key.as_str() {
            "text" => input.text = p.string()?,
            "language" => input.language = p.string()?,
            "selection" => input.selection = p.boolean()?,
            _ => p.skip()?,
        }
        p.ws();
        match p.next()? {
            b',' => continue,
            b'}' => return Some(input),
            _ => return None,
        }
    }
}

pub fn write_output(output: &Output) -> String {
    let mut out = String::from("{");
    let mut first = true;
    for (key, value) in [
        ("replace", &output.replace),
        ("message", &output.message),
        ("html", &output.html),
    ] {
        if let Some(value) = value {
            if !first {
                out.push(',');
            }
            first = false;
            out.push('"');
            out.push_str(key);
            out.push_str("\":");
            escape(value, &mut out);
        }
    }
    out.push('}');
    out
}

fn escape(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }
    fn next(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.at += 1;
        Some(b)
    }
    fn eat(&mut self, b: u8) -> Option<()> {
        (self.next()? == b).then_some(())
    }
    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.at += 1;
        }
    }
    fn boolean(&mut self) -> Option<bool> {
        if self.s[self.at..].starts_with(b"true") {
            self.at += 4;
            Some(true)
        } else if self.s[self.at..].starts_with(b"false") {
            self.at += 5;
            Some(false)
        } else {
            None
        }
    }
    fn hex4(&mut self) -> Option<u32> {
        let digits = self.s.get(self.at..self.at + 4)?;
        self.at += 4;
        u32::from_str_radix(std::str::from_utf8(digits).ok()?, 16).ok()
    }
    fn string(&mut self) -> Option<String> {
        self.eat(b'"')?;
        let mut out = Vec::new();
        loop {
            match self.next()? {
                b'"' => return String::from_utf8(out).ok(),
                b'\\' => {
                    let c = match self.next()? {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hi = self.hex4()?;
                            let code = if (0xd800..0xdc00).contains(&hi) {
                                self.eat(b'\\')?;
                                self.eat(b'u')?;
                                let lo = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&lo) {
                                    return None;
                                }
                                0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00)
                            } else {
                                hi
                            };
                            char::from_u32(code)?
                        }
                        _ => return None,
                    };
                    let mut buf = [0; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
                b => out.push(b),
            }
        }
    }
    /// Skips any value.
    fn skip(&mut self) -> Option<()> {
        match self.peek()? {
            b'"' => self.string().map(|_| ()),
            b'{' | b'[' => {
                let mut depth = 0usize;
                loop {
                    match self.peek()? {
                        b'"' => {
                            self.string()?;
                            continue;
                        }
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth -= 1;
                            if depth == 0 {
                                self.at += 1;
                                return Some(());
                            }
                        }
                        _ => {}
                    }
                    self.at += 1;
                }
            }
            _ => {
                while !matches!(self.peek(), Some(b',' | b'}' | b']') | None) {
                    self.at += 1;
                }
                Some(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_input_and_skips_what_it_does_not_know() {
        let input = read_input(
            r#"{"api":1,"text":"b\na é 😀 \ud83d\ude00","extra":{"x":[1,"}"]},"selection":true,"language":"rust"}"#
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(input.text, "b\na \u{e9} \u{1f600} \u{1f600}");
        assert!(input.selection);
        assert_eq!(input.language, "rust");
        assert_eq!(read_input(b"{}"), Some(Input::default()));
        assert_eq!(read_input(b"not json"), None);
    }

    #[test]
    fn writes_output_escaped() {
        let out = Output::replace("a\"b\\\n\u{1}").with_message("done");
        assert_eq!(
            write_output(&out),
            r#"{"replace":"a\"b\\\n\u0001","message":"done"}"#
        );
        assert_eq!(write_output(&Output::nothing()), "{}");
        assert_eq!(
            write_output(&Output::html("<p>\"hi\"</p>")),
            r#"{"html":"<p>\"hi\"</p>"}"#
        );
    }
}
