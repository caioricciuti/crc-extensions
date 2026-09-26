//! Reading and writing delimiter-separated text the RFC 4180 way: quoted
//! fields, doubled quotes inside them, newlines inside quotes, CRLF. The
//! delimiter is guessed from the first line: a tab if there is one outside
//! quotes, a semicolon if there are more of them than commas, else a comma.

/// One cell, and whether it was written in quotes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Field {
    pub value: String,
    pub quoted: bool,
}

/// A parsed table. Ragged rows are padded with empty fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Csv {
    pub rows: Vec<Vec<Field>>,
    pub delimiter: char,
    pub columns: usize,
}

impl Csv {
    /// The value at a cell, or an empty string.
    pub fn cell(&self, row: usize, column: usize) -> &str {
        self.rows
            .get(row)
            .and_then(|r| r.get(column))
            .map_or("", |f| f.value.as_str())
    }

    /// Whether every non-empty cell below the header in `column` is a
    /// number, and at least one is.
    pub fn numeric_column(&self, column: usize) -> bool {
        numeric_column(
            self.rows
                .iter()
                .skip(1)
                .map(|r| r.get(column).map_or("", |f| f.value.as_str())),
        )
    }
}

pub fn numeric_column<'a>(cells: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = false;
    for cell in cells {
        let cell = cell.trim();
        if cell.is_empty() {
            continue;
        }
        if !is_number(cell) {
            return false;
        }
        seen = true;
    }
    seen
}

/// A number as people write them in tables: optional sign, digits with
/// optional thousands separators and a decimal part, an optional percent.
pub fn is_number(text: &str) -> bool {
    let text = text.strip_suffix('%').unwrap_or(text);
    let text = text
        .strip_prefix('-')
        .or_else(|| text.strip_prefix('+'))
        .unwrap_or(text);
    if text.is_empty() {
        return false;
    }
    let mut digits = 0;
    for c in text.chars() {
        match c {
            '0'..='9' => digits += 1,
            ',' | '_' | '.' | 'e' | 'E' | '-' | '+' => {}
            _ => return false,
        }
    }
    digits > 0 && text.replace([',', '_'], "").parse::<f64>().is_ok()
}

/// Splits the text into leading blank lines, the body, and everything after
/// the body's last non-blank character (usually the final newline). The
/// two ends go back around the converted body untouched.
pub fn frame(text: &str) -> (&str, &str, &str) {
    let mut start = 0;
    for line in text.split_inclusive('\n') {
        if line.trim().is_empty() {
            start += line.len();
        } else {
            break;
        }
    }
    let end = text.trim_end().len().max(start);
    (&text[..start], &text[start..end], &text[end..])
}

/// The line ending the text uses.
pub fn line_ending(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

pub fn detect_delimiter(text: &str) -> char {
    let first = text.lines().next().unwrap_or("");
    let (mut tabs, mut semicolons, mut commas) = (0, 0, 0);
    let mut in_quotes = false;
    for c in first.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            '\t' if !in_quotes => tabs += 1,
            ';' if !in_quotes => semicolons += 1,
            ',' if !in_quotes => commas += 1,
            _ => {}
        }
    }
    if tabs > 0 {
        '\t'
    } else if semicolons > commas {
        ';'
    } else {
        ','
    }
}

/// Parses the body (see [`frame`]). Returns `None` when there is nothing.
pub fn parse(body: &str) -> Option<Csv> {
    if body.trim().is_empty() {
        return None;
    }
    let delimiter = detect_delimiter(body);
    let mut rows: Vec<Vec<Field>> = Vec::new();
    let mut row: Vec<Field> = Vec::new();
    let mut field = Field::default();
    let mut chars = body.chars().peekable();
    let mut in_quotes = false;
    let mut field_started = false;
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.value.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.value.push(c);
            }
            continue;
        }
        match c {
            '"' if !field_started => {
                in_quotes = true;
                field.quoted = true;
                field_started = true;
            }
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                field_started = false;
            }
            c if c == delimiter => {
                row.push(std::mem::take(&mut field));
                field_started = false;
            }
            c => {
                field.value.push(c);
                field_started = true;
            }
        }
    }
    row.push(field);
    rows.push(row);
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    for row in &mut rows {
        row.resize(columns, Field::default());
    }
    Some(Csv {
        rows,
        delimiter,
        columns,
    })
}

/// Writes a field, quoting it when the delimiter, a quote, a line break or
/// an outer space would otherwise be misread.
pub fn quote(value: &str, delimiter: char, force: bool) -> String {
    let needs = force
        || value.contains(delimiter)
        || value.contains('"')
        || value.contains('\n')
        || value.contains('\r')
        || value.starts_with(' ')
        || value.ends_with(' ');
    if needs {
        let mut out = String::with_capacity(value.len() + 2);
        out.push('"');
        for c in value.chars() {
            if c == '"' {
                out.push('"');
            }
            out.push(c);
        }
        out.push('"');
        out
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(csv: &Csv) -> Vec<Vec<&str>> {
        csv.rows
            .iter()
            .map(|r| r.iter().map(|f| f.value.as_str()).collect())
            .collect()
    }

    #[test]
    fn reads_quotes_commas_and_newlines_inside_fields() {
        let csv = parse("name,note\n\"Smith, J\",\"said \"\"hi\"\"\nthen left\"\nplain,").unwrap();
        assert_eq!(csv.delimiter, ',');
        assert_eq!(
            values(&csv),
            [
                vec!["name", "note"],
                vec!["Smith, J", "said \"hi\"\nthen left"],
                vec!["plain", ""],
            ]
        );
        assert!(csv.rows[1][0].quoted);
        assert!(!csv.rows[2][0].quoted);
    }

    #[test]
    fn guesses_the_delimiter_from_the_first_line() {
        assert_eq!(detect_delimiter("a\tb,c"), '\t');
        assert_eq!(detect_delimiter("a;b;c,d"), ';');
        assert_eq!(detect_delimiter("a,b;c"), ',');
        assert_eq!(detect_delimiter("\"a;b\",c"), ',');
        let csv = parse("a;b\r\n1;2\r\n").unwrap();
        assert_eq!(values(&csv), [vec!["a", "b"], vec!["1", "2"], vec!["", ""]]);
    }

    #[test]
    fn pads_ragged_rows() {
        let csv = parse("a,b,c\n1\n").unwrap();
        assert_eq!(csv.columns, 3);
        assert_eq!(csv.cell(1, 2), "");
    }

    #[test]
    fn frames_blank_lines_around_the_body() {
        assert_eq!(frame("\n\na,b\n1,2\n\n"), ("\n\n", "a,b\n1,2", "\n\n"));
        assert_eq!(frame("a,b"), ("", "a,b", ""));
        assert_eq!(frame("  \n"), ("  \n", "", ""));
    }

    #[test]
    fn recognises_numbers() {
        for n in ["1", "-2.5", "1,000", "3e5", "12%", "+7"] {
            assert!(is_number(n), "{n}");
        }
        for n in ["", "-", "1.2.3", "abc", "1a", "1-2"] {
            assert!(!is_number(n), "{n}");
        }
    }

    #[test]
    fn quotes_only_when_needed() {
        assert_eq!(quote("plain", ',', false), "plain");
        assert_eq!(quote("a,b", ',', false), "\"a,b\"");
        assert_eq!(quote("say \"hi\"", ',', false), "\"say \"\"hi\"\"\"");
        assert_eq!(quote(" x", ',', false), "\" x\"");
        assert_eq!(quote("x", ',', true), "\"x\"");
    }
}
