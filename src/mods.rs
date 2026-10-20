//! A mod as read from the repository: its config, game profile, descriptor
//! and item ID.

use std::io;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::config::{Config, ModConfig};
use crate::descriptor::{self, DescriptorMod};
use crate::game::{DescriptorFormat, GameProfile};
use crate::item_id::{self, ResolvedItemId};
use crate::metadata::{self, MetadataJson};

/// A mod read from the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mod {
    pub config: ModConfig,
    pub profile: &'static GameProfile,
    /// The mod root: [`Config::base_dir`] joined with the mod's `path`.
    pub dir: PathBuf,
    pub descriptor: Descriptor,
    /// `None` when the Workshop item does not exist yet.
    pub item_id: Option<ResolvedItemId>,
}

/// The mod's descriptor, in its game's format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Descriptor {
    DescriptorMod(DescriptorMod),
    MetadataJson(MetadataJson),
}

impl Descriptor {
    /// The Workshop title.
    pub fn name(&self) -> &str {
        match self {
            Descriptor::DescriptorMod(descriptor) => &descriptor.name,
            Descriptor::MetadataJson(metadata) => &metadata.name,
        }
    }

    /// The mod's own version, used for change notes.
    pub fn version(&self) -> Option<&str> {
        match self {
            Descriptor::DescriptorMod(descriptor) => descriptor.version.as_deref(),
            Descriptor::MetadataJson(metadata) => metadata.version.as_deref(),
        }
    }

    /// The Workshop tags, verbatim.
    pub fn tags(&self) -> &[String] {
        match self {
            Descriptor::DescriptorMod(descriptor) => &descriptor.tags,
            Descriptor::MetadataJson(metadata) => &metadata.tags,
        }
    }

    /// The supported game version (`supported_version` or
    /// `supported_game_version`).
    pub fn supported_version(&self) -> Option<&str> {
        match self {
            Descriptor::DescriptorMod(descriptor) => descriptor.supported_version.as_deref(),
            Descriptor::MetadataJson(metadata) => metadata.supported_game_version.as_deref(),
        }
    }
}

impl Mod {
    /// Reads the descriptor of the mod that `config` describes and resolves
    /// its item ID.
    pub fn load(base_dir: &Path, config: &ModConfig) -> Result<Mod, Error> {
        let profile = config.game.profile();
        let dir = base_dir.join(&config.path);
        let path = dir.join(profile.descriptor.descriptor_path());
        let bytes = std::fs::read(&path).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                Error::MissingDescriptor {
                    label: config.label(),
                    path: path.clone(),
                    game: config.game,
                    expected: profile.descriptor.descriptor_path(),
                }
            } else {
                Error::Read {
                    path: path.clone(),
                    source,
                }
            }
        })?;

        let (descriptor, remote_file_id) = match profile.descriptor {
            DescriptorFormat::DescriptorMod => {
                let descriptor = descriptor::parse(&bytes).map_err(|source| Error::Descriptor {
                    path: path.clone(),
                    source,
                })?;
                let remote_file_id = descriptor.remote_file_id;
                (Descriptor::DescriptorMod(descriptor), remote_file_id)
            }
            DescriptorFormat::MetadataJson => {
                let metadata = metadata::parse(&bytes).map_err(|source| Error::Metadata {
                    path: path.clone(),
                    source,
                })?;
                (Descriptor::MetadataJson(metadata), None)
            }
        };
        let item_id = item_id::resolve(profile.descriptor, remote_file_id, config.item_id)
            .map_err(|source| Error::ItemId {
                label: config.label(),
                source,
            })?;

        Ok(Mod {
            config: config.clone(),
            profile,
            dir,
            descriptor,
            item_id,
        })
    }
}

impl Config {
    /// Reads every mod in file order, stopping at the first error.
    pub fn load_mods(&self) -> Result<Vec<Mod>, Error> {
        self.mods
            .iter()
            .map(|config| Mod::load(&self.base_dir, config))
            .collect()
    }
}
