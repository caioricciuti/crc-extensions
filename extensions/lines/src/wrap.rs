//! Filling paragraphs: hard wrap to a width, or unwrap to one line.
//!
//! A paragraph is a run of lines with the same indentation and comment
//! marker, ended by a blank line, a change of either, or a new list item.
//! In prose (crc calls it `text`, which covers Markdown) every paragraph is
//! filled; in code only comment lines are, everything else passes through
//! untouched, as do fenced code blocks, tables and headings.

use crate::{Line, ending_of, lines};

/// Comment markers a paragraph may start with, longest first so `///` is
/// not read as `//` followed by `/`.
const CODE_MARKERS: &[&str] = &["//!", "///", "//", "#", "--", ";", "*", ">"];
const PROSE_MARKERS: &[&str] = &[">"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Parsed<'a> {
    indent: &'a str,
    marker: Option<&'a str>,
    /// A list bullet, `- `, `* `, `+ ` or `1. `, with its trailing space.
    bullet: Option<&'a str>,
    /// The text after all of the above.
    rest: &'a str,
}

fn parse<'a>(body: &'a str, prose: bool) -> Parsed<'a> {
    let indent = &body[..body.len() - body.trim_start().len()];
    let mut rest = &body[indent.len()..];
    let markers = if prose { PROSE_MARKERS } else { CODE_MARKERS };
    let mut marker = None;
    for m in markers {
        if let Some(after) = rest.strip_prefix(m)
            && (after.is_empty() || after.starts_with(' '))
        {
            marker = Some(&rest[..m.len()]);
            rest = after.trim_start_matches(' ');
            break;
        }
    }
    let bullet = bullet(rest);
    if let Some(b) = bullet {
        rest = &rest[b.len()..];
    }
    Parsed {
        indent,
        marker,
        bullet,
        rest,
    }
}

fn bullet(text: &str) -> Option<&str> {
    if text.starts_with("- ") || text.starts_with("* ") || text.starts_with("+ ") {
        return Some(&text[..2]);
    }
    let digits = text.len() - text.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits > 0 && digits <= 4 {
        let after = &text[digits..];
        if after.starts_with(". ") || after.starts_with(") ") {
            return Some(&text[..digits + 2]);
        }
    }
    None
}

fn is_fence(body: &str) -> bool {
    let t = body.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// Lines that are never filled, besides code outside comments.
fn passthrough(parsed: &Parsed<'_>, prose: bool) -> bool {
    parsed.rest.trim().is_empty()
        || parsed.rest.starts_with('|')
        || (prose && parsed.marker.is_none() && parsed.rest.starts_with('#'))
        || (prose
            && parsed.marker.is_none()
            && parsed.bullet.is_none()
            && parsed.indent.chars().count() >= 4)
        || (!prose && parsed.marker.is_none())
}

struct Paragraph<'a> {
    indent: &'a str,
    marker: Option<&'a str>,
    bullet: Option<&'a str>,
    lines: Vec<Line<'a>>,
    words: Vec<&'a str>,
}

impl<'a> Paragraph<'a> {
    fn start(line: Line<'a>, parsed: Parsed<'a>) -> Paragraph<'a> {
        Paragraph {
            indent: parsed.indent,
            marker: parsed.marker,
            bullet: parsed.bullet,
            lines: vec![line],
            words: parsed.rest.split_whitespace().collect(),
        }
    }

    /// Whether `parsed` continues this paragraph: same marker, no new
    /// bullet, and either the same indentation or, under a bullet, the
    /// hanging indentation a wrapped continuation line has.
    fn accepts(&self, parsed: &Parsed<'a>) -> bool {
        if parsed.marker != self.marker || parsed.bullet.is_some() || parsed.rest.trim().is_empty()
        {
            return false;
        }
        if parsed.indent == self.indent {
            return true;
        }
        match (self.marker, self.bullet) {
            (None, Some(bullet)) => {
                parsed.indent.len() == self.indent.len() + bullet.chars().count()
                    && parsed.indent.starts_with(self.indent)
                    && parsed.indent[self.indent.len()..].chars().all(|c| c == ' ')
            }
            _ => false,
        }
    }

    fn first_prefix(&self) -> String {
        let mut p = String::from(self.indent);
        if let Some(m) = self.marker {
            p.push_str(m);
            p.push(' ');
        }
        if let Some(b) = self.bullet {
            p.push_str(b);
        }
        p
    }

    fn continuation_prefix(&self) -> String {
        let mut p = String::from(self.indent);
        if let Some(m) = self.marker {
            p.push_str(m);
            p.push(' ');
        }
        if let Some(b) = self.bullet {
            p.extend(std::iter::repeat_n(' ', b.chars().count()));
        }
        p
    }

    fn write(&self, width: Option<usize>, default_ending: &str, out: &mut String) {
        if self.words.is_empty() {
            for line in &self.lines {
                out.push_str(line.body);
                out.push_str(line.ending);
            }
            return;
        }
        let first = self.first_prefix();
        let continuation = self.continuation_prefix();
        let mut filled: Vec<String> = Vec::new();
        let mut line = first.clone();
        let mut len = first.chars().count();
        let mut has_word = false;
        for word in &self.words {
            let word_len = word.chars().count();
            if has_word && width.is_some_and(|w| len + 1 + word_len > w) {
                filled.push(std::mem::replace(&mut line, continuation.clone()));
                len = continuation.chars().count();
                has_word = false;
            }
            if has_word {
                line.push(' ');
                len += 1;
            }
            line.push_str(word);
            len += word_len;
            has_word = true;
        }
        filled.push(line);
        let last_ending = self.lines.last().map_or("", |l| l.ending);
        let count = filled.len();
        for (i, text) in filled.iter().enumerate() {
            out.push_str(text);
            if i + 1 == count {
                out.push_str(last_ending);
            } else {
                // Endings the paragraph had at this position, except the
                // last one, which may be the missing final newline.
                let original = self
                    .lines
                    .get(i)
                    .filter(|_| i + 1 < self.lines.len())
                    .map(|l| l.ending);
                out.push_str(original.unwrap_or(default_ending));
            }
        }
    }
}

/// Fills every paragraph to `width` columns, or to one line when `None`.
pub fn fill(text: &str, language: &str, width: Option<usize>) -> String {
    let prose = matches!(language, "text" | "markdown" | "");
    let default_ending = ending_of(text);
    let mut out = String::with_capacity(text.len() + text.len() / 8);
    let mut current: Option<Paragraph<'_>> = None;
    let mut in_fence = false;
    for line in lines(text) {
        if is_fence(line.body) {
            in_fence = !in_fence;
        }
        let parsed = parse(line.body, prose);
        let through = in_fence || is_fence(line.body) || passthrough(&parsed, prose);
        if through {
            if let Some(p) = current.take() {
                p.write(width, default_ending, &mut out);
            }
            out.push_str(line.body);
            out.push_str(line.ending);
            continue;
        }
        match current.as_mut() {
            Some(p) if p.accepts(&parsed) => {
                p.lines.push(line);
                p.words.extend(parsed.rest.split_whitespace());
            }
            _ => {
                if let Some(p) = current.take() {
                    p.write(width, default_ending, &mut out);
                }
                current = Some(Paragraph::start(line, parsed));
            }
        }
    }
    if let Some(p) = current.take() {
        p.write(width, default_ending, &mut out);
    }
    out
}
