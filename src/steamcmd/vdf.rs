//! The `workshop_build_item` VDF (spec section 9).
//!
//! steamcmd reads it as KeyValues without escape sequences: a quoted value
//! runs to the next `"`, even after a `\`, and `\` is an ordinary character.
//! Newlines, `{`, `}` and `//` inside a quoted value are kept as they are.

use std::fmt::Write;
use std::path::{Path, PathBuf};

use super::Error;
use crate::config::Visibility;

/// The keys of one `workshopitem` VDF. Only the keys the plan sets are
/// written; `None` leaves that Workshop field as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemVdf {
    pub app_id: u32,
    /// `0` creates a new item.
    pub published_file_id: u64,
    pub content_folder: PathBuf,
    pub preview_file: Option<PathBuf>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub visibility: Option<Visibility>,
    pub change_note: Option<String>,
}

/// Text for a quoted VDF value. A `"` would end the value, so each one
/// becomes a typographic quote: `“` where a quotation opens (at the start,
/// after whitespace or an opening bracket), `”` elsewhere.
pub fn text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut previous: Option<char> = None;
    for c in value.chars() {
        out.push(match c {
            '"' if previous.is_none_or(|p| p.is_whitespace() || "([{‘“".contains(p)) => '“',
            '"' => '”',
            c => c,
        });
        previous = Some(c);
    }
    out
}

/// A path for a quoted VDF value, as it is.
fn path(path: &Path) -> Result<String, Error> {
    let text = path.to_string_lossy();
    if text.contains('"') {
        return Err(Error::QuoteInPath(path.to_owned()));
    }
    Ok(text.into_owned())
}

/// Steam's `ERemoteStoragePublishedFileVisibility`.
fn visibility_code(visibility: Visibility) -> u8 {
    match visibility {
        Visibility::Public => 0,
        Visibility::Friends => 1,
        Visibility::Private => 2,
        Visibility::Unlisted => 3,
    }
}

impl ItemVdf {
    /// The VDF text; fails when a path contains `"`.
    pub fn render(&self) -> Result<String, Error> {
        let mut fields: Vec<(&str, String)> = vec![
            ("appid", self.app_id.to_string()),
            ("publishedfileid", self.published_file_id.to_string()),
            ("contentfolder", path(&self.content_folder)?),
        ];
        if let Some(preview) = &self.preview_file {
            fields.push(("previewfile", path(preview)?));
        }
        if let Some(title) = &self.title {
            fields.push(("title", text(title)));
        }
        if let Some(description) = &self.description {
            fields.push(("description", text(description)));
        }
        if let Some(visibility) = self.visibility {
            fields.push(("visibility", visibility_code(visibility).to_string()));
        }
        if let Some(change_note) = &self.change_note {
            fields.push(("changenote", text(change_note)));
        }

        let mut vdf = String::from("\"workshopitem\"\n{\n");
        for (key, value) in fields {
            let _ = writeln!(vdf, "\t\"{key}\" \"{value}\"");
        }
        vdf.push_str("}\n");
        Ok(vdf)
    }
}

/// The `publishedfileid` steamcmd wrote back into the VDF; `0` and a
/// missing key give `None`.
pub fn published_file_id(text: &str) -> Option<u64> {
    const KEY: &str = "\"publishedfileid\"";
    let rest = &text[text.find(KEY)? + KEY.len()..];
    let rest = rest.trim_start().strip_prefix('"')?;
    let digits = &rest[..rest.find('"')?];
    digits.parse().ok().filter(|&id| id != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_only_the_set_keys() {
        let vdf = ItemVdf {
            app_id: 236850,
            published_file_id: 42,
            content_folder: PathBuf::from(r"D:\a\_temp\copy"),
            preview_file: None,
            title: None,
            description: None,
            visibility: None,
            change_note: Some("Version 1.2".to_owned()),
        };
        assert_eq!(
            vdf.render().unwrap(),
            "\"workshopitem\"\n{\n\t\"appid\" \"236850\"\n\t\"publishedfileid\" \"42\"\n\t\"contentfolder\" \"D:\\a\\_temp\\copy\"\n\t\"changenote\" \"Version 1.2\"\n}\n"
        );
    }

    #[test]
    fn renders_every_key() {
        let vdf = ItemVdf {
            app_id: 529340,
            published_file_id: 0,
            content_folder: PathBuf::from("/tmp/copy"),
            preview_file: Some(PathBuf::from("/repo/.metadata/thumbnail.png")),
            title: Some("The \"Best\" Mod".to_owned()),
            description: Some("[b]bold[/b]\nline two".to_owned()),
            visibility: Some(Visibility::Private),
            change_note: None,
        };
        let text = vdf.render().unwrap();
        assert!(text.contains("\t\"publishedfileid\" \"0\"\n"));
        assert!(text.contains("\t\"previewfile\" \"/repo/.metadata/thumbnail.png\"\n"));
        assert!(text.contains("\t\"title\" \"The “Best” Mod\"\n"));
        assert!(text.contains("\t\"description\" \"[b]bold[/b]\nline two\"\n"));
        assert!(text.contains("\t\"visibility\" \"2\"\n"));
        assert!(!text.contains("changenote"));
    }

    #[test]
    fn quotes_become_typographic() {
        assert_eq!(
            text(r#"The new choice "Start-up only" is the default."#),
            "The new choice “Start-up only” is the default."
        );
        assert_eq!(text("\"a\"\n(\"b\") [\"c\"]"), "“a”\n(“b”) [“c”]");
        assert_eq!(text(r#"12" and "x""#), "12” and “x”");
        assert_eq!(text(r"C:\mods\{a} // b"), r"C:\mods\{a} // b");
    }

    /// The tokens steamcmd reads from a VDF, as Valve's KeyValues does
    /// without escape sequences: a quoted token runs to the next `"`, an
    /// unquoted one to whitespace, `"`, `{` or `}`.
    fn tokens(vdf: &str) -> Vec<(String, bool)> {
        let mut tokens = Vec::new();
        let mut rest = vdf;
        loop {
            rest = rest.trim_start();
            let Some(first) = rest.chars().next() else {
                return tokens;
            };
            if first == '"' {
                let end = rest[1..].find('"').expect("an unterminated value") + 1;
                tokens.push((rest[1..end].to_owned(), true));
                rest = &rest[end + 1..];
            } else if first == '{' || first == '}' {
                tokens.push((first.to_string(), false));
                rest = &rest[1..];
            } else {
                let end = rest
                    .find(|c: char| c.is_whitespace() || "\"{}".contains(c))
                    .unwrap_or(rest.len());
                tokens.push((rest[..end].to_owned(), false));
                rest = &rest[end..];
            }
        }
    }

    #[test]
    fn steamcmd_reads_back_every_value() {
        // Bellows v0.11.0's note failed with `got } in key` while `"` was
        // written as `\"`.
        let hard = "### Measured\n\nThe new choice \"Start-up only\" is the default.\n\
                    [code]C:\\mods\\ \\\" {x} } { // not a comment[/code]\n\"\\\"";
        let vdf = ItemVdf {
            app_id: 236850,
            published_file_id: 42,
            content_folder: PathBuf::from(r"D:\a\_temp\copy"),
            preview_file: Some(PathBuf::from(r"D:\a\repo\{thumb}.png")),
            title: Some("\"Quoted\" \\ {title}".to_owned()),
            description: Some(hard.to_owned()),
            visibility: None,
            change_note: Some(hard.to_owned()),
        };
        let tokens = tokens(&vdf.render().unwrap());
        let unquoted = |t: &str| (t.to_owned(), false);
        let quoted = |t: &str| (t.to_owned(), true);
        assert_eq!(tokens[..2], [quoted("workshopitem"), unquoted("{")]);
        assert_eq!(tokens.last(), Some(&unquoted("}")));
        let pairs: Vec<_> = tokens[2..tokens.len() - 1]
            .chunks(2)
            .map(|pair| {
                assert!(pair.iter().all(|(_, quoted)| *quoted), "{pair:?}");
                (pair[0].0.as_str(), pair[1].0.as_str())
            })
            .collect();
        let read_back = text(hard);
        assert_eq!(
            pairs,
            [
                ("appid", "236850"),
                ("publishedfileid", "42"),
                ("contentfolder", r"D:\a\_temp\copy"),
                ("previewfile", r"D:\a\repo\{thumb}.png"),
                ("title", "“Quoted” \\ {title}"),
                ("description", read_back.as_str()),
                ("changenote", read_back.as_str()),
            ]
        );
        assert!(read_back.contains("“Start-up only”") && !read_back.contains('"'));
    }

    #[test]
    fn refuses_a_path_with_a_quote() {
        let vdf = ItemVdf {
            app_id: 236850,
            published_file_id: 42,
            content_folder: PathBuf::from("/tmp/a\"b"),
            preview_file: None,
            title: None,
            description: None,
            visibility: None,
            change_note: None,
        };
        assert!(
            matches!(vdf.render(), Err(Error::QuoteInPath(path)) if path == Path::new("/tmp/a\"b"))
        );
    }

    #[test]
    fn reads_the_written_back_id() {
        let text = "\"workshopitem\"\n{\n\t\"appid\"\t\t\"236850\"\n\t\"publishedfileid\"\t\t\"3823257732\"\n}\n";
        assert_eq!(published_file_id(text), Some(3823257732));
        assert_eq!(published_file_id("\"publishedfileid\" \"0\""), None);
        assert_eq!(published_file_id("\"appid\" \"1\""), None);
    }
}
