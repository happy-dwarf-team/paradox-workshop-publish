//! The Markdown to Steam BBCode converter for descriptions and change notes
//! (spec section 7).
//!
//! Supported: headings 1 to 3, bold, italic, strikethrough, links, lists,
//! quotes, code, horizontal rules, tables and images with absolute URLs.
//! Raw HTML, footnotes, relative image paths and deeper headings are errors
//! that name the line; HTML comments are dropped. A line break inside a
//! paragraph stays a line break, and paragraphs are separated by a blank
//! line.

use std::borrow::Cow;

use pulldown_cmark::{Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd};

/// A construct the converter rejects, and the line it starts on.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {problem}")]
pub struct ConvertError {
    /// 1-based.
    pub line: usize,
    pub problem: Problem,
}

impl ConvertError {
    /// The same error for Markdown that starts on line `first_line` of a
    /// larger file.
    pub fn starting_at(self, first_line: usize) -> ConvertError {
        ConvertError {
            line: self.line + first_line - 1,
            ..self
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Problem {
    #[error("raw HTML is not supported; use Markdown only")]
    Html,
    #[error("footnotes are not supported")]
    Footnote,
    #[error("the image `{0}` needs an absolute http(s) URL; Steam cannot show repository files")]
    RelativeImage(String),
    #[error("{0} headings are not supported; Steam BBCode has h1 to h3")]
    Heading(HeadingLevel),
}

/// Converts Markdown to Steam BBCode.
pub fn to_bbcode(markdown: &str) -> Result<String, ConvertError> {
    // Line numbers stay the same; code blocks lose the carriage returns.
    let markdown = &markdown.replace("\r\n", "\n");
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_FOOTNOTES;
    let lines = LineIndex::new(markdown);
    let error = |offset, problem| ConvertError {
        line: lines.line(offset),
        problem,
    };
    let mut writer = Writer::default();
    // An HTML block being read: where it starts and its text.
    let mut html: Option<(usize, String)> = None;
    for (event, range) in Parser::new_ext(markdown, options).into_offset_iter() {
        // HTML comments show nothing, so they are dropped; GitHub's generated
        // release notes start with one.
        match (&event, &mut html) {
            (Event::Start(Tag::HtmlBlock), _) => html = Some((range.start, String::new())),
            (Event::Html(text), Some((_, block))) => block.push_str(text),
            (Event::End(TagEnd::HtmlBlock), _) => {
                if let Some((start, block)) = html.take()
                    && !only_comments(&block)
                {
                    return Err(error(start, Problem::Html));
                }
            }
            (Event::InlineHtml(text), _) if only_comments(text) => {}
            _ => writer
                .event(event)
                .map_err(|problem| error(range.start, problem))?,
        }
    }
    Ok(writer.finish())
}

/// HTML that holds nothing but comments and whitespace.
fn only_comments(html: &str) -> bool {
    let mut rest = html.trim_start();
    while !rest.is_empty() {
        let Some(comment) = rest.strip_prefix("<!--") else {
            return false;
        };
        let Some(end) = comment.find("-->") else {
            return false;
        };
        rest = comment[end + 3..].trim_start();
    }
    true
}

/// Maps byte offsets to 1-based line numbers.
pub(crate) struct LineIndex(Vec<usize>);

impl LineIndex {
    pub(crate) fn new(text: &str) -> LineIndex {
        let starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(index, _)| index + 1))
            .collect();
        LineIndex(starts)
    }

    pub(crate) fn line(&self, offset: usize) -> usize {
        self.0.partition_point(|&start| start <= offset)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Container {
    Quote,
    List,
    Item,
    Table,
}

#[derive(Default)]
struct Writer {
    out: String,
    /// Inline text not written yet, so that adjacent text events are
    /// protected by one `[noparse]`.
    text: String,
    containers: Vec<Container>,
    /// A container tag was just written: its first block starts on the same
    /// line, as Steam would show a leading line break.
    fresh: bool,
    /// A block ended: the next one starts after a blank line.
    after_block: bool,
    /// The text of the code block being read.
    code: Option<String>,
    /// Inside an image, whose alt text Steam cannot show.
    image: bool,
    table_head: bool,
}

impl Writer {
    fn event(&mut self, event: Event) -> Result<(), Problem> {
        match event {
            Event::Start(tag) => self.start(tag)?,
            Event::End(tag) => self.end(tag),
            Event::Text(text) | Event::Code(text) => {
                if let Some(code) = &mut self.code {
                    code.push_str(&text);
                } else if !self.image {
                    self.inline(&text);
                }
            }
            // Steam shows a newline as a line break, so a line break inside a
            // paragraph stays one, as GitHub shows it in comments.
            Event::SoftBreak if !self.image => self.inline("\n"),
            Event::SoftBreak => {}
            Event::HardBreak => self.push("\n"),
            Event::Rule => {
                self.open_block();
                self.push("[hr][/hr]");
                self.close_block();
            }
            Event::Html(_) | Event::InlineHtml(_) => return Err(Problem::Html),
            Event::FootnoteReference(_) => return Err(Problem::Footnote),
            // Task lists and math are not enabled in the parser options.
            Event::TaskListMarker(_) | Event::InlineMath(_) | Event::DisplayMath(_) => {
                unreachable!("not enabled")
            }
        }
        Ok(())
    }

    fn start(&mut self, tag: Tag) -> Result<(), Problem> {
        match tag {
            Tag::Paragraph => self.open_block(),
            Tag::Heading { level, .. } => {
                if level > HeadingLevel::H3 {
                    return Err(Problem::Heading(level));
                }
                self.open_block();
                self.push(&format!("[{level}]"));
            }
            Tag::BlockQuote(_) => {
                self.open_block();
                self.push("[quote]");
                self.containers.push(Container::Quote);
                self.fresh = true;
            }
            Tag::CodeBlock(_) => {
                self.open_block();
                self.code = Some(String::new());
            }
            Tag::HtmlBlock => return Err(Problem::Html),
            Tag::List(start) => {
                self.open_block();
                self.push(if start.is_some() {
                    "[olist]\n"
                } else {
                    "[list]\n"
                });
                self.containers.push(Container::List);
            }
            Tag::Item => {
                self.newline();
                self.push("[*]");
                self.containers.push(Container::Item);
                self.fresh = true;
            }
            Tag::FootnoteDefinition(_) => return Err(Problem::Footnote),
            Tag::Table(_) => {
                self.open_block();
                self.push("[table]\n");
                self.containers.push(Container::Table);
            }
            Tag::TableHead => {
                self.push("[tr]\n");
                self.table_head = true;
            }
            Tag::TableRow => self.push("[tr]\n"),
            Tag::TableCell => self.push(if self.table_head { "[th]" } else { "[td]" }),
            Tag::Emphasis => self.push("[i]"),
            Tag::Strong => self.push("[b]"),
            Tag::Strikethrough => self.push("[strike]"),
            Tag::Link {
                link_type,
                dest_url,
                ..
            } => {
                let scheme = if link_type == LinkType::Email && !dest_url.contains(':') {
                    "mailto:"
                } else {
                    ""
                };
                self.push(&format!("[url={scheme}{}]", url(&dest_url)));
            }
            Tag::Image { dest_url, .. } => {
                if !is_absolute(&dest_url) {
                    return Err(Problem::RelativeImage(dest_url.into_string()));
                }
                self.push(&format!("[img]{}[/img]", url(&dest_url)));
                self.image = true;
            }
            // Not enabled in the parser options.
            Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Superscript
            | Tag::Subscript
            | Tag::MetadataBlock(_) => unreachable!("not enabled"),
        }
        Ok(())
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.close_block(),
            TagEnd::Heading(level) => {
                self.push(&format!("[/{level}]"));
                self.close_block();
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.trim_newlines();
                self.push("[/quote]");
                self.containers.pop();
                self.close_block();
            }
            TagEnd::CodeBlock => {
                let code = self.code.take().unwrap_or_default();
                let code = code.strip_suffix('\n').unwrap_or(&code);
                self.push(&format!("[code]{}[/code]", protect(code)));
                self.close_block();
            }
            TagEnd::List(ordered) => {
                self.newline();
                self.push(if ordered { "[/olist]" } else { "[/list]" });
                self.containers.pop();
                self.close_block();
            }
            TagEnd::Item => {
                self.flush();
                self.trim_newlines();
                self.containers.pop();
                self.newline();
                self.after_block = false;
            }
            TagEnd::Table => {
                self.push("[/table]");
                self.containers.pop();
                self.close_block();
            }
            TagEnd::TableHead => {
                self.push("[/tr]\n");
                self.table_head = false;
            }
            TagEnd::TableRow => self.push("[/tr]\n"),
            TagEnd::TableCell => self.push(if self.table_head {
                "[/th]\n"
            } else {
                "[/td]\n"
            }),
            TagEnd::Emphasis => self.push("[/i]"),
            TagEnd::Strong => self.push("[/b]"),
            TagEnd::Strikethrough => self.push("[/strike]"),
            TagEnd::Link => self.push("[/url]"),
            TagEnd::Image => self.image = false,
            TagEnd::HtmlBlock | TagEnd::FootnoteDefinition => {}
            TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::Superscript
            | TagEnd::Subscript
            | TagEnd::MetadataBlock(_) => unreachable!("not enabled"),
        }
    }

    fn inline(&mut self, text: &str) {
        self.text.push_str(text);
        self.fresh = false;
    }

    fn push(&mut self, bbcode: &str) {
        self.flush();
        self.out.push_str(bbcode);
        self.fresh = false;
    }

    fn flush(&mut self) {
        if !self.text.is_empty() {
            let text = std::mem::take(&mut self.text);
            self.out.push_str(&protect(&text));
        }
    }

    fn newline(&mut self) {
        self.flush();
        if !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
    }

    fn trim_newlines(&mut self) {
        let trimmed = self.out.trim_end_matches('\n').len();
        self.out.truncate(trimmed);
    }

    fn open_block(&mut self) {
        if std::mem::take(&mut self.fresh) {
            self.after_block = false;
            return;
        }
        self.newline();
        let in_item = self.containers.last() == Some(&Container::Item);
        if std::mem::take(&mut self.after_block) && !in_item {
            self.out.push('\n');
        }
    }

    fn close_block(&mut self) {
        self.newline();
        self.after_block = true;
        self.fresh = false;
    }

    fn finish(mut self) -> String {
        self.flush();
        self.out.truncate(self.out.trim_end().len());
        self.out
    }
}

/// Text that Steam would otherwise read as tags goes in `[noparse]`.
fn protect(text: &str) -> Cow<'_, str> {
    if text.contains('[') {
        Cow::Owned(format!("[noparse]{text}[/noparse]"))
    } else {
        Cow::Borrowed(text)
    }
}

/// A URL that cannot end a `[url=…]` or `[img]` tag early.
fn url(url: &str) -> String {
    url.replace(']', "%5D")
        .replace('[', "%5B")
        .replace(' ', "%20")
}

fn is_absolute(url: &str) -> bool {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert(markdown: &str) -> String {
        to_bbcode(markdown).unwrap()
    }

    fn error(markdown: &str) -> String {
        to_bbcode(markdown).unwrap_err().to_string()
    }

    #[test]
    fn headings_and_paragraphs() {
        assert_eq!(
            convert("# Title\n\nFirst line\nsame paragraph.\n\n## Part\n### Sub\n\nEnd."),
            "[h1]Title[/h1]\n\nFirst line\nsame paragraph.\n\n[h2]Part[/h2]\n\n[h3]Sub[/h3]\n\nEnd."
        );
    }

    #[test]
    fn line_breaks_inside_a_paragraph_stay() {
        // Bellows' credits: one line per credit, no blank lines between.
        assert_eq!(
            convert(
                "**Anvil** for letting a dwarf into the forge.\n\
                 **mimalloc** by Microsoft for the faster memory handling.\n"
            ),
            "[b]Anvil[/b] for letting a dwarf into the forge.\n\
             [b]mimalloc[/b] by Microsoft for the faster memory handling."
        );
        // The team's FAQ answer, then the next paragraph after a blank line.
        assert_eq!(
            convert(
                "I like dwarves.\nI like being happy.\nI am, in fact, a happy dwarf myself.\n\n\
                 Next question."
            ),
            "I like dwarves.\nI like being happy.\nI am, in fact, a happy dwarf myself.\n\n\
             Next question."
        );
        // Hard breaks (two spaces or a backslash) are line breaks too.
        assert_eq!(convert("a  \nb\\\nc\nd"), "a\nb\nc\nd");
        // CRLF files give the same result.
        assert_eq!(convert("one\r\ntwo\r\n\r\nthree\r\n"), "one\ntwo\n\nthree");
    }

    #[test]
    fn line_breaks_in_lists_quotes_and_headings() {
        assert_eq!(
            convert("- first line\n  second line\n- next"),
            "[list]\n[*]first line\nsecond line\n[*]next\n[/list]"
        );
        assert_eq!(
            convert("> said\n> by [b] someone"),
            "[quote][noparse]said\nby [b] someone[/noparse][/quote]"
        );
        // A setext heading may span lines, as GitHub shows it.
        assert_eq!(convert("Two\nlines\n==="), "[h1]Two\nlines[/h1]");
        // An image's alt text is dropped, line breaks included.
        assert_eq!(
            convert("![a\nb](https://example.com/i.png) after"),
            "[img]https://example.com/i.png[/img] after"
        );
    }

    #[test]
    fn inline_formatting() {
        assert_eq!(
            convert("**bold** *italic* _also_ ~~gone~~ `code`  \nnext"),
            "[b]bold[/b] [i]italic[/i] [i]also[/i] [strike]gone[/strike] code\nnext"
        );
    }

    #[test]
    fn links_and_images() {
        assert_eq!(
            convert("[site](<https://example.com/a b>) <https://example.com> <me@example.com>"),
            "[url=https://example.com/a%20b]site[/url] \
             [url=https://example.com]https://example.com[/url] \
             [url=mailto:me@example.com]me@example.com[/url]"
        );
        assert_eq!(
            convert("![Logo](https://example.com/logo.png)"),
            "[img]https://example.com/logo.png[/img]"
        );
        assert_eq!(
            convert("[![Logo](HTTP://example.com/l.png)](https://example.com)"),
            "[url=https://example.com][img]HTTP://example.com/l.png[/img][/url]"
        );
    }

    #[test]
    fn lists() {
        assert_eq!(
            convert("Intro\n\n- one\n- two\n  1. a\n  2. b\n- three\n\nAfter"),
            "Intro\n\n[list]\n[*]one\n[*]two\n[olist]\n[*]a\n[*]b\n[/olist]\n[*]three\n[/list]\n\nAfter"
        );
        // A loose list keeps one line per paragraph inside an item.
        assert_eq!(
            convert("1. first\n\n   more\n\n2. second\n"),
            "[olist]\n[*]first\nmore\n[*]second\n[/olist]"
        );
    }

    #[test]
    fn quotes_code_and_rules() {
        assert_eq!(
            convert("> quoted\n> text\n>\n> again\n\n---\n\n```rust\nlet a = [1];\n```"),
            "[quote]quoted\ntext\n\nagain[/quote]\n\n[hr][/hr]\n\n[code][noparse]let a = [1];[/noparse][/code]"
        );
        assert_eq!(
            convert("    indented\n    code\n"),
            "[code]indented\ncode[/code]"
        );
    }

    #[test]
    fn tables() {
        assert_eq!(
            convert("| Name | Value |\n| :--- | ---: |\n| a | **1** |\n| b | 2 |\n"),
            "[table]\n[tr]\n[th]Name[/th]\n[th]Value[/th]\n[/tr]\n\
             [tr]\n[td]a[/td]\n[td][b]1[/b][/td]\n[/tr]\n\
             [tr]\n[td]b[/td]\n[td]2[/td]\n[/tr]\n[/table]"
        );
    }

    #[test]
    fn brackets_in_text_are_not_tags() {
        assert_eq!(
            convert("Use [b] for bold, see [1.2.0]."),
            "[noparse]Use [b] for bold, see [1.2.0].[/noparse]"
        );
        assert_eq!(convert("`[h1]`"), "[noparse][h1][/noparse]");
    }

    #[test]
    fn html_comments_are_dropped() {
        assert_eq!(
            convert(
                "<!-- Release notes generated using configuration in .github/release.yml -->\n\n\
                 ## What's Changed\n\n<!--\nmulti-line\n-->\nText <!-- inline --> here\n"
            ),
            "[h2]What's Changed[/h2]\n\nText  here"
        );
    }

    #[test]
    fn errors_name_the_line() {
        assert_eq!(
            error("Text\n\n<div>html</div>\n"),
            "line 3: raw HTML is not supported; use Markdown only"
        );
        assert_eq!(
            error("Text with <b>inline</b> HTML"),
            "line 1: raw HTML is not supported; use Markdown only"
        );
        assert_eq!(
            error("a\n\n<!-- comment -->\n<p>\nhidden\n</p>\n"),
            "line 4: raw HTML is not supported; use Markdown only"
        );
        assert_eq!(
            error("Note[^1].\n\n[^1]: The note.\n"),
            "line 1: footnotes are not supported"
        );
        assert_eq!(
            error("Intro\n\n![shot](docs/shot.png)\n"),
            "line 3: the image `docs/shot.png` needs an absolute http(s) URL; \
             Steam cannot show repository files"
        );
        assert_eq!(
            error("# A\n\n#### Deep\n"),
            "line 3: h4 headings are not supported; Steam BBCode has h1 to h3"
        );
    }

    #[test]
    fn windows_line_endings() {
        assert_eq!(
            convert("# A\r\n\r\n```\r\nx\r\ny\r\n```\r\n"),
            "[h1]A[/h1]\n\n[code]x\ny[/code]"
        );
    }

    #[test]
    fn errors_can_be_moved_into_a_larger_file() {
        let error = to_bbcode("ok\n<br>\n").unwrap_err().starting_at(10);
        assert_eq!(error.line, 11);
    }

    #[test]
    fn line_index() {
        let index = LineIndex::new("a\nbc\n\nd");
        assert_eq!(
            [0, 1, 2, 4, 5, 6].map(|offset| index.line(offset)),
            [1, 1, 2, 2, 3, 4]
        );
    }
}
