//! Just enough JSON to show a JWT: a parser that keeps key order and number
//! lexemes as written, and a two-space pretty printer.

/// A JSON value. Numbers stay as the text they were written as, so nothing
/// is rounded on the way through.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    /// The member `key` of an object.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

/// Parses one JSON document.
pub fn parse(text: &str) -> Result<Value, String> {
    let mut p = Parser {
        chars: text.chars().collect(),
        at: 0,
    };
    p.ws();
    let value = p.value(0)?;
    p.ws();
    if p.at < p.chars.len() {
        return Err(format!("unexpected text after the value at {}", p.where_()));
    }
    Ok(value)
}

/// Pretty prints with two-space indentation.
pub fn pretty(value: &Value) -> String {
    let mut out = String::new();
    write(value, 0, &mut out);
    out
}

fn write(value: &Value, depth: usize, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(n),
        Value::String(s) => escape(s, out),
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                out.push_str(if i == 0 { "\n" } else { ",\n" });
                indent(depth + 1, out);
                write(item, depth + 1, out);
            }
            out.push('\n');
            indent(depth, out);
            out.push(']');
        }
        Value::Object(members) => {
            if members.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (key, item)) in members.iter().enumerate() {
                out.push_str(if i == 0 { "\n" } else { ",\n" });
                indent(depth + 1, out);
                escape(key, out);
                out.push_str(": ");
                write(item, depth + 1, out);
            }
            out.push('\n');
            indent(depth, out);
            out.push('}');
        }
    }
}

fn indent(depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str("  ");
    }
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

/// Deeper than this and the input is not a token anyone wrote by hand.
const MAX_DEPTH: usize = 64;

struct Parser {
    chars: Vec<char>,
    at: usize,
}

impl Parser {
    fn where_(&self) -> String {
        let (mut line, mut column) = (1, 1);
        for &c in self.chars.iter().take(self.at) {
            if c == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        format!("line {line}, column {column}")
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += 1;
        Some(c)
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.at += 1;
        }
    }

    fn eat(&mut self, want: char) -> Result<(), String> {
        match self.next() {
            Some(c) if c == want => Ok(()),
            Some(c) => Err(format!(
                "expected {want:?}, found {c:?} at {}",
                self.where_()
            )),
            None => Err(format!(
                "expected {want:?}, found the end at {}",
                self.where_()
            )),
        }
    }

    fn word(&mut self, word: &str, value: Value) -> Result<Value, String> {
        for want in word.chars() {
            self.eat(want)?;
        }
        Ok(value)
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err("nested too deeply".into());
        }
        match self.peek() {
            Some('{') => self.object(depth),
            Some('[') => self.array(depth),
            Some('"') => Ok(Value::String(self.string()?)),
            Some('t') => self.word("true", Value::Bool(true)),
            Some('f') => self.word("false", Value::Bool(false)),
            Some('n') => self.word("null", Value::Null),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            Some(c) => Err(format!("unexpected {c:?} at {}", self.where_())),
            None => Err(format!("unexpected end at {}", self.where_())),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, String> {
        self.eat('{')?;
        let mut members = Vec::new();
        self.ws();
        if self.peek() == Some('}') {
            self.at += 1;
            return Ok(Value::Object(members));
        }
        loop {
            self.ws();
            if self.peek() != Some('"') {
                return Err(format!("expected a key at {}", self.where_()));
            }
            let key = self.string()?;
            self.ws();
            self.eat(':')?;
            self.ws();
            let value = self.value(depth + 1)?;
            members.push((key, value));
            self.ws();
            match self.next() {
                Some(',') => continue,
                Some('}') => return Ok(Value::Object(members)),
                _ => return Err(format!("expected ',' or '}}' at {}", self.where_())),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, String> {
        self.eat('[')?;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(']') {
            self.at += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.ws();
            items.push(self.value(depth + 1)?);
            self.ws();
            match self.next() {
                Some(',') => continue,
                Some(']') => return Ok(Value::Array(items)),
                _ => return Err(format!("expected ',' or ']' at {}", self.where_())),
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat('"')?;
        let mut out = String::new();
        loop {
            match self.next() {
                None => return Err("unterminated string".into()),
                Some('"') => return Ok(out),
                Some('\\') => match self.next() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some('b') => out.push('\u{8}'),
                    Some('f') => out.push('\u{c}'),
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some('u') => {
                        let code = self.hex4()?;
                        let c = if (0xd800..0xdc00).contains(&code) {
                            // A surrogate pair, written as two escapes.
                            self.eat('\\')?;
                            self.eat('u')?;
                            let low = self.hex4()?;
                            if !(0xdc00..0xe000).contains(&low) {
                                return Err("a lone surrogate escape".into());
                            }
                            0x10000 + ((code - 0xd800) << 10) + (low - 0xdc00)
                        } else {
                            code
                        };
                        out.push(char::from_u32(c).unwrap_or('\u{fffd}'));
                    }
                    _ => return Err(format!("bad escape at {}", self.where_())),
                },
                Some(c) => out.push(c),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let mut code = 0;
        for _ in 0..4 {
            let digit = self
                .next()
                .and_then(|c| c.to_digit(16))
                .ok_or_else(|| format!("bad \\u escape at {}", self.where_()))?;
            code = (code << 4) | digit;
        }
        Ok(code)
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.at;
        while matches!(self.peek(), Some('-' | '+' | '.' | 'e' | 'E' | '0'..='9')) {
            self.at += 1;
        }
        let text: String = self.chars[start..self.at].iter().collect();
        if text.parse::<f64>().is_err() {
            return Err(format!("bad number {text:?} at {}", self.where_()));
        }
        Ok(Value::Number(text))
    }
}
