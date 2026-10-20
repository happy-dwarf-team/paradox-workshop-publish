//! The library behind the `pdx-ws` CLI: reading `workshop.toml`, the game
//! profiles and the mod descriptors, computing the publish plan, the
//! published copy, the description, the change note and the checksum check,
//! and publishing the selected mods (`docs/spec/v1.md`, sections 1 to 9).

pub mod bbcode;
pub mod change_note;
pub mod checksum;
pub mod config;
pub mod copy;
pub mod description;
pub mod descriptor;
pub mod error;
pub mod game;
pub mod item_id;
pub mod metadata;
pub mod mods;
pub mod plan;
pub mod publish;
pub mod steamcmd;

pub use error::Error;

/// Process exit codes of `pdx-ws` (spec section 3).
pub mod exit {
    /// Success; warnings allowed.
    pub const SUCCESS: u8 = 0;
    /// At least one mod failed to publish.
    pub const PUBLISH_FAILED: u8 = 1;
    /// A configuration or validation error before any upload.
    pub const VALIDATION: u8 = 2;
    /// The Steam session is missing or expired.
    pub const SESSION: u8 = 3;
}
