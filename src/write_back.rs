//! Writing a new Workshop item's ID back into the repository (spec section
//! 5): `remote_file_id` in `descriptor.mod`, or `item_id` in `workshop.toml`
//! for `metadata.json` games. Only `init` does this; the modder commits it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::descriptor;
use crate::game::DescriptorFormat;
use crate::item_id::ItemId;
use crate::plan::ModPlan;

/// Writing the ID back failed.
#[derive(Debug, thiserror::Error)]
#[error("cannot write the item ID into {}: {source}", path.display())]
pub struct WriteBackError {
    pub path: PathBuf,
    pub source: io::Error,
}

/// Writes `id` into the file the mod keeps its item ID in and returns that
/// file, relative to the folder of `workshop.toml` when it can.
pub fn item_id(config: &Config, plan: &ModPlan, id: ItemId) -> Result<String, WriteBackError> {
    let path = match plan.game.profile().descriptor {
        DescriptorFormat::DescriptorMod => {
            let path = plan
                .dir
                .join(DescriptorFormat::DescriptorMod.descriptor_path());
            let text = read(&path)?;
            write(&path, &descriptor::append_remote_file_id(&text, id))?;
            path
        }
        DescriptorFormat::MetadataJson => {
            let path = config.file.clone();
            let text = read(&path)?;
            let updated =
                with_item_id(&text, plan.id.as_deref(), id).map_err(|source| WriteBackError {
                    path: path.clone(),
                    source,
                })?;
            write(&path, &updated)?;
            path
        }
    };
    Ok(display(&path, &config.base_dir))
}

/// `workshop.toml` text with `item_id = <id>` set in the `[[mod]]` table
/// whose `id` is `mod_id` (or the only one), keeping comments and layout.
pub fn with_item_id(text: &str, mod_id: Option<&str>, id: ItemId) -> io::Result<String> {
    let mut document: toml_edit::DocumentMut = text.parse().map_err(io::Error::other)?;
    let mods = document
        .get_mut("mod")
        .and_then(toml_edit::Item::as_array_of_tables_mut)
        .ok_or_else(|| io::Error::other("there is no [[mod]] table"))?;
    let table = match mod_id {
        Some(mod_id) => mods
            .iter_mut()
            .find(|table| table.get("id").and_then(toml_edit::Item::as_str) == Some(mod_id)),
        None if mods.len() == 1 => mods.get_mut(0),
        None => None,
    }
    .ok_or_else(|| io::Error::other("cannot find the mod's [[mod]] table"))?;
    let value = i64::try_from(id.get()).map_err(io::Error::other)?;
    table["item_id"] = toml_edit::value(value);
    Ok(document.to_string())
}

fn read(path: &Path) -> Result<String, WriteBackError> {
    fs::read_to_string(path).map_err(|source| WriteBackError {
        path: path.to_owned(),
        source,
    })
}

fn write(path: &Path, text: &str) -> Result<(), WriteBackError> {
    fs::write(path, text).map_err(|source| WriteBackError {
        path: path.to_owned(),
        source,
    })
}

fn display(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u64) -> ItemId {
        ItemId::try_from(value).unwrap()
    }

    #[test]
    fn sets_item_id_in_the_only_mod() {
        let text = "schema = 1\n\n# The Victoria 3 mod.\n[[mod]]\ngame = \"vic3\" # inline\n";
        assert_eq!(
            with_item_id(text, None, id(42)).unwrap(),
            "schema = 1\n\n# The Victoria 3 mod.\n[[mod]]\ngame = \"vic3\" # inline\nitem_id = 42\n"
        );
    }

    #[test]
    fn sets_item_id_in_the_mod_with_the_id() {
        let text = "schema = 1\n[[mod]]\nid = \"a\"\ngame = \"vic3\"\n[[mod]]\nid = \"b\"\ngame = \"eu5\"\n";
        let updated = with_item_id(text, Some("b"), id(7)).unwrap();
        assert_eq!(
            updated,
            "schema = 1\n[[mod]]\nid = \"a\"\ngame = \"vic3\"\n[[mod]]\nid = \"b\"\ngame = \"eu5\"\nitem_id = 7\n"
        );
        assert!(with_item_id(text, None, id(7)).is_err());
        assert!(with_item_id(text, Some("c"), id(7)).is_err());
    }

    #[test]
    fn appends_remote_file_id_in_the_file_line_ending() {
        assert_eq!(
            descriptor::append_remote_file_id("name=\"A\"\r\n", id(5)),
            "name=\"A\"\r\nremote_file_id=\"5\"\r\n"
        );
        assert_eq!(
            descriptor::append_remote_file_id("name=\"A\"", id(5)),
            "name=\"A\"\nremote_file_id=\"5\"\n"
        );
    }
}
