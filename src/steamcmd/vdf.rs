//! The `workshop_build_item` VDF (spec section 9).

use std::fmt::Write;
use std::path::PathBuf;

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

/// Escapes `\` and `"` for a quoted VDF value.
pub fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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
    pub fn render(&self) -> String {
        let mut fields: Vec<(&str, String)> = vec![
            ("appid", self.app_id.to_string()),
            ("publishedfileid", self.published_file_id.to_string()),
            (
                "contentfolder",
                self.content_folder.to_string_lossy().into_owned(),
            ),
        ];
        if let Some(preview) = &self.preview_file {
            fields.push(("previewfile", preview.to_string_lossy().into_owned()));
        }
        if let Some(title) = &self.title {
            fields.push(("title", title.clone()));
        }
        if let Some(description) = &self.description {
            fields.push(("description", description.clone()));
        }
        if let Some(visibility) = self.visibility {
            fields.push(("visibility", visibility_code(visibility).to_string()));
        }
        if let Some(change_note) = &self.change_note {
            fields.push(("changenote", change_note.clone()));
        }

        let mut text = String::from("\"workshopitem\"\n{\n");
        for (key, value) in fields {
            let _ = writeln!(text, "\t\"{key}\" \"{}\"", escape(&value));
        }
        text.push_str("}\n");
        text
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
            vdf.render(),
            "\"workshopitem\"\n{\n\t\"appid\" \"236850\"\n\t\"publishedfileid\" \"42\"\n\t\"contentfolder\" \"D:\\\\a\\\\_temp\\\\copy\"\n\t\"changenote\" \"Version 1.2\"\n}\n"
        );
    }

    #[test]
    fn renders_every_key_with_escapes() {
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
        let text = vdf.render();
        assert!(text.contains("\t\"publishedfileid\" \"0\"\n"));
        assert!(text.contains("\t\"previewfile\" \"/repo/.metadata/thumbnail.png\"\n"));
        assert!(text.contains("\t\"title\" \"The \\\"Best\\\" Mod\"\n"));
        assert!(text.contains("\t\"description\" \"[b]bold[/b]\nline two\"\n"));
        assert!(text.contains("\t\"visibility\" \"2\"\n"));
        assert!(!text.contains("changenote"));
    }

    #[test]
    fn reads_the_written_back_id() {
        let text = "\"workshopitem\"\n{\n\t\"appid\"\t\t\"236850\"\n\t\"publishedfileid\"\t\t\"3823257732\"\n}\n";
        assert_eq!(published_file_id(text), Some(3823257732));
        assert_eq!(published_file_id("\"publishedfileid\" \"0\""), None);
        assert_eq!(published_file_id("\"appid\" \"1\""), None);
    }
}
