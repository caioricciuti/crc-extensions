//! Banner: turn the selection into big block letters, a boxed comment or a
//! divider, in the file's comment syntax.

mod comment;
mod font;

use comment::Style;
use crc_extension::{Input, Output};

/// Where a divider ends, counting indentation and the comment marks.
const DIVIDER_WIDTH: usize = 80;
/// The narrowest a box's inside gets, so short labels still get a box.
const MIN_BOX_WIDTH: usize = 20;
/// The longest line the banner draws: beyond this it would not fit any
/// screen, and the cost grows with it.
const MAX_BANNER_LINE: usize = 200;

/// The selection taken apart: its lines without endings, the indentation of
/// its first line, and how to put it back together.
struct Text {
    indent: String,
    lines: Vec<String>,
    ending: &'static str,
    trailing_newline: bool,
}

impl Text {
    /// `None` when there is nothing but whitespace to work with.
    fn parse(text: &str) -> Option<Text> {
        if text.trim().is_empty() {
            return None;
        }
        let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let trailing_newline = text.ends_with('\n');
        let body = text.strip_suffix('\n').unwrap_or(text);
        let body = body.strip_suffix('\r').unwrap_or(body);
        let lines: Vec<String> = body
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l).to_owned())
            .collect();
        let first = lines.first().map(String::as_str).unwrap_or("");
        let indent = first[..first.len() - first.trim_start().len()].to_owned();
        Some(Text {
            indent,
            lines,
            ending,
            trailing_newline,
        })
    }

    fn join(&self, lines: Vec<String>) -> String {
        let mut out = lines.join(self.ending);
        if self.trailing_newline {
            out.push_str(self.ending);
        }
        out
    }
}

fn nothing_selected() -> Output {
    Output::message("Banner: select some text first")
}

fn draw_banner(input: &Input, set: char) -> Output {
    let Some(text) = Text::parse(&input.text) else {
        return nothing_selected();
    };
    let style = Style::for_language(&input.language);
    let mut rows: Vec<String> = Vec::new();
    for line in text
        .lines
        .iter()
        .map(|l| style.strip(l))
        .filter(|l| !l.is_empty())
    {
        if line.chars().count() > MAX_BANNER_LINE {
            return Output::message(format!(
                "Banner: a line longer than {MAX_BANNER_LINE} characters would not fit anywhere"
            ));
        }
        if !rows.is_empty() {
            rows.push(String::new());
        }
        rows.extend(font::render(line, set));
    }
    Output::replace(text.join(style.wrap(&text.indent, &rows)))
}

/// ASCII Banner: the selection in five-row block letters drawn with `#`.
pub fn banner(input: Input) -> Output {
    draw_banner(&input, '#')
}

/// Block Banner: the same letters drawn with `█`.
pub fn block_banner(input: Input) -> Output {
    draw_banner(&input, '\u{2588}')
}

/// Comment Box: the selection's lines inside a `+---+` box.
pub fn comment_box(input: Input) -> Output {
    let Some(text) = Text::parse(&input.text) else {
        return nothing_selected();
    };
    let style = Style::for_language(&input.language);
    let lines: Vec<&str> = text.lines.iter().map(|l| style.strip(l)).collect();
    let width = lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0)
        .max(MIN_BOX_WIDTH);
    let border = format!("+{}+", "-".repeat(width + 4));
    let mut rows = Vec::with_capacity(lines.len() + 2);
    rows.push(border.clone());
    for line in lines {
        let pad = " ".repeat(width - line.chars().count());
        rows.push(format!("|  {line}{pad}  |"));
    }
    rows.push(border);
    Output::replace(text.join(style.wrap(&text.indent, &rows)))
}

/// Comment Divider: each line centred in a line of dashes reaching
/// column 80.
pub fn divider(input: Input) -> Output {
    let Some(text) = Text::parse(&input.text) else {
        return nothing_selected();
    };
    let style = Style::for_language(&input.language);
    let marks = text.indent.chars().count()
        + style.opening().chars().count()
        + style.closing().chars().count();
    let total = DIVIDER_WIDTH.saturating_sub(marks);
    let rows: Vec<String> = text
        .lines
        .iter()
        .map(|l| style.strip(l))
        .map(|label| {
            if label.is_empty() {
                return "-".repeat(total);
            }
            let used = label.chars().count() + 2;
            // At least two dashes on each side, or the label is too long
            // for the width and gets the short form.
            if used + 4 > total {
                return format!("-- {label} --");
            }
            let left = (total - used) / 2;
            let right = total - used - left;
            format!("{} {label} {}", "-".repeat(left), "-".repeat(right))
        })
        .collect();
    Output::replace(text.join(style.wrap(&text.indent, &rows)))
}

crc_extension::commands! {
    "banner" => banner,
    "block_banner" => block_banner,
    "comment_box" => comment_box,
    "divider" => divider,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(f: fn(Input) -> Output, text: &str, language: &str) -> String {
        f(Input {
            text: text.into(),
            selection: true,
            language: language.into(),
            command: String::new(),
        })
        .replace
        .unwrap()
    }

    #[test]
    fn every_glyph_is_five_rows_of_one_width() {
        for c in font::CHARACTERS.chars() {
            let g = font::glyph(c).unwrap_or_else(|| panic!("no glyph for {c:?}"));
            assert_eq!(g.len(), font::HEIGHT);
            let width = g[0].len();
            for row in g {
                assert_eq!(row.len(), width, "{c:?} has ragged rows");
                assert!(row.chars().all(|p| p == '#' || p == '.'), "{c:?}");
            }
        }
        assert_eq!(font::glyph('a'), font::glyph('A'));
        assert!(font::glyph('é').is_none());
    }

    #[test]
    fn the_alphabet_renders() {
        for chunk in [
            "ABCDEFGHIJKLM",
            "NOPQRSTUVWXYZ",
            "0123456789",
            "!?.,:;-_+=/()'\"#@&*%",
        ] {
            let rows = font::render(chunk, '#');
            assert_eq!(rows.len(), font::HEIGHT);
            for row in rows {
                println!("{row}");
            }
            println!();
        }
    }

    #[test]
    fn banner_in_rust() {
        assert_eq!(
            run(banner, "crc", "rust"),
            "//  #### ####   ####\n\
             // #     #   # #\n\
             // #     ####  #\n\
             // #     #  #  #\n\
             //  #### #   #  ####"
        );
    }

    #[test]
    fn banner_in_other_languages() {
        assert!(
            run(banner, "hi", "python")
                .lines()
                .all(|l| l.starts_with("# "))
        );
        let css = run(banner, "hi", "css");
        assert!(
            css.lines()
                .all(|l| l.starts_with("/* ") && l.ends_with(" */"))
        );
        let widths: Vec<usize> = css.lines().map(|l| l.chars().count()).collect();
        assert!(widths.iter().all(|w| *w == widths[0]), "closings line up");
        let html = run(banner, "hi", "html");
        assert!(
            html.lines()
                .all(|l| l.starts_with("<!-- ") && l.ends_with(" -->"))
        );
        let plain = run(banner, "hi", "text");
        assert!(plain.starts_with("#   # ###"));
        assert_eq!(run(banner, "hi", "json"), plain);
    }

    #[test]
    fn block_banner_uses_full_blocks() {
        let out = run(block_banner, "I", "rust");
        assert_eq!(out, "// ███\n//  █\n//  █\n//  █\n// ███");
    }

    #[test]
    fn unknown_characters_show_as_blocks_and_spaces_separate_words() {
        let rows = font::render("é", '#');
        assert_eq!(rows[0], "###");
        let rows = font::render("I I", '#');
        assert_eq!(rows[0], "###  ###");
    }

    #[test]
    fn multiple_lines_become_separate_blocks() {
        let out = run(banner, "I\nI\n", "python");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 11);
        assert_eq!(lines[5], "#");
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn comment_box_exact() {
        assert_eq!(
            run(comment_box, "Hello, world", "rust"),
            "// +------------------------+\n\
             // |  Hello, world          |\n\
             // +------------------------+"
        );
        let long = "This label is longer than twenty characters";
        let out = run(comment_box, long, "text");
        assert_eq!(out.lines().next().unwrap().len(), long.len() + 6);
    }

    #[test]
    fn comment_box_does_not_comment_twice() {
        let once = run(comment_box, "note", "rust");
        let twice = run(comment_box, "// note\n// more", "rust");
        assert!(twice.contains("|  note"));
        assert!(!twice.contains("// // "));
        assert!(twice.lines().all(|l| l.starts_with("// ")));
        assert!(once.lines().all(|l| l.starts_with("// ")));
        let css = run(comment_box, "/* note */", "css");
        assert!(css.contains("|  note"));
    }

    #[test]
    fn divider_reaches_column_eighty() {
        for (language, text) in [
            ("rust", "Section"),
            ("python", "Helpers"),
            ("css", "Layout"),
            ("text", "Notes"),
        ] {
            let out = run(divider, text, language);
            assert_eq!(out.chars().count(), 80, "{out}");
            assert!(out.contains(&format!(" {text} ")));
        }
        assert_eq!(
            run(divider, "Section", "rust"),
            "// ---------------------------------- Section ----------------------------------"
        );
        assert_eq!(
            run(divider, "Section\n\n", "rust"),
            format!(
                "// ---------------------------------- Section ----------------------------------\n// {}\n",
                "-".repeat(77)
            )
        );
        let long = "x".repeat(90);
        assert_eq!(run(divider, &long, "rust"), format!("// -- {long} --"));
    }

    #[test]
    fn empty_selection_is_a_message() {
        for f in [banner, block_banner, comment_box, divider] {
            let out = f(Input {
                text: "  \n".into(),
                selection: true,
                ..Input::default()
            });
            assert_eq!(out.replace, None);
            assert_eq!(
                out.message.as_deref(),
                Some("Banner: select some text first")
            );
        }
    }

    #[test]
    fn indentation_is_kept_and_counted() {
        let out = run(banner, "    I", "rust");
        assert!(out.lines().all(|l| l.starts_with("    // ")));
        let out = run(divider, "    Section", "rust");
        assert_eq!(out.chars().count(), 80);
        assert!(out.starts_with("    // "));
        let out = run(comment_box, "\tlabel", "python");
        assert!(out.lines().all(|l| l.starts_with("\t# ")));
    }

    #[test]
    fn windows_line_endings_survive() {
        let out = run(banner, "I\r\nI\r\n", "rust");
        assert!(out.ends_with("\r\n"));
        assert!(!out.contains("\n\n"));
        assert_eq!(out.matches("\r\n").count(), 11);
        let out = run(comment_box, "a\r\nb", "text");
        assert_eq!(out.matches("\r\n").count(), 3);
    }

    #[test]
    fn overlong_banner_line_is_refused() {
        let out = banner(Input {
            text: "x".repeat(201),
            selection: true,
            ..Input::default()
        });
        assert!(out.replace.is_none());
        assert!(out.message.unwrap().contains("200"));
    }
}
