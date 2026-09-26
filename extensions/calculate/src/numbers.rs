//! Finding the numbers in ordinary text: a column in a table, figures in a
//! sentence, literals in code. Positions are byte offsets into the text.

/// A number found in text.
#[derive(Debug, Clone, PartialEq)]
pub struct Number {
    /// Where it starts, including a leading `-`.
    pub start: usize,
    /// One past its last byte.
    pub end: usize,
    pub negative: bool,
    /// Its value with the sign applied.
    pub value: f64,
    /// Its magnitude, when it is a whole number that fits.
    pub magnitude: Option<i128>,
    /// 10, or 16, 2 and 8 for `0x`, `0b` and `0o` literals.
    pub radix: u32,
    /// True when the digits carry no separator, fraction or prefix.
    pub plain: bool,
}

impl Number {
    /// The whole-number value with its sign, when it fits.
    pub fn int(&self) -> Option<i128> {
        let m = self.magnitude?;
        if self.negative {
            m.checked_neg()
        } else {
            Some(m)
        }
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Every number in `text`, in order. A number starts at a word boundary:
/// `abc123` and `v2` are names, not numbers. Letters right after a number
/// are read as a unit (`10px`, `2x`) and left alone.
pub fn find(text: &str) -> Vec<Number> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if !(b.is_ascii_digit() || b == b'-') {
            i += 1;
            continue;
        }
        if text[..i].chars().next_back().is_some_and(is_word) {
            i += 1;
            continue;
        }
        let start = i;
        let negative = b == b'-';
        if negative {
            if !bytes.get(i + 1).is_some_and(u8::is_ascii_digit) {
                i += 1;
                continue;
            }
            i += 1;
        }
        match read(text, i) {
            Some((end, magnitude, value, radix, plain)) if value.is_finite() => {
                found.push(Number {
                    start,
                    end,
                    negative,
                    value: if negative { -value } else { value },
                    magnitude,
                    radix,
                    plain,
                });
                i = end;
            }
            _ => i += 1,
        }
    }
    found
}

/// Reads the digits at `at` (a digit). Returns the end, the whole value if
/// any, the float value, the radix and whether the digits were plain.
fn read(text: &str, at: usize) -> Option<(usize, Option<i128>, f64, u32, bool)> {
    let bytes = text.as_bytes();
    if bytes.get(at) == Some(&b'0') {
        let radix = match bytes.get(at + 1) {
            Some(b'x' | b'X') => Some(16),
            Some(b'b' | b'B') => Some(2),
            Some(b'o' | b'O') => Some(8),
            _ => None,
        };
        if let Some(radix) = radix {
            let mut i = at + 2;
            let mut value: i128 = 0;
            let mut digits = 0usize;
            while let Some(&b) = bytes.get(i) {
                if b == b'_' && digits > 0 {
                    i += 1;
                    continue;
                }
                let Some(d) = char::from(b).to_digit(radix) else {
                    break;
                };
                value = value
                    .checked_mul(i128::from(radix))?
                    .checked_add(i128::from(d))?;
                digits += 1;
                i += 1;
            }
            if digits == 0 {
                return None;
            }
            // Precision past 2^53 is lost, which is what a float is.
            #[allow(clippy::cast_precision_loss)]
            let f = value as f64;
            return Some((i, Some(value), f, radix, false));
        }
    }
    let mut i = at;
    let mut digits = String::new();
    let mut plain = true;
    while let Some(&b) = bytes.get(i) {
        if b.is_ascii_digit() {
            digits.push(char::from(b));
            i += 1;
        } else if (b == b'_' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit))
            || (b == b',' && thousands_group(bytes, i))
        {
            plain = false;
            i += 1;
        } else {
            break;
        }
    }
    let mut whole = true;
    if bytes.get(i) == Some(&b'.') && bytes.get(i + 1).is_some_and(u8::is_ascii_digit) {
        whole = false;
        plain = false;
        digits.push('.');
        i += 1;
        while let Some(&b) = bytes.get(i) {
            if !b.is_ascii_digit() {
                break;
            }
            digits.push(char::from(b));
            i += 1;
        }
    }
    let magnitude = if whole {
        digits.parse::<i128>().ok()
    } else {
        None
    };
    let value = digits.parse::<f64>().ok()?;
    Some((i, magnitude, value, 10, plain))
}

/// A `,` at `at` is a thousands separator when exactly three digits follow.
fn thousands_group(bytes: &[u8], at: usize) -> bool {
    (1..=3).all(|k| bytes.get(at + k).is_some_and(u8::is_ascii_digit))
        && !bytes.get(at + 4).is_some_and(u8::is_ascii_digit)
}

/// Words that read as hexadecimal without a prefix: only hex digits, at
/// least one letter and one digit, two characters or more. `ff` and `add`
/// do not count; `deadbeef` does not either, on purpose, since `ff` alone
/// is also a word. `1f`, `c0ffee1` and `7a` do.
pub fn bare_hex(text: &str) -> Vec<(usize, usize, i128)> {
    let mut found = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        match (start, is_word(c)) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                start = None;
                let word = &text[s..i];
                let letters = word.bytes().filter(u8::is_ascii_alphabetic).count();
                let digits = word.bytes().filter(u8::is_ascii_digit).count();
                if word.len() >= 2
                    && letters > 0
                    && digits > 0
                    && word.bytes().all(|b| b.is_ascii_hexdigit())
                    && let Ok(value) = i128::from_str_radix(word, 16)
                {
                    found.push((s, i, value));
                }
            }
            _ => {}
        }
    }
    found
}

/// Replaces the given byte ranges, which must be sorted and not overlap.
pub fn splice(text: &str, replacements: &[(usize, usize, String)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (start, end, with) in replacements {
        if *start < at
            || *end > text.len()
            || !text.is_char_boundary(*start)
            || !text.is_char_boundary(*end)
        {
            continue;
        }
        out.push_str(&text[at..*start]);
        out.push_str(with);
        at = *end;
    }
    out.push_str(&text[at..]);
    out
}

/// A number for a message: thousands separated, up to six decimals, no
/// trailing zeros.
pub fn pretty(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    if value.abs() >= 1e21 {
        return format!("{value:e}");
    }
    let text = format!("{:.6}", value.abs());
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    let fraction = fraction.trim_end_matches('0');
    let mut grouped = String::new();
    for (n, c) in whole.chars().enumerate() {
        if n > 0 && (whole.len() - n) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    let sign = if value < 0.0 && (whole != "0" || !fraction.is_empty()) {
        "-"
    } else {
        ""
    };
    if fraction.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{fraction}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(text: &str) -> Vec<f64> {
        find(text).iter().map(|n| n.value).collect()
    }

    #[test]
    fn finds_numbers_and_skips_names() {
        assert_eq!(
            values("Paid $1,200.50 and 45% on 2 items, -3 refunds"),
            [1200.5, 45.0, 2.0, -3.0]
        );
        assert_eq!(values("abc123 v2 item_4 é9"), Vec::<f64>::new());
        assert_eq!(
            values("2x 10px 0x1F 0b101 1_000"),
            [2.0, 10.0, 31.0, 5.0, 1000.0]
        );
        assert_eq!(values("1,2,3 and 1,2345"), [1.0, 2.0, 3.0, 1.0, 2345.0]);
        assert_eq!(values("3-1"), [3.0, 1.0]);
        assert_eq!(values(""), Vec::<f64>::new());
    }

    #[test]
    fn hex_words_are_conservative() {
        let found: Vec<i128> = bare_hex("add ff 1f c0ffee1 0x10 deadbeef 7a")
            .iter()
            .map(|h| h.2)
            .collect();
        assert_eq!(found, [0x1f, 0xc0ffee1, 0x7a]);
    }

    #[test]
    fn pretty_numbers() {
        assert_eq!(pretty(1234567.5), "1,234,567.5");
        assert_eq!(pretty(-0.25), "-0.25");
        assert_eq!(pretty(120.0), "120");
        assert_eq!(pretty(0.1234567), "0.123457");
    }
}
