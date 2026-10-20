//! Game profiles: what publishing needs to know about each supported game.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A game that `pdx-ws` can publish mods for, named by its `workshop.toml` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Game {
    Eu4,
    Hoi4,
    Stellaris,
    Imperator,
    Ck3,
    Vic3,
    Eu5,
}

/// Where a game keeps its mod descriptor, and with it the preview image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorFormat {
    /// Clausewitz script in `descriptor.mod` in the mod root.
    DescriptorMod,
    /// JSON in `.metadata/metadata.json`.
    MetadataJson,
}

impl DescriptorFormat {
    /// The descriptor's path relative to the mod root.
    pub fn descriptor_path(self) -> &'static str {
        match self {
            DescriptorFormat::DescriptorMod => "descriptor.mod",
            DescriptorFormat::MetadataJson => ".metadata/metadata.json",
        }
    }

    /// The Workshop preview image's path relative to the mod root.
    pub fn thumbnail_path(self) -> &'static str {
        match self {
            DescriptorFormat::DescriptorMod => "thumbnail.png",
            DescriptorFormat::MetadataJson => ".metadata/thumbnail.png",
        }
    }
}

/// What publishing needs to know about one game.
#[derive(Debug, PartialEq, Eq)]
pub struct GameProfile {
    pub game: Game,
    /// The `game` value in `workshop.toml`.
    pub key: &'static str,
    pub name: &'static str,
    pub app_id: u32,
    pub descriptor: DescriptorFormat,
    /// The launcher's documented tag limit, if any.
    pub tag_limit: Option<usize>,
}

const PROFILES: [GameProfile; 7] = [
    GameProfile {
        game: Game::Eu4,
        key: "eu4",
        name: "Europa Universalis IV",
        app_id: 236850,
        descriptor: DescriptorFormat::DescriptorMod,
        tag_limit: None,
    },
    GameProfile {
        game: Game::Hoi4,
        key: "hoi4",
        name: "Hearts of Iron IV",
        app_id: 394360,
        descriptor: DescriptorFormat::DescriptorMod,
        tag_limit: None,
    },
    GameProfile {
        game: Game::Stellaris,
        key: "stellaris",
        name: "Stellaris",
        app_id: 281990,
        descriptor: DescriptorFormat::DescriptorMod,
        tag_limit: Some(10),
    },
    GameProfile {
        game: Game::Imperator,
        key: "imperator",
        name: "Imperator: Rome",
        app_id: 859580,
        descriptor: DescriptorFormat::DescriptorMod,
        tag_limit: None,
    },
    GameProfile {
        game: Game::Ck3,
        key: "ck3",
        name: "Crusader Kings III",
        app_id: 1158310,
        descriptor: DescriptorFormat::DescriptorMod,
        tag_limit: None,
    },
    GameProfile {
        game: Game::Vic3,
        key: "vic3",
        name: "Victoria 3",
        app_id: 529340,
        descriptor: DescriptorFormat::MetadataJson,
        tag_limit: Some(5),
    },
    GameProfile {
        game: Game::Eu5,
        key: "eu5",
        name: "Europa Universalis V",
        app_id: 3450310,
        descriptor: DescriptorFormat::MetadataJson,
        tag_limit: None,
    },
];

impl Game {
    /// Every supported game, in the spec's order.
    pub const ALL: [Game; 7] = [
        Game::Eu4,
        Game::Hoi4,
        Game::Stellaris,
        Game::Imperator,
        Game::Ck3,
        Game::Vic3,
        Game::Eu5,
    ];

    pub fn profile(self) -> &'static GameProfile {
        &PROFILES[self as usize]
    }

    /// The `game` value in `workshop.toml`.
    pub fn key(self) -> &'static str {
        self.profile().key
    }
}

impl fmt::Display for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

/// A `game` value that is not a supported game.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GameError {
    #[error("CK2 uses legacy single-file Workshop items, which pdx-ws does not support")]
    Ck2,
    #[error("unknown game `{0}`; expected one of eu4, hoi4, stellaris, imperator, ck3, vic3, eu5")]
    Unknown(String),
}

impl FromStr for Game {
    type Err = GameError;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        if key == "ck2" {
            return Err(GameError::Ck2);
        }
        Game::ALL
            .into_iter()
            .find(|game| game.key() == key)
            .ok_or_else(|| GameError::Unknown(key.to_owned()))
    }
}

impl<'de> Deserialize<'de> for Game {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let key = String::deserialize(deserializer)?;
        key.parse().map_err(serde::de::Error::custom)
    }
}

impl Serialize for Game {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_are_indexed_by_game() {
        for game in Game::ALL {
            assert_eq!(game.profile().game, game);
        }
    }

    #[test]
    fn keys_round_trip() {
        for game in Game::ALL {
            assert_eq!(game.key().parse::<Game>(), Ok(game));
        }
    }

    #[test]
    fn app_ids_and_formats_match_the_spec() {
        let table: Vec<_> = Game::ALL
            .into_iter()
            .map(|game| {
                let profile = game.profile();
                (profile.key, profile.app_id, profile.descriptor)
            })
            .collect();
        use DescriptorFormat::*;
        assert_eq!(
            table,
            [
                ("eu4", 236850, DescriptorMod),
                ("hoi4", 394360, DescriptorMod),
                ("stellaris", 281990, DescriptorMod),
                ("imperator", 859580, DescriptorMod),
                ("ck3", 1158310, DescriptorMod),
                ("vic3", 529340, MetadataJson),
                ("eu5", 3450310, MetadataJson),
            ]
        );
    }

    #[test]
    fn ck2_is_rejected_with_an_explanation() {
        let error = "ck2".parse::<Game>().unwrap_err();
        assert_eq!(
            error.to_string(),
            "CK2 uses legacy single-file Workshop items, which pdx-ws does not support"
        );
    }

    #[test]
    fn unknown_games_list_the_supported_ones() {
        let error = "EU4".parse::<Game>().unwrap_err();
        assert_eq!(error, GameError::Unknown("EU4".to_owned()));
        assert!(error.to_string().contains("eu4, hoi4"));
    }

    #[test]
    fn thumbnails_follow_the_descriptor_format() {
        assert_eq!(
            Game::Eu4.profile().descriptor.thumbnail_path(),
            "thumbnail.png"
        );
        assert_eq!(
            Game::Eu5.profile().descriptor.thumbnail_path(),
            ".metadata/thumbnail.png"
        );
    }
}
