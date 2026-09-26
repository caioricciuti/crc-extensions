//! Calculate: evaluate arithmetic in place, sum and describe the numbers in
//! a selection, convert number bases, and convert Unix timestamps to dates
//! and back.

mod expr;
mod numbers;
mod time;

use crc_extension::{Input, Output};
use numbers::{Number, pretty, splice};

/// Evaluate. One line: the expression becomes its result, or `expr = old`
/// becomes `expr = result`. Several lines: every line that is an
/// expression becomes `expr = result`; the rest stay as they are.
pub fn evaluate(input: Input) -> Output {
    let text = &input.text;
    if text.trim().is_empty() {
        return Output::message("Calculate: nothing to evaluate");
    }
    let single = !text.trim_end_matches(['\r', '\n']).contains('\n');
    if single {
        let body = text.trim_end_matches(['\r', '\n']);
        let ending = &text[body.len()..];
        let (expression, assigned) = split_assignment(body);
        let expression = expression.trim();
        return match expr::evaluate(expression) {
            Ok(value) => match value.render() {
                Ok(result) if assigned => {
                    Output::replace(format!("{expression} = {result}{ending}"))
                }
                Ok(result) => Output::replace(format!("{result}{ending}")),
                Err(e) => Output::message(format!("Calculate: {e}")),
            },
            Err(e) => Output::message(format!("Calculate: {e}")),
        };
    }
    let mut out = String::with_capacity(text.len());
    let mut evaluated = 0usize;
    let mut first_error = None;
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        let ending = &line[body.len()..];
        let indent = &body[..body.len() - body.trim_start().len()];
        let content = body.trim();
        let (expression, _) = split_assignment(content);
        let expression = expression.trim();
        if content.is_empty() || expression.is_empty() || expr::is_literal(expression) {
            out.push_str(line);
            continue;
        }
        match expr::evaluate(expression).and_then(expr::Value::render) {
            Ok(result) => {
                evaluated += 1;
                out.push_str(indent);
                out.push_str(expression);
                out.push_str(" = ");
                out.push_str(&result);
                out.push_str(ending);
            }
            Err(e) => {
                if !matches!(
                    e,
                    expr::Error::Unexpected(_)
                        | expr::Error::UnknownName(_)
                        | expr::Error::Incomplete
                ) {
                    first_error.get_or_insert(e);
                }
                out.push_str(line);
            }
        }
    }
    match (evaluated, first_error) {
        (0, Some(e)) => Output::message(format!("Calculate: {e}")),
        (0, None) => Output::message("Calculate: nothing to evaluate"),
        (n, _) => Output::replace(out)
            .with_message(format!("Calculate: evaluated {n} {}", plural(n, "line"))),
    }
}

/// `expr = anything` is the expression and true; otherwise the whole text.
fn split_assignment(text: &str) -> (&str, bool) {
    match text.split_once('=') {
        Some((expression, _)) if !expression.trim().is_empty() => (expression, true),
        _ => (text, false),
    }
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

/// Sum Numbers: the total of every number in the text, in the status line.
pub fn sum(input: Input) -> Output {
    let found = numbers::find(&input.text);
    if found.is_empty() {
        return Output::message("Calculate: no numbers here");
    }
    let total: f64 = found.iter().map(|n| n.value).sum();
    Output::message(format!(
        "sum {} of {} {}",
        pretty(total),
        found.len(),
        plural(found.len(), "number")
    ))
}

/// Number Statistics: count, sum, min, max, mean and median, in the status
/// line.
pub fn statistics(input: Input) -> Output {
    let mut values: Vec<f64> = numbers::find(&input.text).iter().map(|n| n.value).collect();
    if values.is_empty() {
        return Output::message("Calculate: no numbers here");
    }
    values.sort_by(f64::total_cmp);
    let n = values.len();
    let total: f64 = values.iter().sum();
    // Fine for a count: precision would only matter past 2^53 numbers.
    #[allow(clippy::cast_precision_loss)]
    let mean = total / n as f64;
    let median = if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    };
    Output::message(format!(
        "{n} {}: sum {}, min {}, max {}, mean {}, median {}",
        plural(n, "number"),
        pretty(total),
        pretty(values[0]),
        pretty(values[n - 1]),
        pretty(mean),
        pretty(median)
    ))
}

/// To Hexadecimal: every whole number becomes `0x..`.
pub fn to_hex(input: Input) -> Output {
    convert(&input.text, 16, |n, m| format!("{}0x{m:x}", sign(n)))
}

/// To Binary: every whole number becomes `0b..`.
pub fn to_binary(input: Input) -> Output {
    convert(&input.text, 2, |n, m| format!("{}0b{m:b}", sign(n)))
}

fn sign(n: &Number) -> &'static str {
    if n.negative { "-" } else { "" }
}

/// Rewrites every whole number not already in `radix`.
fn convert(text: &str, radix: u32, write: impl Fn(&Number, i128) -> String) -> Output {
    let replacements: Vec<(usize, usize, String)> = numbers::find(text)
        .iter()
        .filter(|n| n.radix != radix)
        .filter_map(|n| n.magnitude.map(|m| (n.start, n.end, write(n, m))))
        .collect();
    finish(text, replacements, "number")
}

/// To Decimal: `0x`, `0b` and `0o` literals, and words that can only be
/// hexadecimal, become plain decimal numbers.
pub fn to_decimal(input: Input) -> Output {
    let text = &input.text;
    let mut replacements: Vec<(usize, usize, String)> = numbers::find(text)
        .iter()
        .filter(|n| n.radix != 10)
        .filter_map(|n| n.int().map(|v| (n.start, n.end, v.to_string())))
        .collect();
    replacements.extend(
        numbers::bare_hex(text)
            .into_iter()
            .map(|(s, e, v)| (s, e, v.to_string())),
    );
    replacements.sort_by_key(|r| r.0);
    replacements.dedup_by_key(|r| r.0);
    finish(text, replacements, "number")
}

fn finish(text: &str, replacements: Vec<(usize, usize, String)>, what: &str) -> Output {
    if replacements.is_empty() {
        return Output::message(format!("Calculate: no {what}s to convert"));
    }
    let n = replacements.len();
    Output::replace(splice(text, &replacements))
        .with_message(format!("Calculate: converted {n} {}", plural(n, what)))
}

/// Unix Timestamp to Date: 9 or 10 digits are seconds, 12 or 13
/// milliseconds, 15 or 16 microseconds. Each becomes an ISO 8601 UTC date.
pub fn timestamp_to_date(input: Input) -> Output {
    let text = &input.text;
    let replacements: Vec<(usize, usize, String)> = numbers::find(text)
        .iter()
        .filter(|n| n.plain && !n.negative && n.radix == 10)
        .filter_map(|n| {
            let value = n.magnitude?;
            let date = match n.end - n.start {
                9 | 10 => time::format_utc(i64::try_from(value).ok()?, None),
                12 | 13 => {
                    let secs = i64::try_from(value / 1000).ok()?;
                    let millis = u64::try_from(value % 1000).ok()?;
                    time::format_utc(secs, Some((millis, 3)))
                }
                15 | 16 => {
                    let secs = i64::try_from(value / 1_000_000).ok()?;
                    let micros = u64::try_from(value % 1_000_000).ok()?;
                    time::format_utc(secs, Some((micros, 6)))
                }
                _ => None,
            }?;
            Some((n.start, n.end, date))
        })
        .collect();
    finish(text, replacements, "timestamp")
}

/// Date to Unix Timestamp: every ISO 8601 date or date-time becomes Unix
/// seconds. A time without an offset is UTC.
pub fn date_to_timestamp(input: Input) -> Output {
    let text = &input.text;
    let bytes = text.as_bytes();
    let mut replacements = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit()
            || text[..i]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            i += 1;
            continue;
        }
        match time::parse_iso(text, i) {
            Some((end, seconds)) => {
                replacements.push((i, end, seconds.to_string()));
                i = end;
            }
            None => i += 1,
        }
    }
    finish(text, replacements, "timestamp")
}

crc_extension::commands! {
    "evaluate" => evaluate,
    "sum" => sum,
    "statistics" => statistics,
    "to_hex" => to_hex,
    "to_binary" => to_binary,
    "to_decimal" => to_decimal,
    "timestamp_to_date" => timestamp_to_date,
    "date_to_timestamp" => date_to_timestamp,
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
        run(f, text)
            .replace
            .unwrap_or_else(|| panic!("no replacement for {text:?}"))
    }

    fn message(f: fn(Input) -> Output, text: &str) -> String {
        run(f, text)
            .message
            .unwrap_or_else(|| panic!("no message for {text:?}"))
    }

    #[test]
    fn one_line_becomes_its_result() {
        assert_eq!(replaced(evaluate, "12 * (3 + 4)"), "84");
        assert_eq!(replaced(evaluate, "12 * (3 + 4)\n"), "84\n");
        assert_eq!(replaced(evaluate, "  2 ^ 10 = 999"), "2 ^ 10 = 1024");
        assert_eq!(message(evaluate, "1 / 0"), "Calculate: division by zero");
        assert_eq!(
            message(evaluate, "hello there"),
            "Calculate: unknown name hello"
        );
    }

    #[test]
    fn many_lines_keep_their_shape() {
        let text = "Budget\n  rent: 1200\n  100 * 12 = 1000\n  sqrt(2)\n42\n\n1/0\n";
        let out = run(evaluate, text);
        assert_eq!(
            out.replace.as_deref(),
            Some("Budget\n  rent: 1200\n  100 * 12 = 1200\n  sqrt(2) = 1.41421356237\n42\n\n1/0\n")
        );
        assert_eq!(out.message.as_deref(), Some("Calculate: evaluated 2 lines"));
        assert_eq!(
            message(evaluate, "a\nb\n"),
            "Calculate: nothing to evaluate"
        );
        assert_eq!(
            message(evaluate, "1/0\n2/0\n"),
            "Calculate: division by zero"
        );
        assert_eq!(message(evaluate, ""), "Calculate: nothing to evaluate");
    }

    #[test]
    fn sums_and_describes() {
        assert_eq!(
            message(sum, "3 apples, 4.5 pears and -1 plum, item_7"),
            "sum 6.5 of 3 numbers"
        );
        assert_eq!(
            message(sum, "| 1,200 |\n| 800 |\n"),
            "sum 2,000 of 2 numbers"
        );
        assert_eq!(message(sum, "no digits"), "Calculate: no numbers here");
        assert_eq!(
            message(statistics, "1 40 8 10 11"),
            "5 numbers: sum 70, min 1, max 40, mean 14, median 10"
        );
        assert_eq!(
            message(statistics, "1 2 3 4"),
            "4 numbers: sum 10, min 1, max 4, mean 2.5, median 2.5"
        );
        assert_eq!(message(statistics, ""), "Calculate: no numbers here");
    }

    #[test]
    fn converts_bases() {
        assert_eq!(
            replaced(to_hex, "255 and -16, 0b11 but 0xff stays"),
            "0xff and -0x10, 0x3 but 0xff stays"
        );
        assert_eq!(replaced(to_binary, "5 0x0a"), "0b101 0b1010");
        assert_eq!(
            replaced(to_decimal, "0xff 0b101 0o17 1f add ff"),
            "255 5 15 31 add ff"
        );
        assert_eq!(
            message(to_decimal, "12 34"),
            "Calculate: no numbers to convert"
        );
        assert_eq!(
            message(to_hex, "3.5 only"),
            "Calculate: no numbers to convert"
        );
        assert_eq!(
            run(to_hex, "1 2").message.as_deref(),
            Some("Calculate: converted 2 numbers")
        );
    }

    #[test]
    fn keeps_line_endings_and_scales() {
        assert_eq!(
            replaced(evaluate, "1 + 1\r\n2 * 2\r\n"),
            "1 + 1 = 2\r\n2 * 2 = 4\r\n"
        );
        assert_eq!(replaced(to_hex, "10\r\n"), "0xa\r\n");
        let big = "total 1,234.5 for order 0x1f at 1727361600 on 2024-09-26\n".repeat(20_000);
        for f in [
            evaluate,
            sum,
            statistics,
            to_hex,
            to_binary,
            to_decimal,
            timestamp_to_date,
            date_to_timestamp,
        ] {
            let out = run(f, &big);
            assert!(out.replace.is_some() || out.message.is_some());
        }
    }

    #[test]
    fn timestamps_both_ways() {
        assert_eq!(
            replaced(
                timestamp_to_date,
                "at 1727361600 and 1700000000123 and 1700000000123456 not 42"
            ),
            "at 2024-09-26T14:40:00Z and 2023-11-14T22:13:20.123Z and 2023-11-14T22:13:20.123456Z not 42"
        );
        assert_eq!(
            replaced(timestamp_to_date, "951782400"),
            "2000-02-29T00:00:00Z"
        );
        assert_eq!(
            message(timestamp_to_date, "id 12345"),
            "Calculate: no timestamps to convert"
        );
        assert_eq!(
            replaced(
                date_to_timestamp,
                "from 2024-09-26 to 2024-09-26T16:40:00+02:00, x2024-01-01"
            ),
            "from 1727308800 to 1727361600, x2024-01-01"
        );
        assert_eq!(
            replaced(date_to_timestamp, "2024-09-26 14:40Z"),
            "1727361600"
        );
        assert_eq!(
            message(date_to_timestamp, "2023-02-29"),
            "Calculate: no timestamps to convert"
        );
    }
}
