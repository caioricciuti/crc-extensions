//! The page around the rendered body: a stylesheet that follows light and
//! dark, and a Content-Security-Policy that says the same as crc's own
//! rules (no script, nothing from the network), in case the page is ever
//! shown somewhere without them.

const CSP: &str = "default-src 'none'; img-src file: data:; media-src file: data:; \
                   style-src 'unsafe-inline'";

const STYLE: &str = r#"
:root { color-scheme: light dark; --fg: #1f2328; --muted: #59636e; --line: #d1d9e0;
  --soft: #f6f8fa; --link: #0969da; --bg: #ffffff; }
@media (prefers-color-scheme: dark) {
  :root { --fg: #d1d7e0; --muted: #9198a1; --line: #3d444d; --soft: #151b23;
    --link: #4493f8; --bg: #0d1117; }
}
html { background: var(--bg); }
body { color: var(--fg); font: 15px/1.6 -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif;
  max-width: 860px; margin: 0 auto; padding: 24px 32px 64px; word-wrap: break-word; }
h1, h2, h3, h4, h5, h6 { line-height: 1.25; margin: 1.5em 0 0.6em; font-weight: 600; }
h1 { font-size: 2em; padding-bottom: 0.3em; border-bottom: 1px solid var(--line); }
h2 { font-size: 1.5em; padding-bottom: 0.3em; border-bottom: 1px solid var(--line); }
h3 { font-size: 1.25em; } h4 { font-size: 1em; } h5 { font-size: 0.875em; }
h6 { font-size: 0.85em; color: var(--muted); }
body > :first-child { margin-top: 0; }
p, ul, ol, blockquote, pre, table { margin: 0 0 1em; }
a { color: var(--link); text-decoration: none; }
ul, ol { padding-left: 2em; }
li + li { margin-top: 0.25em; }
li:has(> input[type=checkbox]) { list-style: none; margin-left: -1.4em; }
input[type=checkbox] { margin: 0 0.4em 0 0; vertical-align: middle; }
blockquote { padding: 0 1em; color: var(--muted); border-left: 0.25em solid var(--line); }
code, pre { font: 0.875em/1.45 ui-monospace, "SF Mono", Menlo, monospace; }
code { background: var(--soft); padding: 0.2em 0.4em; border-radius: 6px; }
pre { background: var(--soft); padding: 16px; border-radius: 6px; overflow: auto; }
pre code { background: none; padding: 0; font-size: 1em; }
table { border-collapse: collapse; display: block; width: max-content; max-width: 100%; overflow: auto; }
th, td { border: 1px solid var(--line); padding: 6px 13px; }
th { font-weight: 600; }
tr:nth-child(2n) { background: var(--soft); }
img { max-width: 100%; }
hr { border: 0; height: 0.25em; background: var(--line); margin: 1.5em 0; }
del { color: var(--muted); }
.footnote-definition { font-size: 0.875em; color: var(--muted); display: flex; gap: 0.5em; }
.footnote-definition p { margin: 0; }
"#;

/// A whole page around `body`.
pub fn wrap(body: &str, title: Option<String>) -> String {
    let title = escape(title.as_deref().unwrap_or("Preview"));
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{CSP}\">\
         <title>{title}</title><style>{STYLE}</style></head>\n<body>\n{body}</body></html>\n"
    )
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}
