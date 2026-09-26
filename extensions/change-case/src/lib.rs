//! Change Case: upper, lower and title case for prose, and the identifier
//! cases (snake_case, kebab-case, camelCase, PascalCase, CONSTANT_CASE) for
//! names, line by line so a column of names converts together.

use crc_extension::{Input, Output};

/// UPPER CASE.
pub fn upper(input: Input) -> Output {
    Output::replace(input.text.to_uppercase())
}

/// lower case.
pub fn lower(input: Input) -> Output {
    Output::replace(input.text.to_lowercase())
}

/// Title Case: the first letter of each word up, the rest down.
pub fn title(input: Input) -> Output {
    let mut out = String::with_capacity(input.text.len());
    let mut start = true;
    for c in input.text.chars() {
        if c.is_alphanumeric() || c == '\'' {
            if start {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            start = false;
        } else {
            out.push(c);
            start = true;
        }
    }
    Output::replace(out)
}

/// snake_case.
pub fn snake(input: Input) -> Output {
    per_line(&input.text, |w| join(w, "_", Case::Lower))
}

/// kebab-case.
pub fn kebab(input: Input) -> Output {
    per_line(&input.text, |w| join(w, "-", Case::Lower))
}

/// CONSTANT_CASE.
pub fn constant(input: Input) -> Output {
    per_line(&input.text, |w| join(w, "_", Case::Upper))
}

/// camelCase.
pub fn camel(input: Input) -> Output {
    per_line(&input.text, |w| {
        let mut out = String::new();
        for (i, word) in w.iter().enumerate() {
            if i == 0 {
                out.push_str(&word.to_lowercase());
            } else {
                out.push_str(&capitalized(word));
            }
        }
        out
    })
}

/// PascalCase.
pub fn pascal(input: Input) -> Output {
    per_line(&input.text, |w| {
        w.iter().map(|word| capitalized(word)).collect()
    })
}

enum Case {
    Lower,
    Upper,
}

fn join(words: &[String], separator: &str, case: Case) -> String {
    words
        .iter()
        .map(|w| match case {
            Case::Lower => w.to_lowercase(),
            Case::Upper => w.to_uppercase(),
        })
        .collect::<Vec<_>>()
        .join(separator)
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first
            .to_uppercase()
            .chain(chars.flat_map(char::to_lowercase))
            .collect(),
        None => String::new(),
    }
}

/// Converts each line's words, keeping its indentation and line ending, so
/// blank lines and the text's shape survive.
fn per_line(text: &str, convert: impl Fn(&[String]) -> String) -> Output {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (body, ending) = match line.strip_suffix('\n') {
            Some(body) => (
                body.strip_suffix('\r').unwrap_or(body),
                &line[body.trim_end_matches('\r').len()..],
            ),
            None => (line, ""),
        };
        let indent = &body[..body.len() - body.trim_start().len()];
        let words = words(body.trim());
        out.push_str(indent);
        if !words.is_empty() {
            out.push_str(&convert(&words));
        }
        out.push_str(ending);
    }
    Output::replace(out)
}

/// The words of a name or phrase: split at anything that is not a letter
/// or digit, and where the case changes: `parseHTTPResponse` is parse, HTTP,
/// Response; `snake_case` and `kebab-case` split at their separators.
pub fn words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut current = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            continue;
        }
        if let Some(&prev) = current.chars().last().as_ref() {
            let next = chars.get(i + 1).copied();
            // aB: a new word at the capital. ABc: the last capital of a run
            // starts the next word (HTTPResponse is HTTP, Response).
            let boundary = (prev.is_lowercase() && c.is_uppercase())
                || (prev.is_uppercase()
                    && c.is_uppercase()
                    && next.is_some_and(|n| n.is_lowercase()))
                || (prev.is_ascii_digit() && c.is_alphabetic() && c.is_uppercase());
            if boundary {
                words.push(std::mem::take(&mut current));
            }
        }
        current.push(c);
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

crc_extension::commands! {
    "upper" => upper,
    "lower" => lower,
    "title" => title,
    "snake" => snake,
    "kebab" => kebab,
    "camel" => camel,
    "pascal" => pascal,
    "constant" => constant,
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
    fn splits_names_the_way_people_mean_them() {
        assert_eq!(words("parseHTTPResponse"), ["parse", "HTTP", "Response"]);
        assert_eq!(words("snake_case_name"), ["snake", "case", "name"]);
        assert_eq!(words("kebab-case name"), ["kebab", "case", "name"]);
        assert_eq!(words("PascalCase2D"), ["Pascal", "Case2", "D"]);
        assert_eq!(words("  "), Vec::<String>::new());
    }

    #[test]
    fn converts_each_line_and_keeps_its_shape() {
        let text = "user id\n    parseHTTPResponse\n\nmax_retry_count\r\n";
        assert_eq!(
            run(snake, text),
            "user_id\n    parse_http_response\n\nmax_retry_count\r\n"
        );
        assert_eq!(run(kebab, "user id"), "user-id");
        assert_eq!(run(camel, "max_retry_count"), "maxRetryCount");
        assert_eq!(run(pascal, "max-retry-count"), "MaxRetryCount");
        assert_eq!(run(constant, "maxRetryCount"), "MAX_RETRY_COUNT");
    }

    #[test]
    fn prose_cases() {
        assert_eq!(run(upper, "straße"), "STRASSE");
        assert_eq!(run(lower, "ÉCOLE"), "école");
        assert_eq!(
            run(title, "the QUICK brown fox's tail"),
            "The Quick Brown Fox's Tail"
        );
    }
}
