//! `descriptor.mod`: the Clausewitz-script descriptor of EU4, HOI4, Stellaris,
//! Imperator and CK3 mods (spec section 5).
//!
//! The format is `key="value"` or `key=value`, lists are `key={ "a" "b" }`,
//! and `#` starts a comment. Keys other than the ones below are kept in
//! [`DescriptorMod::entries`] and otherwise ignored.

use std::collections::HashMap;
use std::ops::Range;

use crate::item_id::ItemId;

/// A parsed `descriptor.mod`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorMod {
    /// The Workshop title.
    pub name: String,
    pub version: Option<String>,
    pub supported_version: Option<String>,
    pub tags: Vec<String>,
    pub picture: Option<String>,
    pub dependencies: Vec<String>,
    /// Every `replace_path`, in file order.
    pub replace_paths: Vec<String>,
    pub remote_file_id: Option<ItemId>,
    /// Every entry in file order, with its place in the text, so the
    /// published copy can drop local-only keys and add `remote_file_id`.
    pub entries: Vec<Entry>,
    /// The file's text, which [`Entry::span`] indexes.
    pub text: String,
}

/// One `key=value` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub value: Value,
    /// The 1-based line of the key.
    pub line: usize,
    /// Byte range of the entry, from the key to the end of the value.
    pub span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A quoted or bare value.
    Text(String),
    /// A `{ ... }` list of quoted or bare values.
    List(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DescriptorError {
    #[error("the file starts with a UTF-8 byte order mark (BOM); save it as UTF-8 without BOM")]
    Bom,
    #[error("the file is not valid UTF-8")]
    NotUtf8,
    #[error("line {line}: {message}")]
    Syntax { line: usize, message: String },
    #[error("line {line}: `{key}` appears a second time (first on line {first})")]
    Duplicate {
        key: String,
        line: usize,
        first: usize,
    },
    #[error("line {line}: `{key}` must be {expected}")]
    WrongType {
        key: String,
        line: usize,
        expected: &'static str,
    },
    #[error("line {line}: remote_file_id `{value}` is not a Workshop item ID")]
    InvalidRemoteFileId { line: usize, value: String },
    #[error("`name` is missing or empty; the Workshop title comes from it")]
    MissingName,
}

const TEXT: &str = "a single value, e.g. key=\"value\"";
const LIST: &str = "a list, e.g. key={ \"a\" \"b\" }";

/// Parses the bytes of a `descriptor.mod`, which must be UTF-8 without a BOM.
pub fn parse(bytes: &[u8]) -> Result<DescriptorMod, DescriptorError> {
    if bytes.starts_with(b"\xEF\xBB\xBF") {
        return Err(DescriptorError::Bom);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| DescriptorError::NotUtf8)?;
    let entries = parse_entries(text)?;

    let mut first_lines: HashMap<&str, usize> = HashMap::new();
    let mut name = None;
    let mut version = None;
    let mut supported_version = None;
    let mut tags = Vec::new();
    let mut picture = None;
    let mut dependencies = Vec::new();
    let mut replace_paths = Vec::new();
    let mut remote_file_id = None;
    for entry in &entries {
        let key = entry.key.as_str();
        let single = matches!(
            key,
            "name"
                | "version"
                | "supported_version"
                | "picture"
                | "remote_file_id"
                | "path"
                | "archive"
                | "user_dir"
                | "tags"
                | "dependencies"
        );
        if single {
            if let Some(&first) = first_lines.get(key) {
                return Err(DescriptorError::Duplicate {
                    key: key.to_owned(),
                    line: entry.line,
                    first,
                });
            }
            first_lines.insert(key, entry.line);
        }
        match key {
            "name" => name = Some(text_value(entry)?),
            "version" => version = Some(text_value(entry)?),
            "supported_version" => supported_version = Some(text_value(entry)?),
            "picture" => picture = Some(text_value(entry)?),
            "path" | "archive" | "user_dir" => {
                text_value(entry)?;
            }
            "replace_path" => replace_paths.push(text_value(entry)?),
            "tags" => tags = list_value(entry)?,
            "dependencies" => dependencies = list_value(entry)?,
            "remote_file_id" => {
                let value = text_value(entry)?;
                let id = value
                    .parse()
                    .map_err(|_| DescriptorError::InvalidRemoteFileId {
                        line: entry.line,
                        value,
                    })?;
                remote_file_id = Some(id);
            }
            _ => {}
        }
    }

    let name = name
        .filter(|name| !name.trim().is_empty())
        .ok_or(DescriptorError::MissingName)?;
    Ok(DescriptorMod {
        name,
        version,
        supported_version,
        tags,
        picture,
        dependencies,
        replace_paths,
        remote_file_id,
        entries,
        text: text.to_owned(),
    })
}

/// Keys that only make sense on the modder's computer, left out of the
/// published copy (spec section 6).
pub const LOCAL_ONLY_KEYS: [&str; 3] = ["path", "archive", "user_dir"];

/// The `descriptor.mod` of the published copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedDescriptor {
    pub text: String,
    /// Keys left out, in file order.
    pub removed: Vec<String>,
    /// Keys added at the end.
    pub added: Vec<String>,
}

impl DescriptorMod {
    /// The text for the published copy: without [`LOCAL_ONLY_KEYS`] and with
    /// `remote_file_id` set to `item_id` when the item exists. Everything
    /// else, comments and layout included, stays as it is.
    pub fn for_published_copy(&self, item_id: Option<ItemId>) -> PublishedDescriptor {
        let mut removed = Vec::new();
        let mut cuts = Vec::new();
        for entry in &self.entries {
            let stale_id = entry.key == "remote_file_id" && self.remote_file_id != item_id;
            if LOCAL_ONLY_KEYS.contains(&entry.key.as_str()) || stale_id {
                if !removed.contains(&entry.key) {
                    removed.push(entry.key.clone());
                }
                cuts.push(line_cut(&self.text, entry.span.clone()));
            }
        }

        let mut text = String::with_capacity(self.text.len() + 32);
        let mut pos = 0;
        for cut in cuts {
            text.push_str(&self.text[pos..cut.start]);
            pos = cut.end;
        }
        text.push_str(&self.text[pos..]);

        let mut added = Vec::new();
        if let Some(id) = item_id
            && self.remote_file_id != Some(id)
        {
            let newline = if self.text.contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            if !text.is_empty() && !text.ends_with('\n') {
                text.push_str(newline);
            }
            text.push_str(&format!("remote_file_id=\"{id}\"{newline}"));
            added.push("remote_file_id".to_owned());
        }
        PublishedDescriptor {
            text,
            removed,
            added,
        }
    }
}

/// The range to cut for an entry: its whole line when nothing else is on
/// it, otherwise just the entry.
fn line_cut(text: &str, span: Range<usize>) -> Range<usize> {
    let bytes = text.as_bytes();
    let mut start = span.start;
    while start > 0 && matches!(bytes[start - 1], b' ' | b'\t') {
        start -= 1;
    }
    let mut end = span.end;
    while end < bytes.len() && matches!(bytes[end], b' ' | b'\t') {
        end += 1;
    }
    let line_start = start == 0 || bytes[start - 1] == b'\n';
    if line_start && (end == bytes.len() || bytes[end] == b'\n') {
        start..(end + 1).min(bytes.len())
    } else if line_start && bytes[end..].starts_with(b"\r\n") {
        start..end + 2
    } else {
        // Another entry shares the line: cut the entry and the blanks after it.
        span.start..end
    }
}

fn text_value(entry: &Entry) -> Result<String, DescriptorError> {
    match &entry.value {
        Value::Text(value) => Ok(value.clone()),
        Value::List(_) => Err(DescriptorError::WrongType {
            key: entry.key.clone(),
            line: entry.line,
            expected: TEXT,
        }),
    }
}

fn list_value(entry: &Entry) -> Result<Vec<String>, DescriptorError> {
    match &entry.value {
        Value::List(values) => Ok(values.clone()),
        Value::Text(_) => Err(DescriptorError::WrongType {
            key: entry.key.clone(),
            line: entry.line,
            expected: LIST,
        }),
    }
}

fn parse_entries(text: &str) -> Result<Vec<Entry>, DescriptorError> {
    let mut lexer = Lexer::new(text);
    let mut entries = Vec::new();
    while let Some(key) = lexer.next_token()? {
        let Token::Word(name) = key.token else {
            return Err(syntax(
                key.line,
                format!("expected a key, found {}", key.token.describe()),
            ));
        };
        match lexer.next_token()? {
            Some(Spanned {
                token: Token::Equals,
                ..
            }) => {}
            _ => return Err(syntax(key.line, format!("expected `=` after `{name}`"))),
        }
        let Some(first) = lexer.next_token()? else {
            return Err(syntax(key.line, format!("`{name}` has no value")));
        };
        let (value, end) = match first.token {
            Token::Word(value) | Token::Quoted(value) => (Value::Text(value), first.span.end),
            Token::Open => {
                let mut values = Vec::new();
                loop {
                    let Some(item) = lexer.next_token()? else {
                        return Err(syntax(
                            first.line,
                            format!("the list of `{name}` is not closed with `}}`"),
                        ));
                    };
                    match item.token {
                        Token::Word(value) | Token::Quoted(value) => values.push(value),
                        Token::Close => break (Value::List(values), item.span.end),
                        Token::Open => {
                            return Err(syntax(
                                item.line,
                                format!("nested blocks in `{name}` are not supported"),
                            ));
                        }
                        Token::Equals => {
                            return Err(syntax(
                                item.line,
                                format!("unexpected `=` in the list of `{name}`"),
                            ));
                        }
                    }
                }
            }
            token => {
                return Err(syntax(
                    first.line,
                    format!("`{name}` has no value, found {}", token.describe()),
                ));
            }
        };
        entries.push(Entry {
            key: name,
            value,
            line: key.line,
            span: key.span.start..end,
        });
    }
    Ok(entries)
}

fn syntax(line: usize, message: String) -> DescriptorError {
    DescriptorError::Syntax { line, message }
}

#[derive(Debug)]
enum Token {
    Word(String),
    Quoted(String),
    Equals,
    Open,
    Close,
}

impl Token {
    fn describe(&self) -> String {
        match self {
            Token::Word(word) => format!("`{word}`"),
            Token::Quoted(text) => format!("\"{text}\""),
            Token::Equals => "`=`".to_owned(),
            Token::Open => "`{`".to_owned(),
            Token::Close => "`}`".to_owned(),
        }
    }
}

struct Spanned {
    token: Token,
    line: usize,
    span: Range<usize>,
}

struct Lexer<'a> {
    text: &'a str,
    pos: usize,
    line: usize,
}

impl<'a> Lexer<'a> {
    fn new(text: &'a str) -> Self {
        Lexer {
            text,
            pos: 0,
            line: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.text[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        if c == '\n' {
            self.line += 1;
        }
        Some(c)
    }

    fn skip_blanks(&mut self) {
        while let Some(c) = self.peek() {
            if c == '#' {
                while self.peek().is_some_and(|c| c != '\n') {
                    self.bump();
                }
            } else if c.is_whitespace() {
                self.bump();
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Result<Option<Spanned>, DescriptorError> {
        self.skip_blanks();
        let start = self.pos;
        let line = self.line;
        let Some(c) = self.bump() else {
            return Ok(None);
        };
        let token = match c {
            '=' => Token::Equals,
            '{' => Token::Open,
            '}' => Token::Close,
            '"' => {
                let mut value = String::new();
                loop {
                    match self.bump() {
                        None => return Err(syntax(line, "the string is not closed".to_owned())),
                        Some('"') => break,
                        Some('\\') if matches!(self.peek(), Some('"' | '\\')) => {
                            value.extend(self.bump());
                        }
                        Some(c) => value.push(c),
                    }
                }
                Token::Quoted(value)
            }
            _ => {
                while self.peek().is_some_and(|c| {
                    !c.is_whitespace() && !matches!(c, '=' | '{' | '}' | '"' | '#')
                }) {
                    self.bump();
                }
                Token::Word(self.text[start..self.pos].to_owned())
            }
        };
        Ok(Some(Spanned {
            token,
            line,
            span: start..self.pos,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error(text: &str) -> String {
        parse(text.as_bytes()).unwrap_err().to_string()
    }

    #[test]
    fn reads_the_known_keys() {
        let text = r#"version="1.4.2"
tags={
	"Gameplay"
	"Missions and Decisions"
}
name="My Mod"
picture="thumbnail.png"
replace_path="history/provinces"
replace_path="common/bookmarks"
dependencies={ "Base Mod" }
supported_version="v1.37.*"
remote_file_id="2948217283"
"#;
        let descriptor = parse(text.as_bytes()).unwrap();
        assert_eq!(descriptor.name, "My Mod");
        assert_eq!(descriptor.version.as_deref(), Some("1.4.2"));
        assert_eq!(descriptor.supported_version.as_deref(), Some("v1.37.*"));
        assert_eq!(descriptor.tags, ["Gameplay", "Missions and Decisions"]);
        assert_eq!(descriptor.picture.as_deref(), Some("thumbnail.png"));
        assert_eq!(descriptor.dependencies, ["Base Mod"]);
        assert_eq!(
            descriptor.replace_paths,
            ["history/provinces", "common/bookmarks"]
        );
        assert_eq!(descriptor.remote_file_id.map(ItemId::get), Some(2948217283));
        assert_eq!(descriptor.entries.len(), 9);
    }

    #[test]
    fn entries_keep_their_line_and_span() {
        let text = "# comment\nname = \"A\"\npath=\"C:/Users/someone/mod\"\ntags = { a b }\n";
        let descriptor = parse(text.as_bytes()).unwrap();
        let spans: Vec<_> = descriptor
            .entries
            .iter()
            .map(|entry| (entry.key.as_str(), entry.line, &text[entry.span.clone()]))
            .collect();
        assert_eq!(
            spans,
            [
                ("name", 2, "name = \"A\""),
                ("path", 3, "path=\"C:/Users/someone/mod\""),
                ("tags", 4, "tags = { a b }"),
            ]
        );
    }

    #[test]
    fn accepts_bare_values_crlf_comments_and_escapes() {
        let text = "name=\"Say \\\"hi\\\"\" # trailing comment\r\nremote_file_id=123\r\nunknown_key={ x }\r\n";
        let descriptor = parse(text.as_bytes()).unwrap();
        assert_eq!(descriptor.name, "Say \"hi\"");
        assert_eq!(descriptor.remote_file_id.map(ItemId::get), Some(123));
        assert_eq!(descriptor.entries[2].value, Value::List(vec!["x".into()]));
    }

    #[test]
    fn rejects_a_bom() {
        let mut bytes = b"\xEF\xBB\xBF".to_vec();
        bytes.extend_from_slice(b"name=\"A\"\n");
        assert_eq!(parse(&bytes), Err(DescriptorError::Bom));
    }

    #[test]
    fn rejects_invalid_utf8() {
        assert_eq!(parse(b"name=\"\xE9\"\n"), Err(DescriptorError::NotUtf8));
    }

    #[test]
    fn name_is_required() {
        assert_eq!(
            error("version=\"1\"\n"),
            "`name` is missing or empty; the Workshop title comes from it"
        );
        assert!(parse(b"name=\"  \"\n").is_err());
    }

    #[test]
    fn single_keys_cannot_repeat() {
        assert_eq!(
            error("name=\"A\"\nversion=\"1\"\nname=\"B\"\n"),
            "line 3: `name` appears a second time (first on line 1)"
        );
        assert!(error("name=\"A\"\ntags={}\ntags={}\n").contains("`tags` appears a second time"));
    }

    #[test]
    fn values_must_have_the_right_shape() {
        assert_eq!(
            error("name=\"A\"\ntags=\"Gameplay\"\n"),
            "line 2: `tags` must be a list, e.g. key={ \"a\" \"b\" }"
        );
        assert_eq!(
            error("name={ \"A\" }\n"),
            "line 1: `name` must be a single value, e.g. key=\"value\""
        );
        assert_eq!(
            error("name=\"A\"\nremote_file_id=\"12x\"\n"),
            "line 2: remote_file_id `12x` is not a Workshop item ID"
        );
    }

    #[test]
    fn syntax_errors_name_the_line() {
        for (text, expected) in [
            ("name=\"A\n", "line 1: the string is not closed"),
            (
                "name=\"A\"\ntags={ \"a\"\n",
                "line 2: the list of `tags` is not closed with `}`",
            ),
            ("name \"A\"\n", "line 1: expected `=` after `name`"),
            ("name=\"A\"\n}\n", "line 2: expected a key, found `}`"),
            ("name=\"A\"\nversion=\n", "line 2: `version` has no value"),
            (
                "name=\"A\"\nx={ { } }\n",
                "line 2: nested blocks in `x` are not supported",
            ),
            (
                "name=\"A\"\nx={ a = b }\n",
                "line 2: unexpected `=` in the list of `x`",
            ),
        ] {
            assert_eq!(error(text), expected, "{text:?}");
        }
    }

    fn published(text: &str, item_id: Option<u64>) -> PublishedDescriptor {
        let descriptor = parse(text.as_bytes()).unwrap();
        descriptor.for_published_copy(item_id.map(|id| ItemId::try_from(id).unwrap()))
    }

    #[test]
    fn the_published_copy_drops_local_keys_and_keeps_the_rest() {
        let text = "# My mod\nname=\"A\"\npath=\"C:/Users/someone/mod\"\n\tarchive=\"a.zip\"  \ntags={ \"x\" }\nuser_dir=\"a\"\nremote_file_id=\"7\"\n";
        let copy = published(text, Some(7));
        assert_eq!(
            copy.text,
            "# My mod\nname=\"A\"\ntags={ \"x\" }\nremote_file_id=\"7\"\n"
        );
        assert_eq!(copy.removed, ["path", "archive", "user_dir"]);
        assert!(copy.added.is_empty());
    }

    #[test]
    fn the_published_copy_gains_remote_file_id() {
        let copy = published("name=\"A\"\r\npath=\"x\"\r\n", Some(42));
        assert_eq!(copy.text, "name=\"A\"\r\nremote_file_id=\"42\"\r\n");
        assert_eq!(copy.added, ["remote_file_id"]);

        let copy = published("name=\"A\"", Some(42));
        assert_eq!(copy.text, "name=\"A\"\nremote_file_id=\"42\"\n");
    }

    #[test]
    fn entries_sharing_a_line_are_cut_alone() {
        let copy = published("name=\"A\" path=\"x\" version=\"1\"\n", None);
        assert_eq!(copy.text, "name=\"A\" version=\"1\"\n");
    }

    #[test]
    fn a_new_item_has_no_remote_file_id() {
        let copy = published("name=\"A\"\n", None);
        assert_eq!(copy.text, "name=\"A\"\n");
        assert!(copy.removed.is_empty() && copy.added.is_empty());
    }
}
