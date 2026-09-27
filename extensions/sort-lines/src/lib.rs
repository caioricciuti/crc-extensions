//! Sort Lines: the selected lines, or the whole document, in order.

use crc_extension::{Input, Output};

pub fn sort(input: Input) -> Output {
    sorted(input, false)
}

pub fn sort_reverse(input: Input) -> Output {
    sorted(input, true)
}

pub fn unique(input: Input) -> Output {
    let (lines, trailing) = split(&input.text);
    let mut seen = std::collections::HashSet::new();
    let kept: Vec<&str> = lines.into_iter().filter(|l| seen.insert(*l)).collect();
    let removed = input.text.lines().count() - kept.len();
    Output::replace(join(&kept, trailing)).with_message(match removed {
        0 => "no duplicate lines".to_owned(),
        1 => "removed 1 duplicate line".to_owned(),
        n => format!("removed {n} duplicate lines"),
    })
}

fn sorted(input: Input, reverse: bool) -> Output {
    let (mut lines, trailing) = split(&input.text);
    lines.sort_unstable();
    if reverse {
        lines.reverse();
    }
    Output::replace(join(&lines, trailing))
}

/// The lines, and the line break to put back between them: CRLF when the
/// text uses it (`lines()` drops the `\r`, and joining with `\n` turned a
/// CRLF selection into LF), and whether it ended with one.
fn split(text: &str) -> (Vec<&str>, Break) {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    (
        text.lines().collect(),
        Break {
            newline,
            trailing: text.ends_with('\n'),
        },
    )
}

struct Break {
    newline: &'static str,
    trailing: bool,
}

fn join(lines: &[&str], end: Break) -> String {
    let mut out = lines.join(end.newline);
    if end.trailing {
        out.push_str(end.newline);
    }
    out
}

crc_extension::commands! {
    "sort" => sort,
    "sort_reverse" => sort_reverse,
    "unique" => unique,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(f: fn(Input) -> Output, text: &str) -> String {
        f(Input {
            text: text.into(),
            ..Input::default()
        })
        .replace
        .unwrap()
    }

    #[test]
    fn sorts_and_keeps_the_trailing_newline() {
        assert_eq!(run(sort, "b\na\nc\n"), "a\nb\nc\n");
        assert_eq!(run(sort, "b\na"), "a\nb");
        assert_eq!(run(sort_reverse, "b\na\nc\n"), "c\nb\na\n");
        assert_eq!(run(unique, "a\nb\na\n"), "a\nb\n");
    }

    #[test]
    fn crlf_stays_crlf() {
        assert_eq!(run(sort, "b\r\na\r\n"), "a\r\nb\r\n");
    }
}
