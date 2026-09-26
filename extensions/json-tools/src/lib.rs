//! JSON Tools: format, minify and sort JSON, convert between JSON and YAML,
//! and turn a JSON sample into types for the file's language.

mod json;
mod types;
mod yaml;

use crc_extension::{Input, Output};
use json::Value;

/// Parses the input as JSON, or explains where it went wrong.
fn read(input: &Input) -> Result<Value, Output> {
    if input.text.trim().is_empty() {
        return Err(Output::message("JSON Tools: nothing to read"));
    }
    json::parse(&input.text).map_err(|e| Output::message(format!("JSON Tools: {e}")))
}

/// Keeps the input's trailing newline, or its absence, on the result.
fn keep_newline(input: &Input, mut text: String) -> String {
    if input.text.ends_with('\n') && !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// Format JSON: two-space indentation, one member per line.
pub fn format(input: Input) -> Output {
    match read(&input) {
        Ok(value) => Output::replace(keep_newline(&input, json::format(&value))),
        Err(out) => out,
    }
}

/// Minify JSON: no whitespace at all.
pub fn minify(input: Input) -> Output {
    match read(&input) {
        Ok(value) => {
            let before = input.text.len();
            let text = json::minify(&value);
            let saved = before.saturating_sub(text.len());
            Output::replace(keep_newline(&input, text))
                .with_message(format!("minified, {saved} bytes smaller"))
        }
        Err(out) => out,
    }
}

/// Sort JSON Keys: every object's keys in order, at every depth, formatted.
pub fn sort_keys(input: Input) -> Output {
    match read(&input) {
        Ok(mut value) => {
            json::sort_keys(&mut value);
            Output::replace(keep_newline(&input, json::format(&value)))
        }
        Err(out) => out,
    }
}

/// JSON to YAML: block style.
pub fn to_yaml(input: Input) -> Output {
    match read(&input) {
        Ok(value) => Output::replace(yaml::emit(&value)),
        Err(out) => out,
    }
}

/// YAML to JSON: formatted JSON from one YAML document.
pub fn from_yaml(input: Input) -> Output {
    if input.text.trim().is_empty() {
        return Output::message("JSON Tools: nothing to read");
    }
    match yaml::parse(&input.text) {
        Ok(value) => {
            let mut text = json::format(&value);
            text.push('\n');
            Output::replace(text)
        }
        Err(e) => Output::message(format!("JSON Tools: YAML {e}")),
    }
}

/// JSON to Types: declarations in the document's language.
pub fn to_types(input: Input) -> Output {
    match read(&input) {
        Ok(value) => {
            let target = types::Target::from_language(&input.language);
            Output::replace(types::generate(&value, target))
                .with_message(format!("{} types from the JSON sample", target.label()))
        }
        Err(out) => out,
    }
}

crc_extension::commands! {
    "format" => format,
    "minify" => minify,
    "sort_keys" => sort_keys,
    "to_yaml" => to_yaml,
    "from_yaml" => from_yaml,
    "to_types" => to_types,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(text: &str, language: &str) -> Input {
        Input {
            text: text.into(),
            selection: true,
            language: language.into(),
            command: String::new(),
        }
    }

    fn run(f: fn(Input) -> Output, text: &str) -> Output {
        f(input(text, "text"))
    }

    #[test]
    fn formats_and_keeps_the_trailing_newline() {
        let out = run(format, "{\"b\":[1,2],\"a\":{}}\n");
        assert_eq!(
            out.replace.as_deref(),
            Some("{\n  \"b\": [\n    1,\n    2\n  ],\n  \"a\": {}\n}\n")
        );
        let out = run(format, "[]");
        assert_eq!(out.replace.as_deref(), Some("[]"));
    }

    #[test]
    fn minifies_and_says_how_much() {
        let out = run(minify, "{\n  \"a\": [ 1, 2 ]\n}\n");
        assert_eq!(out.replace.as_deref(), Some("{\"a\":[1,2]}\n"));
        assert_eq!(out.message.as_deref(), Some("minified, 9 bytes smaller"));
    }

    #[test]
    fn sorts_keys_recursively() {
        let out = run(sort_keys, r#"{"b":{"z":1,"y":2},"a":[{"d":1,"c":2}]}"#);
        assert_eq!(
            out.replace.as_deref(),
            Some(
                "{\n  \"a\": [\n    {\n      \"c\": 2,\n      \"d\": 1\n    }\n  ],\n  \"b\": {\n    \"y\": 2,\n    \"z\": 1\n  }\n}"
            )
        );
    }

    #[test]
    fn bad_json_is_a_message_not_a_change() {
        let out = run(format, "{\n  \"a\": 1,\n  \"b\": \n}");
        assert_eq!(out.replace, None);
        assert_eq!(
            out.message.as_deref(),
            Some("JSON Tools: line 4, column 1: unexpected '}'")
        );
        let out = run(minify, "   \n");
        assert_eq!(out.message.as_deref(), Some("JSON Tools: nothing to read"));
        let out = run(to_types, "[1, 2");
        assert_eq!(
            out.message.as_deref(),
            Some("JSON Tools: line 1, column 6: expected ',' or ']', found the end")
        );
    }

    #[test]
    fn json_to_yaml_and_back() {
        let text = r#"{"name":"crc","list":[{"a":1,"b":[true,null]},"x"],"note":"a: b","n":"12"}"#;
        let yaml = run(to_yaml, text).replace.unwrap();
        assert_eq!(
            yaml,
            "name: crc\nlist:\n  - a: 1\n    b:\n      - true\n      - null\n  - x\nnote: \"a: b\"\nn: \"12\"\n"
        );
        let back = run(from_yaml, &yaml).replace.unwrap();
        assert_eq!(run(minify, &back).replace.unwrap(), format!("{text}\n"));
    }

    #[test]
    fn yaml_errors_carry_the_line() {
        let out = run(from_yaml, "a: 1\nb: *ref\n");
        assert_eq!(
            out.message.as_deref(),
            Some("JSON Tools: YAML line 2: anchors, aliases and tags are not supported")
        );
        assert_eq!(out.replace, None);
    }

    #[test]
    fn types_follow_the_language() {
        let sample = r#"{"id": 1, "tags": ["a"], "owner": {"name": "x", "age": null}}"#;
        let ts = to_types(input(sample, "typescript")).replace.unwrap();
        assert!(
            ts.contains("export interface Owner {\n  name: string;\n  age: unknown | null;\n}")
        );
        assert!(ts.contains(
            "export interface Root {\n  id: number;\n  tags: string[];\n  owner: Owner;\n}"
        ));
        let rs = to_types(input(sample, "rust"));
        assert!(rs.replace.as_deref().unwrap().contains("pub struct Owner {\n    pub name: String,\n    pub age: Option<()>, // empty in the sample\n}"));
        assert_eq!(
            rs.message.as_deref(),
            Some("Rust types from the JSON sample")
        );
        let go = to_types(input(sample, "go")).replace.unwrap();
        assert!(go.contains("type Root struct {\n\tID    int64    `json:\"id\"`\n\tTags  []string `json:\"tags\"`\n\tOwner Owner    `json:\"owner\"`\n}"));
        let py = to_types(input(sample, "python")).replace.unwrap();
        assert!(py.contains(
            "@dataclass\nclass Root:\n    id: int\n    tags: list[str]\n    owner: Owner\n"
        ));
        // Unknown languages get TypeScript.
        let other = to_types(input(sample, "text")).replace.unwrap();
        assert_eq!(other, ts);
    }

    #[test]
    fn optional_fields_merge_across_array_items() {
        let sample = r#"[{"a": 1, "b": "x"}, {"a": 2.5}, {"a": 3, "b": null, "c": [1]}]"#;
        let ts = to_types(input(sample, "typescript")).replace.unwrap();
        assert_eq!(
            ts,
            "export interface Item {\n  a: number;\n  b?: string | null;\n  c?: number[];\n}\n\nexport type Root = Item[];\n"
        );
    }

    #[test]
    fn large_input_is_fast() {
        let mut text = String::from("[");
        for i in 0..50_000 {
            if i > 0 {
                text.push(',');
            }
            text.push_str(&format!(
                r#"{{"id":{i},"name":"user {i}","tags":["a","b"],"ok":true}}"#
            ));
        }
        text.push(']');
        let start = std::time::Instant::now();
        let formatted = run(format, &text).replace.unwrap();
        let _ = run(to_yaml, &formatted).replace.unwrap();
        let _ = run(to_types, &text).replace.unwrap();
        let _ = run(sort_keys, &text).replace.unwrap();
        assert!(start.elapsed().as_secs() < 5, "took {:?}", start.elapsed());
    }
}
