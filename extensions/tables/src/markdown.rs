//! Markdown pipe tables: finding them in text, reading their cells and
//! writing them back aligned.

use crate::width::width;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// No mark in the separator row: rendered left, written as `---`.
    #[default]
    None,
    Left,
    Center,
    Right,
}

/// A table found in the lines of a text: the rows without the separator,
/// where it was, and each column's alignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// Index of the first line and one past the last.
    pub lines: std::ops::Range<usize>,
    pub header: Vec<String>,
    pub body: Vec<Vec<String>>,
    pub aligns: Vec<Align>,
}

/// Splits a row at unescaped pipes, dropping the outer empty cells that
/// leading and trailing pipes leave, and trimming each cell.
pub fn cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            chars.next();
            current.push_str("\\|");
        } else if c == '|' {
            cells.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
    }
    cells.push(current);
    let trimmed = line.trim();
    if trimmed.starts_with('|') && !cells.is_empty() {
        cells.remove(0);
    }
    if trimmed.ends_with('|') && !trimmed.ends_with("\\|") && !cells.is_empty() {
        cells.pop();
    }
    cells.into_iter().map(|c| c.trim().to_owned()).collect()
}

fn has_pipe(line: &str) -> bool {
    let mut prev = ' ';
    for c in line.chars() {
        if c == '|' && prev != '\\' {
            return true;
        }
        prev = c;
    }
    false
}

/// `---`, `:--`, `:-:` or `--:` in every cell.
pub fn separator(line: &str) -> Option<Vec<Align>> {
    if !has_pipe(line) {
        return None;
    }
    let cells = cells(line);
    if cells.is_empty() {
        return None;
    }
    let mut aligns = Vec::with_capacity(cells.len());
    for cell in cells {
        let left = cell.starts_with(':');
        let right = cell.ends_with(':');
        let dashes = cell.trim_matches(':');
        if dashes.is_empty() || !dashes.chars().all(|c| c == '-') {
            return None;
        }
        aligns.push(match (left, right) {
            (true, true) => Align::Center,
            (false, true) => Align::Right,
            (true, false) => Align::Left,
            (false, false) => Align::None,
        });
    }
    Some(aligns)
}

/// Every table in the lines: a header line with a pipe, a separator line
/// under it, then body lines with pipes until a line without one.
pub fn find(lines: &[&str]) -> Vec<Table> {
    let mut tables = Vec::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        let (Some(first), Some(second)) = (lines.get(i), lines.get(i + 1)) else {
            break;
        };
        let Some(aligns) = (has_pipe(first) && separator(first).is_none())
            .then(|| separator(second))
            .flatten()
        else {
            i += 1;
            continue;
        };
        let header = cells(first);
        let mut body = Vec::new();
        let mut end = i + 2;
        while let Some(line) = lines.get(end) {
            if !has_pipe(line) || line.trim().is_empty() {
                break;
            }
            body.push(cells(line));
            end += 1;
        }
        tables.push(Table {
            lines: i..end,
            header,
            body,
            aligns,
        });
        i = end;
    }
    tables
}

pub fn pad(cell: &str, target: usize, align: Align) -> String {
    let free = target.saturating_sub(width(cell));
    let (left, right) = match align {
        Align::None | Align::Left => (0, free),
        Align::Right => (free, 0),
        Align::Center => (free / 2, free - free / 2),
    };
    let mut out = String::with_capacity(cell.len() + free);
    out.extend(std::iter::repeat_n(' ', left));
    out.push_str(cell);
    out.extend(std::iter::repeat_n(' ', right));
    out
}

/// Writes the rows as an aligned pipe table: `header`, the separator, then
/// `body`. Rows shorter than the widest get empty cells.
pub fn render(header: &[String], body: &[Vec<String>], aligns: &[Align]) -> Vec<String> {
    let columns = body
        .iter()
        .map(Vec::len)
        .chain([header.len(), aligns.len()])
        .max()
        .unwrap_or(0);
    let empty = String::new();
    let mut widths = vec![3usize; columns];
    for row in std::iter::once(header).chain(body.iter().map(Vec::as_slice)) {
        for (i, cell) in row.iter().enumerate() {
            if let Some(w) = widths.get_mut(i) {
                *w = (*w).max(width(cell));
            }
        }
    }
    let align_of = |i: usize| aligns.get(i).copied().unwrap_or_default();
    let mut out = Vec::with_capacity(body.len() + 2);
    let row_line = |row: &[String]| {
        let cells: Vec<String> = widths
            .iter()
            .enumerate()
            .map(|(i, &w)| pad(row.get(i).unwrap_or(&empty), w, align_of(i)))
            .collect();
        format!("| {} |", cells.join(" | "))
    };
    out.push(row_line(header));
    let separator: Vec<String> = widths
        .iter()
        .enumerate()
        .map(|(i, &w)| {
            let dashes = |n: usize| "-".repeat(n);
            match align_of(i) {
                Align::None => dashes(w),
                Align::Left => format!(":{}", dashes(w.saturating_sub(1))),
                Align::Right => format!("{}:", dashes(w.saturating_sub(1))),
                Align::Center => format!(":{}:", dashes(w.saturating_sub(2))),
            }
        })
        .collect();
    out.push(format!("| {} |", separator.join(" | ")));
    for row in body {
        out.push(row_line(row));
    }
    out
}

/// A cell's text for a pipe table: pipes escaped, line breaks flattened.
pub fn escape(text: &str) -> String {
    text.replace('|', "\\|")
        .replace("\r\n", " ")
        .replace(['\n', '\r'], " ")
}

/// A pipe table cell back to plain text.
pub fn unescape(text: &str) -> String {
    text.replace("\\|", "|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_cells_with_and_without_outer_pipes() {
        assert_eq!(cells("| a | b |"), ["a", "b"]);
        assert_eq!(cells("a|b"), ["a", "b"]);
        assert_eq!(cells("| a \\| b | c |"), ["a \\| b", "c"]);
        assert_eq!(cells("| a | b \\|"), ["a", "b \\|"]);
        assert_eq!(cells("|  |  |"), ["", ""]);
    }

    #[test]
    fn reads_separators() {
        assert_eq!(
            separator("|:--|:-:|--:|---|"),
            Some(vec![Align::Left, Align::Center, Align::Right, Align::None])
        );
        assert_eq!(separator("| a | b |"), None);
        assert_eq!(separator("|::|"), None);
        assert_eq!(separator("no pipes"), None);
    }

    #[test]
    fn finds_tables_between_other_lines() {
        let lines = [
            "intro", "a | b", "-|-", "1 | 2", "", "x", "|c|", "|-|", "|3|",
        ];
        let tables = find(&lines);
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].lines, 1..4);
        assert_eq!(tables[0].header, ["a", "b"]);
        assert_eq!(tables[0].body, [["1", "2"]]);
        assert_eq!(tables[1].lines, 6..9);
    }

    #[test]
    fn renders_aligned() {
        let out = render(
            &["Name".into(), "Qty".into(), "Note".into()],
            &[
                vec!["Pear".into(), "10".into()],
                vec!["Kiwi".into(), "3".into(), "日本".into()],
            ],
            &[Align::None, Align::Right, Align::Center],
        );
        assert_eq!(
            out,
            [
                "| Name | Qty | Note |",
                "| ---- | --: | :--: |",
                "| Pear |  10 |      |",
                "| Kiwi |   3 | 日本 |",
            ]
        );
    }
}
