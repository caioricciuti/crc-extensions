//! Lining up a delimiter across consecutive lines.
//!
//! A group is a run of lines that share their indentation and each contain
//! the delimiter. A line without it, or with different indentation, ends the
//! group, so a nested block does not get aligned with the code around it.

use crate::lines;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    /// `=` and the compound tokens that contain it: `==`, `!=`, `<=`, `>=`,
    /// `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `=>`, `:=`.
    Equals,
    /// A single `:`, never part of `::`.
    Colon,
}

/// Where a line splits: the byte offset the token starts at, and where the
/// value after it begins.
struct Split {
    token_start: usize,
    token_end: usize,
}

fn find(body: &str, delimiter: Delimiter) -> Option<Split> {
    match delimiter {
        Delimiter::Equals => {
            let at = body.find('=')?;
            let before = body[..at].chars().next_back();
            let token_start = match before {
                Some(c) if "+-*/%&|^!<>:".contains(c) => at - c.len_utf8(),
                _ => at,
            };
            let after = body[at + 1..].chars().next();
            let token_end = match after {
                Some(c @ ('=' | '>')) => at + 1 + c.len_utf8(),
                _ => at + 1,
            };
            Some(Split {
                token_start,
                token_end,
            })
        }
        Delimiter::Colon => {
            let mut search = 0;
            while let Some(found) = body[search..].find(':') {
                let at = search + found;
                let doubled = body[at + 1..].starts_with(':') || body[..at].ends_with(':');
                if !doubled {
                    return Some(Split {
                        token_start: at,
                        token_end: at + 1,
                    });
                }
                search = at + 1;
            }
            None
        }
    }
}

fn indentation(body: &str) -> &str {
    &body[..body.len() - body.trim_start().len()]
}

pub fn align(text: &str, delimiter: Delimiter) -> String {
    let lines = lines(text);
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while at < lines.len() {
        // Collect a group: same indentation, delimiter present.
        let Some(first) = find(lines[at].body, delimiter) else {
            out.push_str(lines[at].body);
            out.push_str(lines[at].ending);
            at += 1;
            continue;
        };
        let indent = indentation(lines[at].body);
        let mut group = vec![(lines[at], first)];
        let mut next = at + 1;
        while next < lines.len() {
            let line = lines[next];
            if indentation(line.body) != indent {
                break;
            }
            match find(line.body, delimiter) {
                Some(split) => group.push((line, split)),
                None => break,
            }
            next += 1;
        }
        let width = group
            .iter()
            .map(|(line, split)| left_of(line.body, split).chars().count())
            .max()
            .unwrap_or(0);
        for (line, split) in &group {
            let left = left_of(line.body, split);
            out.push_str(left);
            match delimiter {
                Delimiter::Equals => {
                    out.extend(std::iter::repeat_n(' ', width - left.chars().count() + 1));
                    out.push_str(&line.body[split.token_start..]);
                }
                Delimiter::Colon => {
                    out.push(':');
                    let value = line.body[split.token_end..].trim();
                    if !value.is_empty() {
                        out.extend(std::iter::repeat_n(' ', width - left.chars().count() + 1));
                        out.push_str(value);
                    }
                }
            }
            out.push_str(line.ending);
        }
        at = next;
    }
    out
}

/// The text before the token, without the spaces that separated them.
fn left_of<'a>(body: &'a str, split: &Split) -> &'a str {
    body[..split.token_start].trim_end()
}
