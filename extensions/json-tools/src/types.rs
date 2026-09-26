//! From a JSON sample to type declarations: infer the shape, then print it
//! as TypeScript, Rust, Go or Python.

use crate::json::{Value, is_integer};

/// The inferred type of a value, merged across every place it appears.
#[derive(Debug, Clone, PartialEq)]
pub struct Ty {
    kind: Kind,
    /// Seen as `null` at least once.
    nullable: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    /// Nothing seen yet (only nulls, or an empty array's items).
    Unknown,
    Bool,
    Int,
    Float,
    Str,
    Array(Box<Ty>),
    Object(Vec<Field>),
    /// Different kinds in the same place.
    Mixed,
}

#[derive(Debug, Clone, PartialEq)]
struct Field {
    key: String,
    ty: Ty,
    /// Missing from at least one object of this shape.
    optional: bool,
}

fn ty(kind: Kind) -> Ty {
    Ty {
        kind,
        nullable: false,
    }
}

/// Infers the type of one value.
pub fn infer(value: &Value) -> Ty {
    match value {
        Value::Null => Ty {
            kind: Kind::Unknown,
            nullable: true,
        },
        Value::Bool(_) => ty(Kind::Bool),
        Value::Number(n) => ty(if is_integer(n) {
            Kind::Int
        } else {
            Kind::Float
        }),
        Value::String(_) => ty(Kind::Str),
        Value::Array(items) => {
            let mut item = ty(Kind::Unknown);
            for v in items {
                item = merge(item, infer(v));
            }
            ty(Kind::Array(Box::new(item)))
        }
        Value::Object(members) => ty(Kind::Object(
            members
                .iter()
                .map(|(k, v)| Field {
                    key: k.clone(),
                    ty: infer(v),
                    optional: false,
                })
                .collect(),
        )),
    }
}

fn merge(a: Ty, b: Ty) -> Ty {
    let nullable = a.nullable || b.nullable;
    let kind = match (a.kind, b.kind) {
        (Kind::Unknown, k) | (k, Kind::Unknown) => k,
        (Kind::Int, Kind::Float) | (Kind::Float, Kind::Int) => Kind::Float,
        (Kind::Array(x), Kind::Array(y)) => Kind::Array(Box::new(merge(*x, *y))),
        (Kind::Object(x), Kind::Object(y)) => Kind::Object(merge_fields(x, y)),
        (x, y) if x == y => x,
        _ => Kind::Mixed,
    };
    Ty { kind, nullable }
}

fn merge_fields(a: Vec<Field>, b: Vec<Field>) -> Vec<Field> {
    let mut out: Vec<Field> = Vec::with_capacity(a.len().max(b.len()));
    for field in a {
        out.push(field);
    }
    let mut seen_in_b = vec![false; out.len()];
    for field in b {
        match out.iter().position(|f| f.key == field.key) {
            Some(i) => {
                if let (Some(existing), Some(seen)) = (out.get_mut(i), seen_in_b.get_mut(i)) {
                    *seen = true;
                    let optional = existing.optional || field.optional;
                    let merged = merge(existing.ty.clone(), field.ty);
                    existing.ty = merged;
                    existing.optional = optional;
                }
            }
            None => {
                out.push(Field {
                    optional: true,
                    ..field
                });
                seen_in_b.push(true);
            }
        }
    }
    for (field, seen) in out.iter_mut().zip(seen_in_b) {
        if !seen {
            field.optional = true;
        }
    }
    out
}

/// The languages types can be printed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    TypeScript,
    Rust,
    Go,
    Python,
}

impl Target {
    /// Picks a target from crc's language name; anything unknown gets
    /// TypeScript.
    pub fn from_language(language: &str) -> Target {
        match language {
            "rust" => Target::Rust,
            "go" => Target::Go,
            "python" => Target::Python,
            _ => Target::TypeScript,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Target::TypeScript => "TypeScript",
            Target::Rust => "Rust",
            Target::Go => "Go",
            Target::Python => "Python",
        }
    }
}

/// Collects every object type under the root into declarations, nested
/// ones first, and rewrites object kinds into references by name.
struct Collector {
    refs: Vec<RefDecl>,
    names: Vec<String>,
}

/// A type with object kinds replaced by the name of their declaration.
#[derive(Debug, Clone)]
enum Ref {
    Unknown,
    Bool,
    Int,
    Float,
    Str,
    Array(Box<RefTy>),
    Named(String),
    Mixed,
}

#[derive(Debug, Clone)]
struct RefTy {
    kind: Ref,
    nullable: bool,
}

struct RefField {
    key: String,
    ty: RefTy,
    optional: bool,
}

struct RefDecl {
    name: String,
    fields: Vec<RefField>,
}

impl Collector {
    fn unique(&mut self, wanted: &str) -> String {
        let base = if wanted.is_empty() {
            "Item".to_owned()
        } else {
            wanted.to_owned()
        };
        let mut name = base.clone();
        let mut n = 2;
        while self.names.contains(&name) {
            name = format!("{base}{n}");
            n += 1;
        }
        self.names.push(name.clone());
        name
    }

    /// Declares `t` and everything under it; the name is a hint from the
    /// key it was found under.
    fn collect(&mut self, t: &Ty, hint: &str) -> RefTy {
        let kind = match &t.kind {
            Kind::Unknown => Ref::Unknown,
            Kind::Bool => Ref::Bool,
            Kind::Int => Ref::Int,
            Kind::Float => Ref::Float,
            Kind::Str => Ref::Str,
            Kind::Mixed => Ref::Mixed,
            Kind::Array(item) => Ref::Array(Box::new(self.collect(item, &singular(hint)))),
            Kind::Object(fields) => {
                let name = self.unique(&pascal(hint));
                let mut ref_fields = Vec::with_capacity(fields.len());
                for f in fields {
                    ref_fields.push(RefField {
                        key: f.key.clone(),
                        ty: self.collect(&f.ty, &f.key),
                        optional: f.optional,
                    });
                }
                self.refs.push(RefDecl {
                    name: name.clone(),
                    fields: ref_fields,
                });
                Ref::Named(name)
            }
        };
        RefTy {
            kind,
            nullable: t.nullable,
        }
    }
}

struct Printer {
    target: Target,
}

/// Prints types for `value` in `target`.
pub fn generate(value: &Value, target: Target) -> String {
    let root = infer(value);
    let mut collector = Collector {
        refs: Vec::new(),
        names: Vec::new(),
    };
    // An object root is `Root` itself; anything else gets `Root` as an
    // alias, with its item type called `Item`.
    let hint = if matches!(root.kind, Kind::Object(_)) {
        "Root"
    } else {
        "Item"
    };
    let root_ref = collector.collect(&root, hint);
    let printer = Printer { target };
    let mut out = String::new();
    let mut needs_any = false;
    let mut needs_optional = false;
    let mut body = String::new();
    for decl in &collector.refs {
        if !body.is_empty() {
            body.push('\n');
        }
        printer.decl(decl, &mut body, &mut needs_any, &mut needs_optional);
    }
    // A root that is not an object gets an alias so the result has a name.
    if !matches!(root_ref.kind, Ref::Named(_)) {
        if !body.is_empty() {
            body.push('\n');
        }
        let alias = printer.type_name(&root_ref, &mut needs_any, &mut needs_optional);
        match target {
            Target::TypeScript => body.push_str(&format!("export type Root = {alias};\n")),
            Target::Rust => body.push_str(&format!("pub type Root = {alias};\n")),
            Target::Go => body.push_str(&format!("type Root {alias}\n")),
            Target::Python => body.push_str(&format!("Root = {alias}\n")),
        }
    }
    if target == Target::Python {
        out.push_str("from dataclasses import dataclass\n");
        let mut typing = Vec::new();
        if needs_any {
            typing.push("Any");
        }
        if needs_optional {
            typing.push("Optional");
        }
        if !typing.is_empty() {
            out.push_str(&format!("from typing import {}\n", typing.join(", ")));
        }
        out.push_str("\n\n");
    }
    out.push_str(&body);
    out
}

impl Printer {
    fn decl(&self, decl: &RefDecl, out: &mut String, any: &mut bool, optional: &mut bool) {
        match self.target {
            Target::TypeScript => {
                out.push_str(&format!("export interface {} {{\n", decl.name));
                for f in &decl.fields {
                    let key = if is_identifier(&f.key) {
                        f.key.clone()
                    } else {
                        let mut quoted = String::new();
                        crate::json::write_string(&f.key, &mut quoted);
                        quoted
                    };
                    let mark = if f.optional { "?" } else { "" };
                    let mut t = self.type_name(&f.ty, any, optional);
                    if f.ty.nullable {
                        t.push_str(" | null");
                    }
                    out.push_str(&format!("  {key}{mark}: {t};\n"));
                }
                out.push_str("}\n");
            }
            Target::Rust => {
                out.push_str("#[derive(Debug, Clone, PartialEq)]\n");
                out.push_str(&format!("pub struct {} {{\n", decl.name));
                let mut used: Vec<String> = Vec::new();
                for f in &decl.fields {
                    let mut name = rust_field(&f.key);
                    while used.contains(&name) {
                        name.push('_');
                    }
                    used.push(name.clone());
                    let plain = name.strip_prefix("r#").unwrap_or(&name);
                    if plain != f.key {
                        out.push_str(&format!("    // key: \"{}\"\n", f.key));
                    }
                    let mut t = self.type_name(&f.ty, any, optional);
                    if f.ty.nullable || f.optional {
                        t = format!("Option<{t}>");
                    }
                    let note = match innermost(&f.ty) {
                        Ref::Mixed => " // mixed types",
                        Ref::Unknown => " // empty in the sample",
                        _ => "",
                    };
                    out.push_str(&format!("    pub {name}: {t},{note}\n"));
                }
                out.push_str("}\n");
            }
            Target::Go => {
                out.push_str(&format!("type {} struct {{\n", decl.name));
                let mut used: Vec<String> = Vec::new();
                let mut rows: Vec<(String, String, String)> = Vec::new();
                for f in &decl.fields {
                    let mut name = go_name(&f.key);
                    if name.is_empty() {
                        name = "Field".to_owned();
                    }
                    while used.contains(&name) {
                        name.push('_');
                    }
                    used.push(name.clone());
                    let mut t = self.type_name(&f.ty, any, optional);
                    if f.ty.nullable && !matches!(f.ty.kind, Ref::Mixed | Ref::Unknown) {
                        t = format!("*{t}");
                    }
                    let omit = if f.optional { ",omitempty" } else { "" };
                    let tag = format!("`json:\"{}{omit}\"`", f.key.replace('"', "\\\""));
                    rows.push((name, t, tag));
                }
                let name_width = rows.iter().map(|r| r.0.len()).max().unwrap_or(0);
                let type_width = rows.iter().map(|r| r.1.len()).max().unwrap_or(0);
                for (name, t, tag) in rows {
                    out.push_str(&format!("\t{name:<name_width$} {t:<type_width$} {tag}\n"));
                }
                out.push_str("}\n");
            }
            Target::Python => {
                out.push_str("@dataclass\n");
                out.push_str(&format!("class {}:\n", decl.name));
                if decl.fields.is_empty() {
                    out.push_str("    pass\n");
                    return;
                }
                let mut used: Vec<String> = Vec::new();
                let mut required = Vec::new();
                let mut defaulted = Vec::new();
                for f in &decl.fields {
                    let mut name = python_field(&f.key);
                    while used.contains(&name) {
                        name.push('_');
                    }
                    used.push(name.clone());
                    let mut t = self.type_name(&f.ty, any, optional);
                    let mut line = String::new();
                    if name != f.key {
                        line.push_str(&format!("    # key: \"{}\"\n", f.key));
                    }
                    if f.ty.nullable || f.optional {
                        *optional = true;
                        t = format!("Optional[{t}]");
                        line.push_str(&format!("    {name}: {t} = None\n"));
                        defaulted.push(line);
                    } else {
                        line.push_str(&format!("    {name}: {t}\n"));
                        required.push(line);
                    }
                }
                for line in required.into_iter().chain(defaulted) {
                    out.push_str(&line);
                }
            }
        }
    }

    fn type_name(&self, t: &RefTy, any: &mut bool, optional: &mut bool) -> String {
        match (&t.kind, self.target) {
            (Ref::Named(n), _) => n.clone(),
            (Ref::Bool, Target::TypeScript) => "boolean".into(),
            (Ref::Bool, Target::Rust) => "bool".into(),
            (Ref::Bool, Target::Go) => "bool".into(),
            (Ref::Bool, Target::Python) => "bool".into(),
            (Ref::Int, Target::TypeScript) | (Ref::Float, Target::TypeScript) => "number".into(),
            (Ref::Int, Target::Rust) => "i64".into(),
            (Ref::Int, Target::Go) => "int64".into(),
            (Ref::Int, Target::Python) => "int".into(),
            (Ref::Float, Target::Rust) => "f64".into(),
            (Ref::Float, Target::Go) => "float64".into(),
            (Ref::Float, Target::Python) => "float".into(),
            (Ref::Str, Target::TypeScript) => "string".into(),
            (Ref::Str, Target::Rust) => "String".into(),
            (Ref::Str, Target::Go) => "string".into(),
            (Ref::Str, Target::Python) => "str".into(),
            (Ref::Array(item), target) => {
                let mut inner = self.type_name(item, any, optional);
                match target {
                    Target::TypeScript => {
                        if item.nullable {
                            inner = format!("({inner} | null)");
                        }
                        format!("{inner}[]")
                    }
                    Target::Rust => {
                        if item.nullable {
                            inner = format!("Option<{inner}>");
                        }
                        format!("Vec<{inner}>")
                    }
                    Target::Go => {
                        if item.nullable && !matches!(item.kind, Ref::Mixed | Ref::Unknown) {
                            inner = format!("*{inner}");
                        }
                        format!("[]{inner}")
                    }
                    Target::Python => {
                        if item.nullable {
                            *optional = true;
                            inner = format!("Optional[{inner}]");
                        }
                        format!("list[{inner}]")
                    }
                }
            }
            (Ref::Mixed | Ref::Unknown, Target::TypeScript) => "unknown".into(),
            (Ref::Mixed, Target::Rust) => "String".into(),
            (Ref::Unknown, Target::Rust) => "()".into(),
            (Ref::Mixed | Ref::Unknown, Target::Go) => "any".into(),
            (Ref::Mixed | Ref::Unknown, Target::Python) => {
                *any = true;
                "Any".into()
            }
        }
    }
}

/// The kind under any number of array layers.
fn innermost(t: &RefTy) -> &Ref {
    match &t.kind {
        Ref::Array(item) => innermost(item),
        other => other,
    }
}

/// Go's exported name for a key, with the usual initialisms in capitals:
/// `user_id` is `UserID`, `api_url` is `APIURL`.
fn go_name(key: &str) -> String {
    const INITIALISMS: &[&str] = &[
        "id", "url", "uri", "http", "https", "api", "json", "xml", "html", "css", "sql", "uuid",
        "ip", "tcp", "udp", "dns", "tls", "ssh", "cpu", "ram", "os", "ui", "utf8", "ascii",
    ];
    let mut out = String::new();
    for word in words(key) {
        let clean: String = word.chars().filter(char::is_ascii_alphanumeric).collect();
        let lower = clean.to_ascii_lowercase();
        if INITIALISMS.contains(&lower.as_str()) {
            out.push_str(&lower.to_ascii_uppercase());
        } else {
            let mut chars = clean.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.extend(chars.flat_map(char::to_lowercase));
            }
        }
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, 'N');
    }
    out
}

/// The words of a key, split at separators and case changes.
fn words(text: &str) -> Vec<String> {
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
        if let Some(prev) = current.chars().last() {
            let next = chars.get(i + 1).copied();
            let boundary = (prev.is_lowercase() && c.is_uppercase())
                || (prev.is_uppercase()
                    && c.is_uppercase()
                    && next.is_some_and(|n| n.is_lowercase()))
                || (prev.is_ascii_digit() != c.is_ascii_digit());
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

/// PascalCase, ASCII letters and digits only, never starting with a digit.
fn pascal(text: &str) -> String {
    let mut out = String::new();
    for word in words(text) {
        let mut chars = word.chars().filter(char::is_ascii_alphanumeric);
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.extend(chars.flat_map(char::to_lowercase));
        }
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, 'N');
    }
    out
}

fn snake(text: &str) -> String {
    let joined = words(text)
        .iter()
        .map(|w| {
            w.chars()
                .filter(char::is_ascii_alphanumeric)
                .flat_map(char::to_lowercase)
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if joined.is_empty() {
        "field".to_owned()
    } else if joined.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("n{joined}")
    } else {
        joined
    }
}

const RUST_KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "break",
    "const",
    "continue",
    "crate",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "Self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    "unsafe",
    "use",
    "where",
    "while",
    "gen",
    "try",
    "macro_rules",
    "union",
];

fn rust_field(key: &str) -> String {
    let name = snake(key);
    if matches!(name.as_str(), "self" | "Self" | "super" | "crate") {
        format!("{name}_")
    } else if RUST_KEYWORDS.contains(&name.as_str()) {
        format!("r#{name}")
    } else {
        name
    }
}

const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield", "match", "case",
];

fn python_field(key: &str) -> String {
    // Keep a key that is already a valid name, so `userId` stays `userId`.
    let name = if is_identifier(key) && !key.contains('$') {
        key.to_owned()
    } else {
        snake(key)
    };
    if PYTHON_KEYWORDS.contains(&name.as_str()) {
        format!("{name}_")
    } else {
        name
    }
}

/// An identifier for TypeScript purposes: letters, digits, `_` and `$`,
/// not starting with a digit.
fn is_identifier(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// A naive singular for naming array item types: `users` becomes `user`,
/// `addresses` becomes `address`, `status` stays.
fn singular(key: &str) -> String {
    let lower = key.to_ascii_lowercase();
    if lower.ends_with("ies") && key.len() > 4 {
        let stem = key.get(..key.len() - 3).unwrap_or(key);
        return format!("{stem}y");
    }
    if lower.ends_with("sses")
        || lower.ends_with("xes")
        || lower.ends_with("ches")
        || lower.ends_with("shes")
    {
        return key.get(..key.len() - 2).unwrap_or(key).to_owned();
    }
    if lower.ends_with('s') && !lower.ends_with("ss") && !lower.ends_with("us") && key.len() > 3 {
        return key.get(..key.len() - 1).unwrap_or(key).to_owned();
    }
    key.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;

    const SAMPLE: &str = r#"{
      "id": 1,
      "user_name": "ann",
      "score": 9.5,
      "active": true,
      "tags": ["a", "b"],
      "address": {"city": "Lisbon", "zip": null},
      "items": [{"sku": "x", "qty": 2}, {"sku": "y", "note": "gift"}],
      "empty": [],
      "type": "admin",
      "mixed": [1, "two"]
    }"#;

    fn emit_types(target: Target) -> String {
        generate(&json::parse(SAMPLE).unwrap(), target)
    }

    #[test]
    fn typescript_interfaces() {
        assert_eq!(
            emit_types(Target::TypeScript),
            "export interface Address {\n  city: string;\n  zip: unknown | null;\n}\n\nexport interface Item {\n  sku: string;\n  qty?: number;\n  note?: string;\n}\n\nexport interface Root {\n  id: number;\n  user_name: string;\n  score: number;\n  active: boolean;\n  tags: string[];\n  address: Address;\n  items: Item[];\n  empty: unknown[];\n  type: string;\n  mixed: unknown[];\n}\n"
        );
    }

    #[test]
    fn rust_structs() {
        let out = emit_types(Target::Rust);
        assert!(out.starts_with("#[derive(Debug, Clone, PartialEq)]\npub struct Address {\n    pub city: String,\n    pub zip: Option<()>, // empty in the sample\n}\n"));
        assert!(out.contains("pub struct Item {\n    pub sku: String,\n    pub qty: Option<i64>,\n    pub note: Option<String>,\n}\n"));
        assert!(out.contains("    pub tags: Vec<String>,\n"));
        assert!(out.contains("    pub r#type: String,\n"));
        assert!(out.contains("    pub mixed: Vec<String>, // mixed types\n"));
        assert!(out.ends_with("pub struct Root {\n    pub id: i64,\n    pub user_name: String,\n    pub score: f64,\n    pub active: bool,\n    pub tags: Vec<String>,\n    pub address: Address,\n    pub items: Vec<Item>,\n    pub empty: Vec<()>, // empty in the sample\n    pub r#type: String,\n    pub mixed: Vec<String>, // mixed types\n}\n"));
    }

    #[test]
    fn go_structs() {
        let out = emit_types(Target::Go);
        assert!(out.contains("type Item struct {\n\tSku  string `json:\"sku\"`\n\tQty  int64  `json:\"qty,omitempty\"`\n\tNote string `json:\"note,omitempty\"`\n}\n"));
        assert!(out.contains("\tUserName string   `json:\"user_name\"`\n"));
        assert!(out.contains("\tMixed    []any    `json:\"mixed\"`\n"));
        assert!(out.contains("\tZip  any    `json:\"zip\"`\n"));
    }

    #[test]
    fn python_dataclasses() {
        let out = emit_types(Target::Python);
        assert!(out.starts_with("from dataclasses import dataclass\nfrom typing import Any, Optional\n\n\n@dataclass\nclass Address:\n    city: str\n    zip: Optional[Any] = None\n"));
        assert!(out.contains("@dataclass\nclass Item:\n    sku: str\n    qty: Optional[int] = None\n    note: Optional[str] = None\n"));
        assert!(out.contains("    type: str\n"));
        assert!(out.contains("    mixed: list[Any]\n"));
    }

    #[test]
    fn root_arrays_and_scalars_get_an_alias() {
        let v = json::parse(r#"[{"a": 1}, {"a": 2, "b": null}]"#).unwrap();
        assert_eq!(
            generate(&v, Target::TypeScript),
            "export interface Item {\n  a: number;\n  b?: unknown | null;\n}\n\nexport type Root = Item[];\n"
        );
        let v = json::parse("[1, 2.5]").unwrap();
        assert_eq!(generate(&v, Target::Rust), "pub type Root = Vec<f64>;\n");
        assert_eq!(generate(&v, Target::Go), "type Root []float64\n");
        assert_eq!(
            generate(&v, Target::Python),
            "from dataclasses import dataclass\n\n\nRoot = list[float]\n"
        );
    }

    #[test]
    fn names_are_singular_pascal_and_unique() {
        assert_eq!(singular("users"), "user");
        assert_eq!(singular("addresses"), "address");
        assert_eq!(singular("categories"), "category");
        assert_eq!(singular("status"), "status");
        assert_eq!(singular("bus"), "bus");
        assert_eq!(pascal("user_address"), "UserAddress");
        assert_eq!(pascal("2fa-codes"), "N2FaCodes");
        assert_eq!(snake("userID"), "user_id");
        assert_eq!(snake("Content-Type"), "content_type");
        let v = json::parse(r#"{"item": {"x": 1}, "items": [{"y": 2}]}"#).unwrap();
        let out = generate(&v, Target::TypeScript);
        assert!(out.contains("export interface Item {\n  x: number;\n}"));
        assert!(out.contains("export interface Item2 {\n  y: number;\n}"));
    }
}
