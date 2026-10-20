//! Workshop item IDs and where a mod's ID comes from.

use std::fmt;
use std::num::NonZeroU64;
use std::str::FromStr;

use serde::Deserialize;

use crate::game::DescriptorFormat;

/// A Workshop item's `publishedfileid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(try_from = "u64")]
pub struct ItemId(NonZeroU64);

impl ItemId {
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

impl TryFrom<u64> for ItemId {
    type Error = &'static str;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        NonZeroU64::new(value)
            .map(ItemId)
            .ok_or("a Workshop item ID must be a positive integer")
    }
}

impl FromStr for ItemId {
    type Err = &'static str;

    /// Parses the decimal digits of `remote_file_id`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("a Workshop item ID must be a positive integer");
        }
        let value = text
            .parse::<u64>()
            .map_err(|_| "a Workshop item ID must fit in 64 bits")?;
        ItemId::try_from(value)
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Where a mod's item ID was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemIdSource {
    /// `remote_file_id` in `descriptor.mod`.
    RemoteFileId,
    /// `item_id` in `workshop.toml`.
    WorkshopToml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedItemId {
    pub id: ItemId,
    pub source: ItemIdSource,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ItemIdError {
    #[error(
        "workshop.toml has item_id {item_id} but descriptor.mod has remote_file_id \
         {remote_file_id}; keep the ID only in descriptor.mod"
    )]
    Conflict {
        item_id: ItemId,
        remote_file_id: ItemId,
    },
    #[error(
        "item_id is for metadata.json games only; put remote_file_id=\"{0}\" in \
         descriptor.mod instead"
    )]
    NotForDescriptorMod(ItemId),
}

/// Picks the mod's item ID from its single source of truth (spec section 5):
/// `remote_file_id` in `descriptor.mod`, or `item_id` in `workshop.toml` for
/// `metadata.json` games. `None` means the item does not exist yet.
pub fn resolve(
    format: DescriptorFormat,
    remote_file_id: Option<ItemId>,
    item_id: Option<ItemId>,
) -> Result<Option<ResolvedItemId>, ItemIdError> {
    match (format, remote_file_id, item_id) {
        (DescriptorFormat::DescriptorMod, Some(remote_file_id), Some(item_id))
            if remote_file_id != item_id =>
        {
            Err(ItemIdError::Conflict {
                item_id,
                remote_file_id,
            })
        }
        (DescriptorFormat::DescriptorMod, _, Some(item_id)) => {
            Err(ItemIdError::NotForDescriptorMod(item_id))
        }
        (DescriptorFormat::DescriptorMod, remote_file_id, None) => {
            Ok(remote_file_id.map(|id| ResolvedItemId {
                id,
                source: ItemIdSource::RemoteFileId,
            }))
        }
        (DescriptorFormat::MetadataJson, _, item_id) => Ok(item_id.map(|id| ResolvedItemId {
            id,
            source: ItemIdSource::WorkshopToml,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u64) -> ItemId {
        ItemId::try_from(value).unwrap()
    }

    #[test]
    fn parses_remote_file_id_digits() {
        assert_eq!("2948217283".parse(), Ok(id(2948217283)));
        for bad in ["", "0", "-1", "12a", " 12", "99999999999999999999"] {
            assert!(bad.parse::<ItemId>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn descriptor_mod_games_use_remote_file_id() {
        let resolved = resolve(DescriptorFormat::DescriptorMod, Some(id(7)), None).unwrap();
        assert_eq!(
            resolved,
            Some(ResolvedItemId {
                id: id(7),
                source: ItemIdSource::RemoteFileId
            })
        );
        assert_eq!(
            resolve(DescriptorFormat::DescriptorMod, None, None),
            Ok(None)
        );
    }

    #[test]
    fn metadata_json_games_use_item_id() {
        let resolved = resolve(DescriptorFormat::MetadataJson, None, Some(id(9))).unwrap();
        assert_eq!(resolved.unwrap().source, ItemIdSource::WorkshopToml);
        assert_eq!(
            resolve(DescriptorFormat::MetadataJson, None, None),
            Ok(None)
        );
    }

    #[test]
    fn a_conflict_is_an_error() {
        let error = resolve(DescriptorFormat::DescriptorMod, Some(id(1)), Some(id(2))).unwrap_err();
        assert_eq!(
            error,
            ItemIdError::Conflict {
                item_id: id(2),
                remote_file_id: id(1)
            }
        );
    }

    #[test]
    fn item_id_is_rejected_for_descriptor_mod_games() {
        for remote_file_id in [None, Some(id(3))] {
            assert_eq!(
                resolve(DescriptorFormat::DescriptorMod, remote_file_id, Some(id(3))),
                Err(ItemIdError::NotForDescriptorMod(id(3)))
            );
        }
    }
}
