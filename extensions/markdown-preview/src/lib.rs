//! Markdown Preview: the document as an HTML page for crc's preview pane.
//! pulldown-cmark parses and writes; this adds the page around it and turns
//! raw HTML in the source into text.

mod page;

use crc_extension::{Input, Output};
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd};

/// The whole document, rendered.
pub fn preview(input: Input) -> Output {
    if input.language != "markdown" {
        return Output::message("Markdown Preview works on Markdown files");
    }
    Output::html(page::wrap(&render(&input.text), title(&input.text)))
}

/// The body: CommonMark with tables, task lists, strikethrough and
/// footnotes. Raw HTML becomes text, so what shows is what the Markdown
/// says and nothing the page could be made to do; a block of it shows as
/// code, keeping its lines.
pub fn render(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES;
    let events = Parser::new_ext(markdown, options).map(|event| match event {
        Event::Start(Tag::HtmlBlock) => Event::Html(CowStr::Borrowed("<pre class=\"raw\"><code>")),
        Event::End(TagEnd::HtmlBlock) => Event::Html(CowStr::Borrowed("</code></pre>\n")),
        Event::Html(html) | Event::InlineHtml(html) => Event::Text(html),
        other => other,
    });
    let mut out = String::with_capacity(markdown.len() * 3 / 2);
    pulldown_cmark::html::push_html(&mut out, events);
    out
}

/// The first heading's text, for the page title.
fn title(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

crc_extension::commands! {
    "preview" => preview,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(text: &str) -> Input {
        Input {
            text: text.into(),
            language: "markdown".into(),
            ..Input::default()
        }
    }

    #[test]
    fn renders_commonmark_and_the_github_additions() {
        let body = render(
            "# Title\n\nSome *em* and **strong** and `code`.\n\n\
             | a | b |\n|---|:-:|\n| 1 | 2 |\n\n- [x] done\n- [ ] todo\n\n~~gone~~\n\n\
             Note[^1].\n\n[^1]: The note.\n\n```rust\nfn main() {}\n```\n",
        );
        for want in [
            "<h1>Title</h1>",
            "<em>em</em>",
            "<strong>strong</strong>",
            "<code>code</code>",
            "<table>",
            "<th style=\"text-align: center\">b</th>",
            "<input disabled=\"\" type=\"checkbox\" checked=\"\"/>",
            "<del>gone</del>",
            "footnote-definition",
            "<pre><code class=\"language-rust\">fn main() {}\n</code></pre>",
        ] {
            assert!(body.contains(want), "missing {want:?} in\n{body}");
        }
    }

    #[test]
    fn raw_html_is_text() {
        let body = render("<script>alert(1)</script>\n\nand <b onclick=\"x\">inline</b>\n");
        assert!(!body.contains("<script"), "{body}");
        assert!(!body.contains("<b "), "{body}");
        assert!(
            body.starts_with(
                "<pre class=\"raw\"><code>&lt;script&gt;alert(1)&lt;/script&gt;\n</code></pre>"
            ),
            "{body}"
        );
        assert!(body.contains("&lt;b onclick=\"x\"&gt;inline"), "{body}");
    }

    #[test]
    fn a_whole_page_for_markdown_and_a_message_otherwise() {
        let out = preview(md("# Notes\n\nHi & bye\n"));
        let html = out.html.unwrap();
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<title>Notes</title>"));
        assert!(html.contains("Content-Security-Policy"));
        assert!(html.contains("<p>Hi &amp; bye</p>"));
        assert!(out.replace.is_none());
        let out = preview(Input {
            language: "rust".into(),
            ..md("fn main() {}")
        });
        assert!(out.html.is_none());
        assert!(out.message.unwrap().contains("Markdown"));
    }

    #[test]
    fn titles_are_escaped() {
        let html = preview(md("# a </title><b>\n")).html.unwrap();
        assert!(
            html.contains("<title>a &lt;/title&gt;&lt;b&gt;</title>"),
            "{html}"
        );
    }
}
