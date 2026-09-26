//! How a language writes a comment, and wrapping lines in it.

/// A language's comment syntax, as far as one line at a time needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// A prefix before each line, `// ` or `# `.
    Line(&'static str),
    /// Each line wrapped, `/* ... */` or `<!-- ... -->`.
    Block(&'static str, &'static str),
    /// No comment at all: plain text, JSON, unknown languages.
    Plain,
}

impl Style {
    /// The style for one of crc's language names.
    pub fn for_language(language: &str) -> Style {
        match language {
            "rust" | "javascript" | "typescript" | "tsx" | "c" | "cpp" | "go" => Style::Line("// "),
            "python" | "toml" | "yaml" | "bash" => Style::Line("# "),
            "css" => Style::Block("/* ", " */"),
            "html" => Style::Block("<!-- ", " -->"),
            _ => Style::Plain,
        }
    }

    /// What the style adds before a line.
    pub fn opening(self) -> &'static str {
        match self {
            Style::Line(prefix) | Style::Block(prefix, _) => prefix,
            Style::Plain => "",
        }
    }

    /// What the style adds after a line.
    pub fn closing(self) -> &'static str {
        match self {
            Style::Block(_, close) => close,
            Style::Line(_) | Style::Plain => "",
        }
    }

    /// Wraps each line in the comment, after `indent`. Block comments are
    /// padded to the widest line so their closings line up. An empty line
    /// in a line comment is the bare prefix without its trailing space.
    pub fn wrap(self, indent: &str, lines: &[String]) -> Vec<String> {
        let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        lines
            .iter()
            .map(|line| {
                let mut out = String::from(indent);
                match self {
                    Style::Line(prefix) => {
                        if line.is_empty() {
                            out.push_str(prefix.trim_end());
                        } else {
                            out.push_str(prefix);
                            out.push_str(line);
                        }
                    }
                    Style::Block(open, close) => {
                        out.push_str(open);
                        out.push_str(line);
                        out.extend(std::iter::repeat_n(' ', width - line.chars().count()));
                        out.push_str(close);
                    }
                    Style::Plain => out.push_str(line),
                }
                out
            })
            .collect()
    }

    /// A line with this comment's marks and the indentation taken off, so
    /// text that is already a comment is not commented twice.
    pub fn strip(self, line: &str) -> &str {
        let body = line.trim();
        match self {
            Style::Line(prefix) => body
                .strip_prefix(prefix.trim_end())
                .map(|rest| rest.strip_prefix(' ').unwrap_or(rest))
                .unwrap_or(body),
            Style::Block(open, close) => {
                let inner = body
                    .strip_prefix(open.trim_end())
                    .and_then(|rest| rest.strip_suffix(close.trim_start()));
                match inner {
                    Some(inner) => inner.trim(),
                    None => body,
                }
            }
            Style::Plain => body,
        }
    }
}
