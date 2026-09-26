//! Lines: align, join, number, wrap, sort naturally and tidy lines, and
//! count words. Every command works line by line and keeps each line's own
//! ending (`\n` or `\r\n`), so the text's shape survives.

mod align;
mod wrap;

use crc_extension::{Input, Output};
use std::cmp::Ordering;

/// One line of the text: its body and the ending that followed it, which is
/// empty for the last line of a text that does not end with a newline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line<'a> {
    pub body: &'a str,
    pub ending: &'a str,
}

/// Splits text into lines, keeping each line's ending.
pub fn lines(text: &str) -> Vec<Line<'_>> {
    text.split_inclusive('\n')
        .map(|line| {
            let body = line.strip_suffix('\n').unwrap_or(line);
            let body = body.strip_suffix('\r').unwrap_or(body);
            Line {
                body,
                ending: &line[body.len()..],
            }
        })
        .collect()
}

/// The ending the text uses: that of its first line, `\n` when it has none.
pub fn ending_of(text: &str) -> &'static str {
    match text.find('\n') {
        Some(at) if text[..at].ends_with('\r') => "\r\n",
        _ => "\n",
    }
}

fn blank(line: &Line<'_>) -> bool {
    line.body.trim().is_empty()
}

/// Writes `bodies` in order, with the endings the text had at each position,
/// so reordering lines never moves the missing final newline.
fn with_original_endings(original: &[Line<'_>], bodies: &[&str]) -> String {
    let mut out = String::new();
    for (body, line) in bodies.iter().zip(original) {
        out.push_str(body);
        out.push_str(line.ending);
    }
    out
}

/// Align by =: consecutive lines with the same indentation and an `=` (or a
/// compound token like `==`, `+=`, `=>`, `:=`) get their tokens lined up.
pub fn align_equals(input: Input) -> Output {
    Output::replace(align::align(&input.text, align::Delimiter::Equals))
}

/// Align by :: consecutive lines with the same indentation and a `:` get
/// the text after the colon lined up, `key:   value` style.
pub fn align_colon(input: Input) -> Output {
    Output::replace(align::align(&input.text, align::Delimiter::Colon))
}

/// Join Lines: each paragraph (lines between blank lines) becomes one line,
/// its lines trimmed and joined with a space.
pub fn join(input: Input) -> Output {
    let lines = lines(&input.text);
    let mut out = String::with_capacity(input.text.len());
    let mut paragraph: Vec<&str> = Vec::new();
    let flush = |paragraph: &mut Vec<&str>, ending: &str, out: &mut String| {
        if !paragraph.is_empty() {
            out.push_str(&paragraph.join(" "));
            out.push_str(ending);
            paragraph.clear();
        }
    };
    let mut last_ending = "";
    for line in &lines {
        if blank(line) {
            flush(&mut paragraph, last_ending, &mut out);
            out.push_str(line.ending);
        } else {
            paragraph.push(line.body.trim());
        }
        last_ending = line.ending;
    }
    flush(&mut paragraph, last_ending, &mut out);
    Output::replace(out)
}

/// Number Lines: right-aligned numbers, a space, the line.
pub fn number(input: Input) -> Output {
    let lines = lines(&input.text);
    let width = lines.len().to_string().len();
    let mut out = String::with_capacity(input.text.len() + lines.len() * (width + 1));
    for (i, line) in lines.iter().enumerate() {
        if line.body.is_empty() {
            out.push_str(&format!("{:>width$}{}", i + 1, line.ending));
        } else {
            out.push_str(&format!("{:>width$} {}{}", i + 1, line.body, line.ending));
        }
    }
    Output::replace(out)
}

/// Reverse Lines.
pub fn reverse(input: Input) -> Output {
    let lines = lines(&input.text);
    let bodies: Vec<&str> = lines.iter().rev().map(|l| l.body).collect();
    Output::replace(with_original_endings(&lines, &bodies))
}

/// Sort Lines Naturally: case-insensitive, with runs of digits compared as
/// numbers, so `file2` comes before `file10`. Stable.
pub fn natural_sort(input: Input) -> Output {
    let lines = lines(&input.text);
    let mut bodies: Vec<&str> = lines.iter().map(|l| l.body).collect();
    bodies.sort_by(|a, b| natural_cmp(a, b));
    Output::replace(with_original_endings(&lines, &bodies))
}

/// Sort Lines by Length: shortest first. Stable.
pub fn sort_by_length(input: Input) -> Output {
    let lines = lines(&input.text);
    let mut bodies: Vec<&str> = lines.iter().map(|l| l.body).collect();
    bodies.sort_by_cached_key(|b| b.chars().count());
    Output::replace(with_original_endings(&lines, &bodies))
}

/// Compares two strings the way people read them: letters without regard to
/// case, digit runs by value.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut a = a.chars().peekable();
    let mut b = b.chars().peekable();
    // The first difference in case alone, used only when nothing else
    // separates the two, so "File2" and "file10" still order by number.
    let mut case_tie = Ordering::Equal;
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return case_tie,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let na = digits(&mut a);
                let nb = digits(&mut b);
                let ta = na.trim_start_matches('0');
                let tb = nb.trim_start_matches('0');
                let order = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                a.next();
                b.next();
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                if case_tie == Ordering::Equal {
                    case_tie = x.cmp(&y);
                }
            }
        }
    }
}

fn digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut out = String::new();
    while let Some(c) = chars.peek().copied() {
        if !c.is_ascii_digit() {
            break;
        }
        out.push(c);
        chars.next();
    }
    out
}

/// Trim Trailing Whitespace: spaces and tabs at the end of every line.
pub fn trim_trailing(input: Input) -> Output {
    let mut out = String::with_capacity(input.text.len());
    for line in lines(&input.text) {
        out.push_str(line.body.trim_end_matches([' ', '\t']));
        out.push_str(line.ending);
    }
    Output::replace(out)
}

/// Squeeze Blank Lines: a run of blank lines becomes one empty line.
pub fn squeeze_blank(input: Input) -> Output {
    let mut out = String::with_capacity(input.text.len());
    let mut previous_blank = false;
    for line in lines(&input.text) {
        let is_blank = blank(&line);
        if is_blank && previous_blank {
            continue;
        }
        if !is_blank {
            out.push_str(line.body);
        }
        out.push_str(line.ending);
        previous_blank = is_blank;
    }
    Output::replace(out)
}

/// Remove Blank Lines.
pub fn remove_blank(input: Input) -> Output {
    let mut out = String::with_capacity(input.text.len());
    for line in lines(&input.text) {
        if !blank(&line) {
            out.push_str(line.body);
            out.push_str(line.ending);
        }
    }
    Output::replace(out)
}

/// Hard Wrap at 80: re-flows each paragraph to 80 columns, keeping the
/// comment marker, indentation and list bullets it started with.
pub fn wrap(input: Input) -> Output {
    Output::replace(wrap::fill(&input.text, &input.language, Some(80)))
}

/// Unwrap Paragraphs: each paragraph becomes one line.
pub fn unwrap(input: Input) -> Output {
    Output::replace(wrap::fill(&input.text, &input.language, None))
}

/// Word Count: words, characters, lines and reading time, in the status
/// line. Changes nothing.
pub fn word_count(input: Input) -> Output {
    let words = input
        .text
        .split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count();
    let characters = input.text.chars().count();
    let line_count = lines(&input.text).len();
    let read = if words < 200 {
        "under 1 min read".to_owned()
    } else {
        format!("about {} min read", words.div_ceil(200))
    };
    let scope = if input.selection { "selection: " } else { "" };
    Output::message(format!(
        "{scope}{} {}, {} {}, {} {}, {read}",
        thousands(words),
        plural(words, "word"),
        thousands(characters),
        plural(characters, "character"),
        thousands(line_count),
        plural(line_count, "line"),
    ))
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

/// 1234567 as 1,234,567.
pub fn thousands(n: usize) -> String {
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

crc_extension::commands! {
    "align_equals" => align_equals,
    "align_colon" => align_colon,
    "join" => join,
    "number" => number,
    "reverse" => reverse,
    "natural_sort" => natural_sort,
    "sort_by_length" => sort_by_length,
    "trim_trailing" => trim_trailing,
    "squeeze_blank" => squeeze_blank,
    "remove_blank" => remove_blank,
    "wrap" => wrap,
    "unwrap" => unwrap,
    "word_count" => word_count,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(f: fn(Input) -> Output, text: &str) -> String {
        run_in(f, text, "text")
    }

    fn run_in(f: fn(Input) -> Output, text: &str, language: &str) -> String {
        f(Input {
            text: text.into(),
            language: language.into(),
            ..Input::default()
        })
        .replace
        .unwrap()
    }

    #[test]
    fn aligns_equals_in_groups_with_compound_tokens() {
        let text = "let a = 1;\nlet total += 2;\nlet x == y;\n\nz = 3;\n    nested = 4;\n";
        assert_eq!(
            run(align_equals, text),
            "let a     = 1;\nlet total += 2;\nlet x     == y;\n\nz = 3;\n    nested = 4;\n"
        );
    }

    #[test]
    fn aligns_arrows_and_lines_without_the_delimiter_break_the_group() {
        let text = "a => 1,\nlonger => 2,\nno delimiter\nb => 3,\nbbbb => 4,";
        assert_eq!(
            run(align_equals, text),
            "a      => 1,\nlonger => 2,\nno delimiter\nb    => 3,\nbbbb => 4,"
        );
    }

    #[test]
    fn aligns_colons_after_the_key_and_skips_double_colons() {
        let text = "name: crc\nversion:   0.1\nurl: https://example.com\nstd::mem::take\n";
        assert_eq!(
            run(align_colon, text),
            "name:    crc\nversion: 0.1\nurl:     https://example.com\nstd::mem::take\n"
        );
        assert_eq!(run(align_colon, "a:\nlonger: 1\n"), "a:\nlonger: 1\n");
    }

    #[test]
    fn keeps_crlf_endings() {
        assert_eq!(run(number, "a\r\nb\r\n"), "1 a\r\n2 b\r\n");
        assert_eq!(run(trim_trailing, "a  \r\nb\t\r\n"), "a\r\nb\r\n");
        assert_eq!(
            run(align_equals, "a = 1\r\nbb = 2\r\n"),
            "a  = 1\r\nbb = 2\r\n"
        );
    }

    #[test]
    fn joins_paragraphs_separately() {
        assert_eq!(run(join, "a\n  b\n\nc\nd"), "a b\n\nc d");
        assert_eq!(run(join, "a\nb\n"), "a b\n");
    }

    #[test]
    fn numbers_right_aligned() {
        let text = (1..=10).map(|_| "x\n").collect::<String>();
        let out = run(number, &text);
        assert!(out.starts_with(" 1 x\n 2 x\n"));
        assert!(out.ends_with("10 x\n"));
        assert_eq!(run(number, "a\n\nb"), "1 a\n2\n3 b");
    }

    #[test]
    fn reverse_keeps_the_missing_final_newline_at_the_end() {
        assert_eq!(run(reverse, "a\nb\nc"), "c\nb\na");
        assert_eq!(run(reverse, "a\nb\n"), "b\na\n");
    }

    #[test]
    fn natural_sort_orders_numbers_by_value_without_regard_to_case() {
        let text = "file10\nFile2\nfile1\nfile02\nb\nA\n";
        assert_eq!(
            run(natural_sort, text),
            "A\nb\nfile1\nFile2\nfile02\nfile10\n"
        );
        assert_eq!(natural_cmp("v1.2.10", "v1.2.9"), Ordering::Greater);
        assert_eq!(natural_cmp("", "a"), Ordering::Less);
    }

    #[test]
    fn sorts_by_length_stably() {
        assert_eq!(run(sort_by_length, "ccc\nb\naa\nd\n"), "b\nd\naa\nccc\n");
    }

    #[test]
    fn squeezes_and_removes_blank_lines() {
        assert_eq!(run(squeeze_blank, "a\n\n  \n\nb\n\n"), "a\n\nb\n\n");
        assert_eq!(run(remove_blank, "a\n\n  \nb\n"), "a\nb\n");
    }

    #[test]
    fn wraps_prose_and_comments() {
        let text = "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen\n";
        assert_eq!(
            run(wrap, text),
            "one two three four five six seven eight nine ten eleven twelve thirteen fourteen\nfifteen sixteen\n"
        );
        let comment = "    // alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi\n    // rho\n    let code = 1;\n";
        assert_eq!(
            run_in(wrap, comment, "rust"),
            "    // alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi\n    // omicron pi rho\n    let code = 1;\n"
        );
    }

    #[test]
    fn wraps_list_items_with_a_hanging_indent() {
        let text = "- alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho\n- short\n";
        assert_eq!(
            run(wrap, text),
            "- alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi\n  omicron pi rho\n- short\n"
        );
    }

    #[test]
    fn leaves_fenced_code_tables_and_headings_alone() {
        let long = "x ".repeat(50);
        let text = format!("# {long}\n```\n{long}\n```\n| {long}|\n");
        assert_eq!(run(wrap, &text), text);
        assert_eq!(run(unwrap, &text), text);
        let unbreakable = format!("{}\n", "a".repeat(100));
        assert_eq!(run(wrap, &unbreakable), unbreakable);
    }

    #[test]
    fn unwrap_undoes_wrap() {
        let text = "// alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau\n\n> quoted words that are\n> wrapped\n";
        let wrapped = run_in(wrap, text, "go");
        assert_eq!(wrapped.lines().count(), 4);
        assert_eq!(
            run_in(unwrap, &wrapped, "go"),
            "// alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau\n\n> quoted words that are wrapped\n"
        );
    }

    #[test]
    fn counts_words_in_the_status_line() {
        let text = "one two, three\n\nfour\n";
        let out = word_count(Input {
            text: text.into(),
            ..Input::default()
        });
        assert_eq!(out.replace, None);
        assert_eq!(
            out.message.as_deref(),
            Some("4 words, 21 characters, 3 lines, under 1 min read")
        );
        let long = word_count(Input {
            text: "word ".repeat(1234),
            selection: true,
            ..Input::default()
        });
        assert_eq!(
            long.message.as_deref(),
            Some("selection: 1,234 words, 6,170 characters, 1 line, about 7 min read")
        );
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(1_000_000), "1,000,000");
    }

    /// Five megabytes through the slowest commands, well inside crc's
    /// budget. Run with `cargo test -p lines --release -- --ignored`.
    #[test]
    #[ignore]
    fn five_megabytes_stay_fast() {
        let mut text = String::new();
        for i in 0..120_000 {
            text.push_str(&format!(
                "    // item{} = value {} alpha beta gamma\n",
                i % 977,
                i
            ));
        }
        assert!(text.len() > 5_000_000);
        for f in [
            natural_sort as fn(Input) -> Output,
            sort_by_length,
            align_equals,
            wrap,
            unwrap,
            join,
        ] {
            let start = std::time::Instant::now();
            let out = f(Input {
                text: text.clone(),
                language: "rust".into(),
                ..Input::default()
            });
            assert!(out.replace.is_some());
            assert!(start.elapsed().as_millis() < 700, "{:?}", start.elapsed());
        }
    }

    #[test]
    fn empty_input_is_fine_everywhere() {
        for f in [
            align_equals as fn(Input) -> Output,
            align_colon,
            join,
            number,
            reverse,
            natural_sort,
            sort_by_length,
            trim_trailing,
            squeeze_blank,
            remove_blank,
            wrap,
            unwrap,
        ] {
            assert_eq!(run(f, ""), "");
        }
        let out = word_count(Input::default());
        assert_eq!(
            out.message.as_deref(),
            Some("0 words, 0 characters, 0 lines, under 1 min read")
        );
    }
}
