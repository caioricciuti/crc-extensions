//! YAML out and in. Out is block style, always. In is the subset real
//! configuration files use: block and flow collections, the three scalar
//! styles, literal and folded blocks, comments. No anchors, aliases or tags.

use crate::json::Value;
use std::fmt;

// ---------------------------------------------------------------- writing

/// Writes `value` as block-style YAML with two-space indentation.
pub fn emit(value: &Value) -> String {
    let mut out = String::new();
    match value {
        Value::Array(items) if !items.is_empty() => write_sequence(items, 0, &mut out),
        Value::Object(members) if !members.is_empty() => write_mapping(members, 0, &mut out),
        _ => {
            write_scalar_or_empty(value, 0, &mut out);
            out.push('\n');
        }
    }
    out
}

fn indent(depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn write_sequence(items: &[Value], depth: usize, out: &mut String) {
    for item in items {
        indent(depth, out);
        out.push('-');
        match item {
            Value::Array(inner) if !inner.is_empty() => {
                out.push('\n');
                write_sequence(inner, depth + 1, out);
            }
            Value::Object(members) if !members.is_empty() => {
                // The first key shares the dash's line, the rest align under it.
                out.push(' ');
                write_members(members, depth + 1, true, out);
            }
            _ => {
                out.push(' ');
                write_scalar_or_empty(item, depth + 1, out);
                out.push('\n');
            }
        }
    }
}

fn write_mapping(members: &[(String, Value)], depth: usize, out: &mut String) {
    write_members(members, depth, false, out);
}

/// Writes the members of a mapping at `depth`. With `first_inline`, the
/// first key is written where the cursor already is (after `- `).
fn write_members(members: &[(String, Value)], depth: usize, first_inline: bool, out: &mut String) {
    for (i, (key, value)) in members.iter().enumerate() {
        if !(first_inline && i == 0) {
            indent(depth, out);
        }
        write_key(key, out);
        out.push(':');
        match value {
            Value::Array(items) if !items.is_empty() => {
                out.push('\n');
                write_sequence(items, depth + 1, out);
            }
            Value::Object(inner) if !inner.is_empty() => {
                out.push('\n');
                write_mapping(inner, depth + 1, out);
            }
            _ => {
                out.push(' ');
                write_scalar_or_empty(value, depth + 1, out);
                out.push('\n');
            }
        }
    }
}

fn write_key(key: &str, out: &mut String) {
    if needs_quotes(key) || key.contains('\n') {
        write_double_quoted(key, out);
    } else {
        out.push_str(key);
    }
}

/// Writes a scalar, or `[]` / `{}` for an empty collection. A multi-line
/// string becomes a literal block whose lines sit at `depth`.
fn write_scalar_or_empty(value: &Value, depth: usize, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(n),
        Value::Array(_) => out.push_str("[]"),
        Value::Object(_) => out.push_str("{}"),
        Value::String(s) => write_string(s, depth, out),
    }
}

fn write_string(s: &str, depth: usize, out: &mut String) {
    if s.contains('\n') && !s.chars().any(|c| c != '\n' && c.is_control()) {
        // Literal block. Chomping keeps exactly the trailing newlines.
        let trailing = s.len() - s.trim_end_matches('\n').len();
        out.push('|');
        match trailing {
            0 => out.push('-'),
            1 => {}
            _ => out.push('+'),
        }
        let body = s.trim_end_matches('\n');
        if body.starts_with(' ') {
            // A leading space needs an explicit indentation indicator.
            out.push('2');
        }
        for line in body.split('\n') {
            out.push('\n');
            if !line.is_empty() {
                indent(depth, out);
                out.push_str(line);
            }
        }
        return;
    }
    if needs_quotes(s) {
        write_double_quoted(s, out);
    } else {
        out.push_str(s);
    }
}

/// Whether a plain scalar would read back as something else, or not at all.
fn needs_quotes(s: &str) -> bool {
    if s.is_empty() || s != s.trim() {
        return true;
    }
    let first = s.chars().next().unwrap_or(' ');
    if "-?:,[]{}&*!|>'\"%@`#".contains(first) {
        return true;
    }
    if s.contains(": ") || s.ends_with(':') || s.contains(" #") || s.chars().any(char::is_control) {
        return true;
    }
    if plain_scalar(s) != Value::String(s.to_owned()) {
        return true;
    }
    // Older YAML readers turn these into booleans, and dates into
    // timestamps; quoting keeps them strings everywhere.
    let lower = s.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "yes" | "no" | "on" | "off" | ".inf" | ".nan" | "-.inf"
    ) {
        return true;
    }
    let b = s.as_bytes();
    b.len() >= 10 && b[..4].iter().all(u8::is_ascii_digit) && b[4] == b'-'
}

fn write_double_quoted(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

// ---------------------------------------------------------------- reading

#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub line: usize,
    pub what: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.what)
    }
}

fn err(line: usize, what: impl Into<String>) -> Error {
    Error {
        line,
        what: what.into(),
    }
}

/// One source line: its indentation in spaces, its content with the
/// indentation and any comment removed, and its 1-based number. `raw` is
/// the line as written, for block scalars.
#[derive(Debug, Clone)]
struct Line {
    indent: usize,
    text: String,
    raw: String,
    number: usize,
}

impl Line {
    fn is_blank(&self) -> bool {
        self.text.is_empty()
    }
}

/// Parses one YAML document.
pub fn parse(text: &str) -> Result<Value, Error> {
    let mut lines = Vec::new();
    let mut documents = 0;
    for (i, raw) in text.lines().enumerate() {
        let number = i + 1;
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        if raw.contains('\t') && raw.trim_start_matches(' ').starts_with('\t') {
            return Err(err(number, "tabs are not allowed for indentation"));
        }
        if raw.starts_with('%') && lines.is_empty() {
            continue;
        }
        if raw == "---" || raw.starts_with("--- ") {
            documents += 1;
            if documents > 1 {
                return Err(err(number, "more than one document; convert one at a time"));
            }
            let rest = raw.get(3..).unwrap_or("").trim();
            if rest.is_empty() {
                continue;
            }
            lines.push(Line {
                indent: 0,
                text: strip_comment(rest).trim_end().to_owned(),
                raw: rest.to_owned(),
                number,
            });
            continue;
        }
        if raw == "..." {
            break;
        }
        let indent = raw.len() - raw.trim_start_matches(' ').len();
        let content = raw.get(indent..).unwrap_or("");
        lines.push(Line {
            indent,
            text: strip_comment(content).trim_end().to_owned(),
            raw: raw.to_owned(),
            number,
        });
    }
    let mut p = Reader {
        lines,
        at: 0,
        depth: 0,
    };
    p.skip_blank();
    if p.at >= p.lines.len() {
        return Ok(Value::Null);
    }
    let first_indent = p.lines.get(p.at).map_or(0, |l| l.indent);
    let value = p.node(first_indent)?;
    p.skip_blank();
    if let Some(line) = p.lines.get(p.at) {
        return Err(err(
            line.number,
            "unexpected content; is the indentation right?",
        ));
    }
    Ok(value)
}

/// Removes a `# comment` that is not inside quotes. A `#` counts as a
/// comment only at the start or after a space.
fn strip_comment(text: &str) -> &str {
    let mut single = false;
    let mut double = false;
    let mut prev = ' ';
    let mut escaped = false;
    for (i, c) in text.char_indices() {
        if double {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                double = false;
            }
        } else if single {
            if c == '\'' {
                single = false;
            }
        } else if c == '#' && (i == 0 || prev == ' ' || prev == '\t') {
            return text.get(..i).unwrap_or(text);
        } else if c == '"' && (i == 0 || !prev.is_alphanumeric()) {
            double = true;
        } else if c == '\'' && (i == 0 || !prev.is_alphanumeric()) {
            single = true;
        }
        prev = c;
    }
    text
}

struct Reader {
    lines: Vec<Line>,
    at: usize,
    depth: usize,
}

/// Nesting deeper than this is refused rather than overflowing the stack.
const MAX_DEPTH: usize = 512;

impl Reader {
    fn skip_blank(&mut self) {
        while self.lines.get(self.at).is_some_and(Line::is_blank) {
            self.at += 1;
        }
    }

    fn current(&self) -> Option<&Line> {
        self.lines.get(self.at)
    }

    /// The next non-blank line, without moving.
    fn next_content(&self) -> Option<&Line> {
        self.lines.iter().skip(self.at).find(|l| !l.is_blank())
    }

    /// Parses the node that starts at the current line, which sits at
    /// `indent`.
    fn node(&mut self, indent: usize) -> Result<Value, Error> {
        self.skip_blank();
        let line = match self.current() {
            Some(l) => l.clone(),
            None => return Ok(Value::Null),
        };
        if line.indent != indent {
            return Err(err(line.number, "unexpected indentation"));
        }
        if self.depth >= MAX_DEPTH {
            return Err(err(line.number, "nested too deep"));
        }
        self.depth += 1;
        let value = if is_dash(&line.text) {
            self.sequence(indent)
        } else if split_key(&line.text, line.number)?.is_some() {
            self.mapping(indent)
        } else {
            self.scalar(&line.text, indent, line.number)
        };
        self.depth -= 1;
        value
    }

    fn sequence(&mut self, indent: usize) -> Result<Value, Error> {
        let mut items = Vec::new();
        loop {
            self.skip_blank();
            let line = match self.current() {
                Some(l) if l.indent == indent && is_dash(&l.text) => l.clone(),
                Some(l) if l.indent > indent => {
                    return Err(err(l.number, "unexpected indentation in a sequence"));
                }
                _ => return Ok(Value::Array(items)),
            };
            let rest = line.text.get(1..).unwrap_or("");
            let spaces = rest.len() - rest.trim_start_matches(' ').len();
            let rest = rest.trim_start_matches(' ');
            if rest.is_empty() {
                self.at += 1;
                items.push(self.nested(indent, line.number)?);
                continue;
            }
            // The item continues on this line: pretend the rest is a line
            // of its own, indented to where it starts, and parse it as a
            // node. A mapping's or sequence's later lines align with it.
            let inner_indent = indent + 1 + spaces;
            if let Some(l) = self.lines.get_mut(self.at) {
                l.indent = inner_indent;
                l.text = rest.to_owned();
            }
            items.push(self.node(inner_indent)?);
        }
    }

    fn mapping(&mut self, indent: usize) -> Result<Value, Error> {
        let mut members: Vec<(String, Value)> = Vec::new();
        loop {
            self.skip_blank();
            let line = match self.current() {
                Some(l) if l.indent == indent && !is_dash(&l.text) => l.clone(),
                Some(l) if l.indent > indent => {
                    return Err(err(l.number, "unexpected indentation in a mapping"));
                }
                _ => return Ok(Value::Object(members)),
            };
            let (key, rest) = match split_key(&line.text, line.number)? {
                Some(kv) => kv,
                None => return Err(err(line.number, "expected 'key: value'")),
            };
            if members.iter().any(|(k, _)| *k == key) {
                return Err(err(line.number, format!("duplicate key '{key}'")));
            }
            let value = if rest.is_empty() {
                self.at += 1;
                // `key:` followed by a sequence at the same indentation is
                // allowed, and common.
                match self.next_content() {
                    Some(l) if l.indent == indent && is_dash(&l.text) => {
                        self.skip_blank();
                        self.sequence(indent)?
                    }
                    _ => self.nested(indent, line.number)?,
                }
            } else {
                self.scalar(&rest, indent, line.number)?
            };
            members.push((key, value));
        }
    }

    /// The node on the following lines, indented more than `indent`, or
    /// null when there is none.
    fn nested(&mut self, indent: usize, _line: usize) -> Result<Value, Error> {
        match self.next_content() {
            Some(l) if l.indent > indent => {
                let inner = l.indent;
                self.node(inner)
            }
            _ => Ok(Value::Null),
        }
    }

    /// A value written on the current line after `key:` or `- `. Consumes
    /// the line, plus continuation lines for block and plain scalars.
    fn scalar(&mut self, text: &str, indent: usize, number: usize) -> Result<Value, Error> {
        let text = text.trim();
        if let Some(header) = text.strip_prefix('|').or_else(|| text.strip_prefix('>')) {
            let folded = text.starts_with('>');
            self.at += 1;
            return self.block_scalar(header, folded, indent, number);
        }
        self.at += 1;
        if let Some(first) = text.chars().next() {
            if first == '&' || first == '*' || first == '!' {
                return Err(err(number, "anchors, aliases and tags are not supported"));
            }
            if first == '@' || first == '`' {
                return Err(err(
                    number,
                    format!("a plain scalar cannot start with '{first}'"),
                ));
            }
        }
        if text.starts_with('"') || text.starts_with('\'') {
            let (value, rest) = quoted(text, number)?;
            if !rest.trim().is_empty() {
                return Err(err(number, "unexpected text after the quoted string"));
            }
            return Ok(Value::String(value));
        }
        if text.starts_with('[') || text.starts_with('{') {
            let mut flow = Flow {
                chars: text.chars().collect(),
                at: 0,
                line: number,
                depth: 0,
            };
            let value = flow.value()?;
            flow.ws();
            if flow.at < flow.chars.len() {
                return Err(err(number, "unexpected text after the flow collection"));
            }
            return Ok(value);
        }
        // A plain scalar may continue on more-indented lines.
        let mut joined = text.to_owned();
        while let Some(l) = self.current() {
            if l.is_blank() {
                break;
            }
            if l.indent <= indent || is_dash(&l.text) || split_key(&l.text, l.number)?.is_some() {
                break;
            }
            joined.push(' ');
            joined.push_str(l.text.trim());
            self.at += 1;
        }
        Ok(plain_scalar(&joined))
    }

    fn block_scalar(
        &mut self,
        header: &str,
        folded: bool,
        indent: usize,
        number: usize,
    ) -> Result<Value, Error> {
        let mut chomp = Chomp::Clip;
        let mut explicit = None;
        for c in header.chars() {
            match c {
                '-' => chomp = Chomp::Strip,
                '+' => chomp = Chomp::Keep,
                '1'..='9' => explicit = c.to_digit(10).map(|d| indent + d as usize),
                _ => return Err(err(number, "bad block scalar header")),
            }
        }
        // The block is every following line that is blank or indented past
        // the parent.
        let mut raw_lines: Vec<&str> = Vec::new();
        let mut end = self.at;
        while let Some(l) = self.lines.get(end) {
            let is_blank = l.raw.trim().is_empty();
            if !is_blank && l.indent <= indent {
                break;
            }
            raw_lines.push(&l.raw);
            end += 1;
        }
        let content_indent = explicit.unwrap_or_else(|| {
            raw_lines
                .iter()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.len() - l.trim_start_matches(' ').len())
                .next()
                .unwrap_or(indent + 1)
        });
        let mut lines: Vec<String> = raw_lines
            .iter()
            .map(|l| {
                let cut = content_indent.min(l.len() - l.trim_start_matches(' ').len());
                l.get(cut..).unwrap_or("").to_owned()
            })
            .collect();
        self.at = end;
        // Trailing blank lines are what chomping decides about.
        let mut trailing = 0;
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
            trailing += 1;
        }
        let mut body = String::new();
        if folded {
            let mut prev_blank = false;
            let mut prev_more_indented = false;
            for (i, l) in lines.iter().enumerate() {
                let blank = l.trim().is_empty();
                let more_indented = l.starts_with(' ') || l.starts_with('\t');
                if i > 0 {
                    if blank {
                        body.push('\n');
                    } else if prev_blank || more_indented || prev_more_indented {
                        if !prev_blank {
                            body.push('\n');
                        }
                    } else {
                        body.push(' ');
                    }
                }
                if !blank {
                    body.push_str(l);
                }
                prev_blank = blank;
                prev_more_indented = more_indented;
            }
        } else {
            body = lines.join("\n");
        }
        match chomp {
            Chomp::Strip => {}
            Chomp::Clip => {
                if !body.is_empty() {
                    body.push('\n');
                }
            }
            Chomp::Keep => {
                for _ in 0..=trailing {
                    body.push('\n');
                }
            }
        }
        Ok(Value::String(body))
    }
}

enum Chomp {
    Strip,
    Clip,
    Keep,
}

fn is_dash(text: &str) -> bool {
    text == "-" || text.starts_with("- ")
}

/// Splits `key: rest` at the first `:` that ends the key, or returns
/// `None` when the line is not a mapping entry.
fn split_key(text: &str, number: usize) -> Result<Option<(String, String)>, Error> {
    if text.is_empty() || text.starts_with('[') || text.starts_with('{') {
        return Ok(None);
    }
    if text.starts_with('"') || text.starts_with('\'') {
        let (key, rest) = quoted(text, number)?;
        let rest = rest.trim_start();
        return Ok(match rest.strip_prefix(':') {
            Some(after) if after.is_empty() || after.starts_with(' ') => {
                Some((key, after.trim().to_owned()))
            }
            _ => None,
        });
    }
    let chars: Vec<char> = text.chars().collect();
    for i in 0..chars.len() {
        if chars.get(i) == Some(&':') {
            let next = chars.get(i + 1);
            if next.is_none() || next == Some(&' ') {
                let key: String = chars.iter().take(i).collect();
                let rest: String = chars.iter().skip(i + 1).collect();
                return Ok(Some((key.trim().to_owned(), rest.trim().to_owned())));
            }
        }
    }
    Ok(None)
}

/// Reads a quoted scalar at the start of `text`; returns it and what
/// follows the closing quote.
fn quoted(text: &str, number: usize) -> Result<(String, String), Error> {
    let mut chars = text.char_indices();
    let (_, quote) = chars
        .next()
        .ok_or_else(|| err(number, "expected a string"))?;
    let mut out = String::new();
    let mut escaped = false;
    for (i, c) in chars {
        if quote == '"' {
            if escaped {
                escaped = false;
                match c {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    '0' => out.push('\0'),
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    ' ' => out.push(' '),
                    'u' | 'x' | 'U' => {
                        let width = match c {
                            'x' => 2,
                            'u' => 4,
                            _ => 8,
                        };
                        let start = i + 1;
                        let hex = text
                            .get(start..start + width)
                            .ok_or_else(|| err(number, "bad unicode escape"))?;
                        let code = u32::from_str_radix(hex, 16)
                            .map_err(|_| err(number, "bad unicode escape"))?;
                        out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        // Skip the hex digits: handled by re-slicing below.
                        let rest = text.get(start + width..).unwrap_or("");
                        return finish_double(out, rest, number);
                    }
                    other => return Err(err(number, format!("bad escape '\\{other}'"))),
                }
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                let rest = text.get(i + 1..).unwrap_or("").to_owned();
                return Ok((out, rest));
            } else {
                out.push(c);
            }
        } else if c == '\'' {
            let rest = text.get(i + 1..).unwrap_or("");
            if let Some(after) = rest.strip_prefix('\'') {
                out.push('\'');
                // Continue after the doubled quote by recursing on the tail.
                let mut tail = String::from("'");
                tail.push_str(after);
                let (more, rest) = quoted(&tail, number)?;
                out.push_str(&more);
                return Ok((out, rest));
            }
            return Ok((out, rest.to_owned()));
        } else {
            out.push(c);
        }
    }
    Err(err(number, "unterminated quoted string"))
}

/// Continues a double-quoted string after a unicode escape.
fn finish_double(mut out: String, rest: &str, number: usize) -> Result<(String, String), Error> {
    let mut tail = String::from("\"");
    tail.push_str(rest);
    let (more, rest) = quoted(&tail, number)?;
    out.push_str(&more);
    Ok((out, rest))
}

/// What a plain (unquoted) scalar means under the core schema.
pub fn plain_scalar(text: &str) -> Value {
    match text {
        "" | "~" | "null" | "Null" | "NULL" => return Value::Null,
        "true" | "True" | "TRUE" => return Value::Bool(true),
        "false" | "False" | "FALSE" => return Value::Bool(false),
        _ => {}
    }
    if let Some(n) = number_lexeme(text) {
        return Value::Number(n);
    }
    Value::String(text.to_owned())
}

/// The JSON number for a YAML number, or `None` when `text` is not one.
fn number_lexeme(text: &str) -> Option<String> {
    let (negative, body) = match text.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    if body.is_empty() {
        return None;
    }
    let sign = if negative { "-" } else { "" };
    if let Some(hex) = body.strip_prefix("0x") {
        let n = u64::from_str_radix(hex, 16).ok()?;
        return Some(format!("{sign}{n}"));
    }
    if let Some(oct) = body.strip_prefix("0o") {
        let n = u64::from_str_radix(oct, 8).ok()?;
        return Some(format!("{sign}{n}"));
    }
    if body.bytes().all(|b| b.is_ascii_digit()) {
        let trimmed = body.trim_start_matches('0');
        let digits = if trimmed.is_empty() { "0" } else { trimmed };
        return Some(format!("{sign}{digits}"));
    }
    // A float: digits, optional fraction, optional exponent, in a form
    // JSON accepts once normalised.
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(i) => (body.get(..i)?, Some(body.get(i + 1..)?)),
        None => (body, None),
    };
    let (int_part, frac_part) = match mantissa.find('.') {
        Some(i) => (mantissa.get(..i)?, Some(mantissa.get(i + 1..)?)),
        None => (mantissa, None),
    };
    if !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if let Some(f) = frac_part
        && !f.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    if int_part.is_empty() && frac_part.is_none_or(str::is_empty) {
        return None;
    }
    if let Some(e) = exponent {
        let e_digits = e.strip_prefix(['+', '-']).unwrap_or(e);
        if e_digits.is_empty() || !e_digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    let int_norm = {
        let t = int_part.trim_start_matches('0');
        if t.is_empty() { "0" } else { t }
    };
    let mut out = format!("{sign}{int_norm}");
    if let Some(f) = frac_part {
        out.push('.');
        out.push_str(if f.is_empty() { "0" } else { f });
    }
    if let Some(e) = exponent {
        out.push('e');
        out.push_str(e);
    }
    Some(out)
}

/// Flow collections: `[a, b]` and `{a: 1, b: 2}`, nested, on one line.
struct Flow {
    chars: Vec<char>,
    at: usize,
    line: usize,
    depth: usize,
}

impl Flow {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.at += 1;
        }
    }

    fn value(&mut self) -> Result<Value, Error> {
        self.ws();
        if self.depth >= MAX_DEPTH {
            return Err(err(self.line, "nested too deep"));
        }
        self.depth += 1;
        let value = self.collection_or_scalar();
        self.depth -= 1;
        value
    }

    fn collection_or_scalar(&mut self) -> Result<Value, Error> {
        match self.peek() {
            Some('[') => {
                self.at += 1;
                let mut items = Vec::new();
                loop {
                    self.ws();
                    match self.peek() {
                        Some(']') => {
                            self.at += 1;
                            return Ok(Value::Array(items));
                        }
                        None => return Err(err(self.line, "unterminated '['")),
                        _ => {}
                    }
                    let item = self.value()?;
                    // `[a: 1]` is a single-pair mapping; keep it simple and
                    // accept the scalar only.
                    items.push(item);
                    self.ws();
                    match self.peek() {
                        Some(',') => self.at += 1,
                        Some(']') => {}
                        _ => return Err(err(self.line, "expected ',' or ']'")),
                    }
                }
            }
            Some('{') => {
                self.at += 1;
                let mut members = Vec::new();
                loop {
                    self.ws();
                    match self.peek() {
                        Some('}') => {
                            self.at += 1;
                            return Ok(Value::Object(members));
                        }
                        None => return Err(err(self.line, "unterminated '{'")),
                        _ => {}
                    }
                    let key = match self.value()? {
                        Value::String(s) => s,
                        Value::Null => String::new(),
                        Value::Bool(b) => b.to_string(),
                        Value::Number(n) => n,
                        _ => return Err(err(self.line, "a flow mapping key must be a scalar")),
                    };
                    self.ws();
                    let value = if self.peek() == Some(':') {
                        self.at += 1;
                        self.value()?
                    } else {
                        Value::Null
                    };
                    members.push((key, value));
                    self.ws();
                    match self.peek() {
                        Some(',') => self.at += 1,
                        Some('}') => {}
                        _ => return Err(err(self.line, "expected ',' or '}'")),
                    }
                }
            }
            Some('"') | Some('\'') => {
                let rest: String = self.chars.iter().skip(self.at).collect();
                let (value, after) = quoted(&rest, self.line)?;
                self.at += rest.chars().count() - after.chars().count();
                Ok(Value::String(value))
            }
            Some(c) if c == '&' || c == '*' || c == '!' => Err(err(
                self.line,
                "anchors, aliases and tags are not supported",
            )),
            _ => {
                // A plain scalar runs to the next `,`, `]`, `}` or `: `.
                let start = self.at;
                while let Some(c) = self.peek() {
                    if c == ',' || c == ']' || c == '}' {
                        break;
                    }
                    if c == ':' && matches!(self.chars.get(self.at + 1), None | Some(' ')) {
                        break;
                    }
                    self.at += 1;
                }
                let text: String = self
                    .chars
                    .iter()
                    .skip(start)
                    .take(self.at - start)
                    .collect();
                Ok(plain_scalar(text.trim()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;

    fn yaml_to_json(text: &str) -> String {
        json::minify(&parse(text).unwrap())
    }

    #[test]
    fn emits_block_style() {
        let v = json::parse(
            r#"{"name":"crc","tags":["a","b"],"nested":{"n":1,"empty":{},"list":[]},"items":[{"id":1,"ok":true},{"id":2,"ok":null}],"text":"line one\nline two\n","odd":"yes","colon":"a: b","num":"42"}"#,
        )
        .unwrap();
        assert_eq!(
            emit(&v),
            "name: crc\ntags:\n  - a\n  - b\nnested:\n  n: 1\n  empty: {}\n  list: []\nitems:\n  - id: 1\n    ok: true\n  - id: 2\n    ok: null\ntext: |\n  line one\n  line two\nodd: \"yes\"\ncolon: \"a: b\"\nnum: \"42\"\n"
        );
    }

    #[test]
    fn reads_mappings_sequences_and_scalars() {
        let text = "# config\nname: crc   # trailing\nversion: 0.2.0\nport: 8080\nhex: 0x1f\nenabled: yes\ndebug: false\nnothing: ~\nempty:\ntags:\n- a\n- 'b c'\n- \"d\\te\"\nservers:\n  - host: one\n    ports: [80, 443]\n  - host: two\n    meta: {a: 1, b: [x, y]}\n";
        assert_eq!(
            yaml_to_json(text),
            r#"{"name":"crc","version":"0.2.0","port":8080,"hex":31,"enabled":"yes","debug":false,"nothing":null,"empty":null,"tags":["a","b c","d\te"],"servers":[{"host":"one","ports":[80,443]},{"host":"two","meta":{"a":1,"b":["x","y"]}}]}"#
        );
    }

    #[test]
    fn reads_block_scalars() {
        let text =
            "a: |\n  one\n  two\n\nb: >-\n  folded\n  text\n\n  para\nc: |+\n  keep\n\nd: plain\n";
        assert_eq!(
            yaml_to_json(text),
            r#"{"a":"one\ntwo\n","b":"folded text\npara","c":"keep\n\n","d":"plain"}"#
        );
    }

    #[test]
    fn nested_sequences_and_plain_continuations() {
        let text = "- - a\n  - b\n- key: value\n  other: spans two\n    lines\n- - c\n";
        assert_eq!(
            yaml_to_json(text),
            r#"[["a","b"],{"key":"value","other":"spans two lines"},["c"]]"#
        );
    }

    #[test]
    fn refuses_what_it_cannot_read() {
        assert_eq!(
            parse("a: &x 1\n").unwrap_err().what,
            "anchors, aliases and tags are not supported"
        );
        assert_eq!(parse("a: 1\na: 2\n").unwrap_err().what, "duplicate key 'a'");
        assert_eq!(parse("---\na: 1\n---\nb: 2\n").unwrap_err().line, 3);
        assert_eq!(parse("a:\n\tb: 1\n").unwrap_err().line, 2);
        assert_eq!(parse("a: 1\n  b: 2\n").unwrap_err().line, 2);
        assert_eq!(parse("").unwrap(), Value::Null);
        assert_eq!(
            parse(&"[".repeat(5000)).unwrap_err().what,
            "nested too deep"
        );
        let deep: String = (0..5000)
            .map(|i| format!("{}a:\n", " ".repeat(i)))
            .collect();
        assert_eq!(parse(&deep).unwrap_err().what, "nested too deep");
    }

    #[test]
    fn round_trips_through_yaml() {
        let text = r#"{"a":[1,{"b":"c: d","e":[]}],"f":{"g":"multi\nline\n","h":"- not a list","i":"","j":" padded "}}"#;
        let v = json::parse(text).unwrap();
        let back = parse(&emit(&v)).unwrap();
        assert_eq!(json::minify(&back), text);
    }
}
