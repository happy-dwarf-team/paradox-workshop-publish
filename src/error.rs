//! The errors of reading a repository's mods, all found before any upload.

use std::io;
use std::path::PathBuf;

use crate::config::ConfigError;
use crate::descriptor::DescriptorError;
use crate::exit;
use crate::game::Game;
use crate::item_id::ItemIdError;
use crate::metadata::MetadataError;
use crate::plan::PlanError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read {}: {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("{}: {source}", path.display())]
    Config { path: PathBuf, source: ConfigError },
    #[error("{label}: {} not found; {game} mods need {expected} in the mod folder", path.display())]
    MissingDescriptor {
        label: String,
        path: PathBuf,
        game: Game,
        expected: &'static str,
    },
    #[error("{}: {source}", path.display())]
    Descriptor {
        path: PathBuf,
        source: DescriptorError,
    },
    #[error("{}: {source}", path.display())]
    Metadata {
        path: PathBuf,
        source: MetadataError,
    },
    #[error("{label}: {source}")]
    ItemId { label: String, source: ItemIdError },
    #[error("{0}")]
    Selection(PlanError),
    #[error("{label}: {source}")]
    Plan { label: String, source: PlanError },
}

impl Error {
    /// The `pdx-ws` exit code for this error: all of them are configuration
    /// or validation errors before any upload.
    pub fn exit_code(&self) -> u8 {
        exit::VALIDATION
    }
}
