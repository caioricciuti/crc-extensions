//! Tables: format Markdown tables, and turn CSV or TSV into Markdown
//! tables, box-drawn tables, aligned columns or JSON.

mod csv;
mod markdown;
mod width;

use crc_extension::{Input, Output};
use csv::{Csv, frame, line_ending};
use markdown::{Align, pad};
use width::width;

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// Splits into lines without their endings; a trailing newline gives a
/// final empty line, so joining with the ending restores it.
fn lines_of(text: &str) -> Vec<&str> {
    text.split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect()
}

/// Replaces every table's lines with what `convert` makes of it.
fn rewrite_tables(
    text: &str,
    convert: impl Fn(&markdown::Table) -> Vec<String>,
) -> Option<(String, usize)> {
    let ending = line_ending(text);
    let lines = lines_of(text);
    let tables = markdown::find(&lines);
    if tables.is_empty() {
        return None;
    }
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut at = 0;
    for table in &tables {
        for line in lines.get(at..table.lines.start).unwrap_or(&[]) {
            out.push((*line).to_owned());
        }
        out.extend(convert(table));
        at = table.lines.end;
    }
    for line in lines.get(at..).unwrap_or(&[]) {
        out.push((*line).to_owned());
    }
    Some((out.join(ending), tables.len()))
}

/// Format Markdown Table: aligns every pipe table's columns, honouring the
/// separator row's alignment marks.
pub fn format_markdown(input: Input) -> Output {
    match rewrite_tables(&input.text, |t| {
        markdown::render(&t.header, &t.body, &t.aligns)
    }) {
        Some((text, n)) => {
            Output::replace(text).with_message(format!("formatted {}", plural(n, "table")))
        }
        None => Output::message("Tables: no Markdown table here"),
    }
}

/// Markdown Table to CSV: each table becomes comma-separated lines.
pub fn markdown_to_csv(input: Input) -> Output {
    match rewrite_tables(&input.text, |t| {
        std::iter::once(&t.header)
            .chain(t.body.iter())
            .map(|row| {
                row.iter()
                    .map(|cell| csv::quote(&markdown::unescape(cell), ',', false))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect()
    }) {
        Some((text, n)) => {
            Output::replace(text).with_message(format!("converted {}", plural(n, "table")))
        }
        None => Output::message("Tables: no Markdown table here"),
    }
}

/// Parses the text as CSV and hands the table to `convert`, keeping blank
/// lines around it.
fn from_csv(text: &str, convert: impl Fn(&Csv) -> Vec<String>) -> Output {
    let (before, body, after) = frame(text);
    let Some(csv) = csv::parse(body) else {
        return Output::message("Tables: nothing to convert");
    };
    let lines = convert(&csv);
    let mut out = String::with_capacity(text.len() * 2);
    out.push_str(before);
    out.push_str(&lines.join(line_ending(text)));
    out.push_str(after);
    Output::replace(out).with_message(format!(
        "{}, {}",
        plural(csv.rows.len(), "row"),
        plural(csv.columns, "column")
    ))
}

fn csv_aligns(csv: &Csv) -> Vec<Align> {
    (0..csv.columns)
        .map(|c| {
            if csv.numeric_column(c) {
                Align::Right
            } else {
                Align::None
            }
        })
        .collect()
}

fn escaped_rows(csv: &Csv) -> Vec<Vec<String>> {
    csv.rows
        .iter()
        .map(|row| row.iter().map(|f| markdown::escape(&f.value)).collect())
        .collect()
}

/// CSV to Markdown Table: the first row is the header; numeric columns are
/// right-aligned.
pub fn csv_to_markdown(input: Input) -> Output {
    from_csv(&input.text, |csv| {
        let rows = escaped_rows(csv);
        let (header, body) = rows
            .split_first()
            .map_or((&[][..], &[][..]), |(h, b)| (h.as_slice(), b));
        markdown::render(header, body, &csv_aligns(csv))
    })
}

fn column_widths(csv: &Csv, cell: impl Fn(&csv::Field) -> String) -> Vec<usize> {
    let mut widths = vec![0usize; csv.columns];
    for row in &csv.rows {
        for (i, field) in row.iter().enumerate() {
            if let Some(w) = widths.get_mut(i) {
                *w = (*w).max(width(&cell(field)));
            }
        }
    }
    widths
}

/// CSV to Box Table: Unicode box drawing with the header set apart.
pub fn csv_to_box(input: Input) -> Output {
    from_csv(&input.text, |csv| {
        let flat = |f: &csv::Field| f.value.replace("\r\n", " ").replace(['\n', '\r'], " ");
        let widths = column_widths(csv, flat);
        let aligns = csv_aligns(csv);
        let rule = |left: char, mid: char, right: char| {
            let bars: Vec<String> = widths.iter().map(|&w| "─".repeat(w + 2)).collect();
            format!("{left}{}{right}", bars.join(&mid.to_string()))
        };
        let row_line = |row: &[csv::Field]| {
            let cells: Vec<String> = widths
                .iter()
                .enumerate()
                .map(|(i, &w)| {
                    let text = row.get(i).map(flat).unwrap_or_default();
                    let align = aligns.get(i).copied().unwrap_or_default();
                    format!(" {} ", pad(&text, w, align))
                })
                .collect();
            format!("│{}│", cells.join("│"))
        };
        let mut out = Vec::with_capacity(csv.rows.len() + 4);
        out.push(rule('┌', '┬', '┐'));
        let mut rows = csv.rows.iter();
        if let Some(header) = rows.next() {
            out.push(row_line(header));
            if csv.rows.len() > 1 {
                out.push(rule('├', '┼', '┤'));
            }
        }
        for row in rows {
            out.push(row_line(row));
        }
        out.push(rule('└', '┴', '┘'));
        out
    })
}

/// Align CSV Columns: pads fields so the delimiters line up, keeping the
/// text valid CSV with the same delimiter.
pub fn align_csv(input: Input) -> Output {
    from_csv(&input.text, |csv| {
        let written = |f: &csv::Field| csv::quote(&f.value, csv.delimiter, f.quoted);
        let widths = column_widths(csv, written);
        let aligns = csv_aligns(csv);
        let separator = if csv.delimiter == '\t' {
            "\t".to_owned()
        } else {
            format!("{} ", csv.delimiter)
        };
        csv.rows
            .iter()
            .map(|row| {
                let last = widths.len().saturating_sub(1);
                let cells: Vec<String> = widths
                    .iter()
                    .enumerate()
                    .map(|(i, &w)| {
                        let text = row.get(i).map(written).unwrap_or_default();
                        let align = aligns.get(i).copied().unwrap_or_default();
                        if i == last && matches!(align, Align::None | Align::Left) {
                            text
                        } else {
                            pad(&text, w, align)
                        }
                    })
                    .collect();
                cells.join(&separator)
            })
            .collect()
    })
}

fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
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
    out
}

/// A JSON number literal: no leading zeros, no thousands separators.
fn is_json_number(text: &str) -> bool {
    let rest = text.strip_prefix('-').unwrap_or(text);
    let (int, rest) = match rest.find(['.', 'e', 'E']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let int_ok = int == "0"
        || (!int.is_empty() && !int.starts_with('0') && int.bytes().all(|b| b.is_ascii_digit()));
    if !int_ok {
        return false;
    }
    let (frac, exp) = match rest.strip_prefix('.') {
        Some(after) => match after.find(['e', 'E']) {
            Some(i) => (Some(&after[..i]), &after[i..]),
            None => (Some(after), ""),
        },
        None => (None, rest),
    };
    if frac.is_some_and(|f| f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit())) {
        return false;
    }
    match exp.strip_prefix(['e', 'E']) {
        Some(digits) => {
            let digits = digits.strip_prefix(['+', '-']).unwrap_or(digits);
            !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
        }
        None => exp.is_empty(),
    }
}

fn json_value(text: &str) -> String {
    match text {
        "" => "null".to_owned(),
        "true" | "false" => text.to_owned(),
        t if is_json_number(t) => t.to_owned(),
        t => json_string(t),
    }
}

/// CSV to JSON: an array of objects keyed by the header row. Numbers and
/// booleans are typed; empty cells are null.
pub fn csv_to_json(input: Input) -> Output {
    from_csv(&input.text, |csv| {
        let keys: Vec<String> = (0..csv.columns)
            .map(|i| {
                let name = csv.cell(0, i).trim();
                if name.is_empty() {
                    format!("column_{}", i + 1)
                } else {
                    name.to_owned()
                }
            })
            .collect();
        let mut out = vec!["[".to_owned()];
        let count = csv.rows.len().saturating_sub(1);
        for (n, row) in csv.rows.iter().skip(1).enumerate() {
            out.push("  {".to_owned());
            for (i, key) in keys.iter().enumerate() {
                let value = row.get(i).map_or("", |f| f.value.as_str());
                let comma = if i + 1 < keys.len() { "," } else { "" };
                out.push(format!(
                    "    {}: {}{comma}",
                    json_string(key),
                    json_value(value)
                ));
            }
            out.push(if n + 1 < count {
                "  },".to_owned()
            } else {
                "  }".to_owned()
            });
        }
        out.push("]".to_owned());
        out
    })
}

crc_extension::commands! {
    "format_markdown" => format_markdown,
    "csv_to_markdown" => csv_to_markdown,
    "markdown_to_csv" => markdown_to_csv,
    "csv_to_box" => csv_to_box,
    "align_csv" => align_csv,
    "csv_to_json" => csv_to_json,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(f: fn(Input) -> Output, text: &str) -> Output {
        f(Input {
            text: text.into(),
            ..Input::default()
        })
    }

    fn replaced(f: fn(Input) -> Output, text: &str) -> String {
        run(f, text).replace.expect("a replacement")
    }

    #[test]
    fn formats_a_messy_table_and_keeps_the_text_around_it() {
        let text =
            "Fruit list\n\nname|qty|  note\n:--|--:|:-:\nPear | 10 | ripe\n|Kiwi|3|\n\nDone.\n";
        assert_eq!(
            replaced(format_markdown, text),
            "Fruit list\n\n| name | qty | note |\n| :--- | --: | :--: |\n| Pear |  10 | ripe |\n| Kiwi |   3 |      |\n\nDone.\n"
        );
    }

    #[test]
    fn keeps_escaped_pipes_as_one_cell() {
        let text = "| a | b |\n|---|---|\n| x \\| y | z |\n";
        assert_eq!(
            replaced(format_markdown, text),
            "| a      | b   |\n| ------ | --- |\n| x \\| y | z   |\n"
        );
    }

    #[test]
    fn formats_every_table_and_counts_them() {
        let text = "|a|\n|-|\n|1|\n\n|b|\n|-|\n|22|\n";
        let out = run(format_markdown, text);
        assert_eq!(out.message.as_deref(), Some("formatted 2 tables"));
        assert_eq!(
            out.replace.as_deref(),
            Some("| a   |\n| --- |\n| 1   |\n\n| b   |\n| --- |\n| 22  |\n")
        );
    }

    #[test]
    fn says_when_there_is_no_table() {
        let out = run(format_markdown, "just | a | line\nand another\n");
        assert_eq!(out.replace, None);
        assert_eq!(
            out.message.as_deref(),
            Some("Tables: no Markdown table here")
        );
    }

    #[test]
    fn csv_to_markdown_right_aligns_numbers() {
        let text = "name,qty,price\nPear,10,1.5\n\"Kiwi, gold\",3,\n";
        assert_eq!(
            replaced(csv_to_markdown, text),
            "| name       | qty | price |\n| ---------- | --: | ----: |\n| Pear       |  10 |   1.5 |\n| Kiwi, gold |   3 |       |\n"
        );
    }

    #[test]
    fn detects_tabs_and_semicolons() {
        assert_eq!(
            replaced(csv_to_markdown, "a\tb\n1\t2"),
            "|   a |   b |\n| --: | --: |\n|   1 |   2 |"
        );
        assert_eq!(
            replaced(csv_to_markdown, "a;b\nx,y;z"),
            "| a   | b   |\n| --- | --- |\n| x,y | z   |"
        );
    }

    #[test]
    fn markdown_to_csv_quotes_what_needs_it() {
        let text = "| name | note |\n|---|---|\n| Smith, J | said \"hi\" |\n| a \\| b | plain |\n";
        assert_eq!(
            replaced(markdown_to_csv, text),
            "name,note\n\"Smith, J\",\"said \"\"hi\"\"\"\na | b,plain\n"
        );
    }

    #[test]
    fn draws_a_box_table() {
        let text = "Name,Qty\nPear,10\n日本,3\n";
        assert_eq!(
            replaced(csv_to_box, text),
            "┌──────┬─────┐\n│ Name │ Qty │\n├──────┼─────┤\n│ Pear │  10 │\n│ 日本 │   3 │\n└──────┴─────┘\n"
        );
    }

    #[test]
    fn aligns_csv_columns_and_keeps_quotes() {
        let text = "name,qty,note\n\"Pear\",10,\"a, b\"\nKiwi,3,x\n";
        assert_eq!(
            replaced(align_csv, text),
            "name  , qty, note\n\"Pear\",  10, \"a, b\"\nKiwi  ,   3, x\n"
        );
    }

    #[test]
    fn csv_to_json_types_the_values() {
        let text = "id,name,ok,score,\n1,Pear,true,1.5,x\n007,\"say \"\"hi\"\"\",no,,\n";
        assert_eq!(
            replaced(csv_to_json, text),
            "[\n  {\n    \"id\": 1,\n    \"name\": \"Pear\",\n    \"ok\": true,\n    \"score\": 1.5,\n    \"column_5\": \"x\"\n  },\n  {\n    \"id\": \"007\",\n    \"name\": \"say \\\"hi\\\"\",\n    \"ok\": \"no\",\n    \"score\": null,\n    \"column_5\": null\n  }\n]\n"
        );
    }

    #[test]
    fn keeps_blank_lines_and_crlf_around_csv() {
        let out = run(csv_to_markdown, "\r\na,b\r\n1,2\r\n\r\n");
        assert_eq!(
            out.replace.as_deref(),
            Some("\r\n|   a |   b |\r\n| --: | --: |\r\n|   1 |   2 |\r\n\r\n")
        );
        assert_eq!(out.message.as_deref(), Some("2 rows, 2 columns"));
    }

    #[test]
    fn empty_input_is_a_message_not_a_crash() {
        for f in [csv_to_markdown, csv_to_box, align_csv, csv_to_json] {
            let out = run(f, "  \n\n");
            assert_eq!(out.replace, None);
            assert_eq!(out.message.as_deref(), Some("Tables: nothing to convert"));
        }
        assert_eq!(run(format_markdown, "").replace, None);
        assert_eq!(run(markdown_to_csv, "").replace, None);
    }

    #[test]
    fn json_numbers_are_strict() {
        for n in ["0", "-1", "12.5", "1e10", "2E-3"] {
            assert!(is_json_number(n), "{n}");
        }
        for n in ["007", "1.", ".5", "1,000", "+1", "1e", ""] {
            assert!(!is_json_number(n), "{n}");
        }
    }

    #[test]
    fn a_large_document_converts_quickly() {
        let mut text = String::from("id,name,value\n");
        for i in 0..100_000 {
            text.push_str(&format!("{i},item {i},{}\n", i * 3));
        }
        let start = std::time::Instant::now();
        let out = run(csv_to_markdown, &text);
        assert!(out.replace.is_some());
        assert!(start.elapsed().as_secs() < 5, "took {:?}", start.elapsed());
    }
}
