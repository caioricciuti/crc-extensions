//! JSON in and out: a parser that keeps key order and the original number
//! lexeme, and writers for the formatted and the minified shape.

use std::fmt;

/// A JSON value. Objects keep their keys in document order; numbers keep
/// the text they were written with, so `1.0` stays `1.0` and `1e3` stays
/// `1e3` through a round trip.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

/// Where and why parsing stopped.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub line: usize,
    pub column: usize,
    pub what: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "line {}, column {}: {}",
            self.line, self.column, self.what
        )
    }
}

/// Parses one JSON text. Anything after the value but whitespace is an
/// error.
pub fn parse(text: &str) -> Result<Value, Error> {
    let mut p = Parser {
        s: text.as_bytes(),
        at: 0,
        depth: 0,
    };
    p.ws();
    let value = p.value()?;
    p.ws();
    if p.at < p.s.len() {
        return Err(p.error("unexpected text after the value"));
    }
    Ok(value)
}

/// Nesting deeper than this is refused rather than overflowing the stack.
const MAX_DEPTH: usize = 512;

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
    depth: usize,
}

impl Parser<'_> {
    fn error(&self, what: impl Into<String>) -> Error {
        let (line, column) = position(self.s, self.at);
        Error {
            line,
            column,
            what: what.into(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn value(&mut self) -> Result<Value, Error> {
        match self.peek() {
            None => Err(self.error("expected a value, found the end")),
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b't') => self.word("true", Value::Bool(true)),
            Some(b'f') => self.word("false", Value::Bool(false)),
            Some(b'n') => self.word("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(c) => Err(self.error(format!("unexpected {}", describe(c)))),
        }
    }

    fn word(&mut self, word: &str, value: Value) -> Result<Value, Error> {
        let end = self.at + word.len();
        if self.s.get(self.at..end) == Some(word.as_bytes()) {
            self.at = end;
            Ok(value)
        } else {
            Err(self.error(format!("expected {word}")))
        }
    }

    fn enter(&mut self) -> Result<(), Error> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.error("nested too deep"));
        }
        Ok(())
    }

    fn object(&mut self) -> Result<Value, Error> {
        self.enter()?;
        self.at += 1;
        let mut members = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.at += 1;
            self.depth -= 1;
            return Ok(Value::Object(members));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(self.error(match self.peek() {
                    None => "expected a key, found the end".to_owned(),
                    Some(b'}') => "trailing comma before '}'".to_owned(),
                    Some(c) => format!("expected a quoted key, found {}", describe(c)),
                }));
            }
            let key = self.string()?;
            self.ws();
            if self.peek() != Some(b':') {
                return Err(self.error("expected ':' after the key"));
            }
            self.at += 1;
            self.ws();
            let value = self.value()?;
            members.push((key, value));
            self.ws();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    self.depth -= 1;
                    return Ok(Value::Object(members));
                }
                None => return Err(self.error("expected ',' or '}', found the end")),
                Some(c) => {
                    return Err(self.error(format!("expected ',' or '}}', found {}", describe(c))));
                }
            }
        }
    }

    fn array(&mut self) -> Result<Value, Error> {
        self.enter()?;
        self.at += 1;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.at += 1;
            self.depth -= 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.ws();
            if self.peek() == Some(b']') {
                return Err(self.error("trailing comma before ']'"));
            }
            items.push(self.value()?);
            self.ws();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    self.depth -= 1;
                    return Ok(Value::Array(items));
                }
                None => return Err(self.error("expected ',' or ']', found the end")),
                Some(c) => {
                    return Err(self.error(format!("expected ',' or ']', found {}", describe(c))));
                }
            }
        }
    }

    fn number(&mut self) -> Result<Value, Error> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        match self.peek() {
            Some(b'0') => self.at += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return Err(self.error("expected a digit")),
        }
        if self.peek() == Some(b'.') {
            self.at += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("expected a digit after '.'"));
            }
            self.digits();
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("expected a digit in the exponent"));
            }
            self.digits();
        }
        let lexeme = self.s.get(start..self.at).unwrap_or_default();
        // Only ASCII was consumed, so this cannot fail.
        Ok(Value::Number(String::from_utf8_lossy(lexeme).into_owned()))
    }

    fn digits(&mut self) {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
    }

    /// Reads a string starting at the opening quote.
    fn string(&mut self) -> Result<String, Error> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let c = match self.peek() {
                None => return Err(self.error("unterminated string")),
                Some(c) => c,
            };
            match c {
                b'"' => {
                    self.at += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.at += 1;
                    let e = self
                        .peek()
                        .ok_or_else(|| self.error("unterminated string"))?;
                    self.at += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let first = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&first) {
                                if self.s.get(self.at..self.at + 2) != Some(b"\\u") {
                                    return Err(self.error("lone high surrogate"));
                                }
                                self.at += 2;
                                let second = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&second) {
                                    return Err(self.error("bad low surrogate"));
                                }
                                0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                            } else {
                                first
                            };
                            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        }
                        other => {
                            self.at -= 1;
                            return Err(self.error(format!("bad escape '\\{}'", describe(other))));
                        }
                    }
                }
                0..=0x1F => return Err(self.error("control character in string")),
                _ => {
                    // Copy one whole UTF-8 sequence.
                    let len = utf8_len(c);
                    let bytes = self
                        .s
                        .get(self.at..self.at + len)
                        .ok_or_else(|| self.error("bad UTF-8"))?;
                    let piece = std::str::from_utf8(bytes).map_err(|_| self.error("bad UTF-8"))?;
                    out.push_str(piece);
                    self.at += len;
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, Error> {
        let bytes = self
            .s
            .get(self.at..self.at + 4)
            .ok_or_else(|| self.error("expected four hex digits"))?;
        let mut code = 0u32;
        for &b in bytes {
            let d = (b as char)
                .to_digit(16)
                .ok_or_else(|| self.error("expected four hex digits"))?;
            code = code * 16 + d;
        }
        self.at += 4;
        Ok(code)
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

fn describe(c: u8) -> String {
    if c.is_ascii_graphic() {
        format!("'{}'", c as char)
    } else if c == b' ' {
        "a space".to_owned()
    } else if c.is_ascii() {
        format!("byte 0x{c:02x}")
    } else {
        "a non-ASCII character".to_owned()
    }
}

/// The 1-based line and column (in characters) of a byte offset.
pub fn position(s: &[u8], at: usize) -> (usize, usize) {
    let at = at.min(s.len());
    let mut line = 1;
    let mut column = 1;
    for &b in &s[..at] {
        if b == b'\n' {
            line += 1;
            column = 1;
        } else if (b & 0xC0) != 0x80 {
            column += 1;
        }
    }
    (line, column)
}

/// Writes `value` with two-space indentation.
pub fn format(value: &Value) -> String {
    let mut out = String::new();
    write_pretty(value, 0, &mut out);
    out
}

fn write_pretty(value: &Value, depth: usize, out: &mut String) {
    match value {
        Value::Array(items) if !items.is_empty() => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(depth + 1, out);
                write_pretty(item, depth + 1, out);
            }
            newline(depth, out);
            out.push(']');
        }
        Value::Object(members) if !members.is_empty() => {
            out.push('{');
            for (i, (key, item)) in members.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(depth + 1, out);
                write_string(key, out);
                out.push_str(": ");
                write_pretty(item, depth + 1, out);
            }
            newline(depth, out);
            out.push('}');
        }
        _ => write_compact(value, out),
    }
}

fn newline(depth: usize, out: &mut String) {
    out.push('\n');
    for _ in 0..depth {
        out.push_str("  ");
    }
}

/// Writes `value` without any whitespace.
pub fn minify(value: &Value) -> String {
    let mut out = String::new();
    write_compact(value, &mut out);
    out
}

fn write_compact(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(n),
        Value::String(s) => write_string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_compact(item, out);
            }
            out.push(']');
        }
        Value::Object(members) => {
            out.push('{');
            for (i, (key, item)) in members.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(key, out);
                out.push(':');
                write_compact(item, out);
            }
            out.push('}');
        }
    }
}

/// Writes a JSON string literal.
pub fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Sorts every object's keys, at every depth, in byte order.
pub fn sort_keys(value: &mut Value) {
    match value {
        Value::Array(items) => items.iter_mut().for_each(sort_keys),
        Value::Object(members) => {
            members.sort_by(|a, b| a.0.cmp(&b.0));
            members.iter_mut().for_each(|(_, v)| sort_keys(v));
        }
        _ => {}
    }
}

/// Whether a number lexeme is an integer (no fraction, no exponent).
pub fn is_integer(lexeme: &str) -> bool {
    !lexeme.contains(['.', 'e', 'E'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_order_and_number_text() {
        let v = parse(r#"{"b":1.0,"a":[1e3,-0,true,null,"x\u00e9\ud83d\ude00"]}"#).unwrap();
        assert_eq!(minify(&v), r#"{"b":1.0,"a":[1e3,-0,true,null,"xé😀"]}"#);
        assert_eq!(
            format(&v),
            "{\n  \"b\": 1.0,\n  \"a\": [\n    1e3,\n    -0,\n    true,\n    null,\n    \"xé😀\"\n  ]\n}"
        );
    }

    #[test]
    fn reports_where_it_stopped() {
        let e = parse("{\n  \"a\": 1\n  \"b\": 2\n}").unwrap_err();
        assert_eq!((e.line, e.column), (3, 3));
        assert_eq!(e.what, "expected ',' or '}', found '\"'");
        let e = parse("[1,]").unwrap_err();
        assert_eq!(e.what, "trailing comma before ']'");
        let e = parse("{} x").unwrap_err();
        assert_eq!(e.what, "unexpected text after the value");
        let e = parse("\"abc").unwrap_err();
        assert_eq!(e.what, "unterminated string");
        assert!(parse("").is_err());
        assert!(parse("01").is_err());
        assert!(parse("1.").is_err());
        assert!(parse("\"\\ud800\"").is_err());
    }

    #[test]
    fn deep_nesting_is_refused_not_overflowed() {
        let text = "[".repeat(10_000);
        assert_eq!(parse(&text).unwrap_err().what, "nested too deep");
    }

    #[test]
    fn empty_containers_stay_compact() {
        let v = parse(r#"{"a":{},"b":[],"c":[{}]}"#).unwrap();
        assert_eq!(
            format(&v),
            "{\n  \"a\": {},\n  \"b\": [],\n  \"c\": [\n    {}\n  ]\n}"
        );
    }

    #[test]
    fn sorts_at_every_depth() {
        let mut v = parse(r#"{"b":{"z":1,"y":2},"a":[{"d":1,"c":2}]}"#).unwrap();
        sort_keys(&mut v);
        assert_eq!(minify(&v), r#"{"a":[{"c":2,"d":1}],"b":{"y":2,"z":1}}"#);
    }
}
