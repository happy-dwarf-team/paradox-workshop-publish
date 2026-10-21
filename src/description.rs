//! The Workshop description from the file `description` names (spec
//! section 7).

use std::fs;
use std::path::Path;

use crate::bbcode;
use crate::plan::PlanError;

/// Steam's limit for a description and a change note, in bytes of UTF-8
/// (`k_cchPublishedDocumentDescriptionMax`). Never more than the number of
/// characters, so it is the safe reading of "8000 characters".
pub const TEXT_LIMIT: usize = 8000;

/// Reads the description file at `file` (shown as `path`): `.md` is
/// converted to BBCode and anything else (`.bbcode`) is taken verbatim.
pub fn load(file: &Path, path: &str) -> Result<String, PlanError> {
    let text = read_text(file, path, "description")?;
    let bbcode = if file.extension().is_some_and(|extension| extension == "md") {
        bbcode::to_bbcode(&text).map_err(|source| PlanError::Markdown {
            path: path.to_owned(),
            source,
        })?
    } else {
        text
    };
    check_limit("description", &bbcode)?;
    Ok(bbcode)
}

/// A UTF-8 text file without its BOM.
pub(crate) fn read_text(file: &Path, path: &str, what: &'static str) -> Result<String, PlanError> {
    let bytes = fs::read(file).map_err(|source| PlanError::ReadText {
        what,
        path: path.to_owned(),
        source,
    })?;
    let text = String::from_utf8(bytes).map_err(|_| PlanError::TextNotUtf8 {
        what,
        path: path.to_owned(),
    })?;
    Ok(match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_owned(),
        None => text,
    })
}

pub(crate) fn check_limit(what: &'static str, bbcode: &str) -> Result<(), PlanError> {
    if bbcode.len() > TEXT_LIMIT {
        return Err(PlanError::TextTooLong {
            what,
            size: bbcode.len(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(name: &str, contents: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(name);
        fs::write(&file, contents).unwrap();
        (dir, file)
    }

    #[test]
    fn markdown_is_converted() {
        let (_dir, file) = write("d.md", "\u{feff}# Title\n\n**Bold**\n");
        assert_eq!(load(&file, "d.md").unwrap(), "[h1]Title[/h1]\n[b]Bold[/b]");
    }

    #[test]
    fn bbcode_is_verbatim() {
        let (_dir, file) = write("d.bbcode", "[h1]Title[/h1]\n<not html>\n");
        assert_eq!(
            load(&file, "d.bbcode").unwrap(),
            "[h1]Title[/h1]\n<not html>\n"
        );
    }

    #[test]
    fn markdown_errors_name_the_file_and_line() {
        let (_dir, file) = write("d.md", "Intro\n\n<img src=\"a.png\">\n");
        assert_eq!(
            load(&file, "workshop/d.md").unwrap_err().to_string(),
            "workshop/d.md, line 3: raw HTML is not supported; use Markdown only"
        );
    }

    #[test]
    fn the_limit_applies_after_conversion() {
        // 7997 bytes of Markdown, 8000 after conversion.
        let fits = format!("**{}**", "a".repeat(7993));
        let (_dir, file) = write("d.md", &fits);
        assert_eq!(load(&file, "d.md").unwrap().len(), 8000);

        let over = format!("**{}**", "a".repeat(7994));
        fs::write(&file, over).unwrap();
        assert_eq!(
            load(&file, "d.md").unwrap_err().to_string(),
            "the description is 8001 bytes after conversion to BBCode; \
             Steam takes at most 8000"
        );
    }

    #[test]
    fn a_missing_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let error = load(&dir.path().join("none.md"), "none.md").unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("cannot read the description none.md: "),
            "{error}"
        );
    }
}
