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

fn split(text: &str) -> (Vec<&str>, bool) {
    (text.lines().collect(), text.ends_with('\n'))
}

fn join(lines: &[&str], trailing: bool) -> String {
    let mut out = lines.join("\n");
    if trailing {
        out.push('\n');
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
}
