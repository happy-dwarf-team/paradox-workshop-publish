//! Reading the fixture mods under tests/fixtures.

use std::path::{Path, PathBuf};

use pdx_ws::Error;
use pdx_ws::config::{ChecksumSetting, Config, Visibility};
use pdx_ws::exit;
use pdx_ws::game::{DescriptorFormat, Game};
use pdx_ws::item_id::{ItemId, ItemIdError, ItemIdSource};
use pdx_ws::mods::{Descriptor, Mod};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn load(name: &str) -> Result<Vec<Mod>, Error> {
    Config::load(&fixture(name).join("workshop.toml"))?.load_mods()
}

fn load_one(name: &str) -> Mod {
    let mut mods = load(name).unwrap();
    assert_eq!(mods.len(), 1);
    mods.remove(0)
}

fn item_id(module: &Mod) -> Option<(u64, ItemIdSource)> {
    module
        .item_id
        .map(|resolved| (resolved.id.get(), resolved.source))
}

#[test]
fn eu4_mod_with_remote_file_id() {
    let module = load_one("eu4");
    assert_eq!(module.profile.app_id, 236850);
    assert_eq!(module.dir, fixture("eu4").join("."));
    assert_eq!(module.config.visibility, Some(Visibility::Public));
    assert_eq!(module.config.exclude, ["docs/**"]);
    assert_eq!(module.descriptor.name(), "Fixture Mod");
    assert_eq!(module.descriptor.version(), Some("1.4.2"));
    assert_eq!(module.descriptor.supported_version(), Some("v1.37.*"));
    assert_eq!(
        module.descriptor.tags(),
        ["Gameplay", "Missions and Decisions"]
    );
    assert_eq!(
        item_id(&module),
        Some((2948217283, ItemIdSource::RemoteFileId))
    );
    let Descriptor::DescriptorMod(descriptor) = &module.descriptor else {
        panic!("EU4 uses descriptor.mod");
    };
    assert_eq!(
        descriptor.replace_paths,
        ["history/provinces", "common/bookmarks"]
    );
    assert!(descriptor.entries.iter().any(|entry| entry.key == "path"));
}

#[test]
fn eu4_mod_without_remote_file_id() {
    let module = load_one("eu4-unpublished");
    assert_eq!(module.descriptor.name(), "Unpublished Fixture");
    assert_eq!(module.descriptor.tags(), ["Graphics"]);
    assert_eq!(module.item_id, None);
}

#[test]
fn vic3_mod_with_bom_and_item_id() {
    let module = load_one("vic3");
    assert_eq!(module.profile.app_id, 529340);
    assert_eq!(module.profile.descriptor, DescriptorFormat::MetadataJson);
    assert_eq!(module.descriptor.name(), "Vic3 Fixture");
    assert_eq!(module.descriptor.version(), Some("2.0.1"));
    assert_eq!(module.descriptor.supported_version(), Some("1.9.*"));
    assert_eq!(module.descriptor.tags(), ["Economy", "1.9"]);
    assert_eq!(
        item_id(&module),
        Some((3123456789, ItemIdSource::WorkshopToml))
    );
}

#[test]
fn eu5_mod_without_item_id() {
    let module = load_one("eu5");
    assert_eq!(module.config.game, Game::Eu5);
    assert_eq!(module.profile.app_id, 3450310);
    assert_eq!(module.descriptor.name(), "EU5 Fixture");
    assert_eq!(module.descriptor.tags(), ["Gameplay", "Map", "1.0"]);
    assert_eq!(module.item_id, None);
    let Descriptor::MetadataJson(metadata) = &module.descriptor else {
        panic!("EU5 uses metadata.json");
    };
    assert_eq!(metadata.replace_paths(), ["in_game/map_data"]);
}

#[test]
fn several_mods_with_defaults() {
    let mods = load("multi").unwrap();
    let summary: Vec<_> = mods
        .iter()
        .map(|module| {
            (
                module.config.id.as_deref().unwrap(),
                module.config.game,
                module.config.exclude.clone(),
                module.config.checksum,
                module.descriptor.name(),
                item_id(module).map(|(id, _)| id),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            (
                "alpha",
                Game::Eu4,
                vec!["docs/**".to_owned()],
                ChecksumSetting::Keep,
                "Alpha",
                Some(111),
            ),
            (
                "beta",
                Game::Hoi4,
                vec!["docs/**".to_owned(), "*.psd".to_owned()],
                ChecksumSetting::Ignore,
                "Beta",
                None,
            ),
        ]
    );
}

#[test]
fn item_id_and_remote_file_id_conflict() {
    let error = load("id-conflict").unwrap_err();
    let Error::ItemId { label, source } = &error else {
        panic!("unexpected error: {error}");
    };
    assert_eq!(label, "the mod at `.`");
    assert_eq!(
        *source,
        ItemIdError::Conflict {
            item_id: ItemId::try_from(222).unwrap(),
            remote_file_id: ItemId::try_from(111).unwrap(),
        }
    );
    assert_eq!(error.exit_code(), exit::VALIDATION);
}

#[test]
fn a_missing_descriptor_names_the_expected_file() {
    let error = load("missing-descriptor").unwrap_err();
    let message = error.to_string();
    assert!(
        matches!(error, Error::MissingDescriptor { .. }),
        "{message}"
    );
    assert!(
        message.contains("eu5 mods need .metadata/metadata.json in the mod folder"),
        "{message}"
    );
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn a_missing_config_is_a_read_error() {
    let error = Config::load(&fixture("does-not-exist").join("workshop.toml")).unwrap_err();
    assert!(matches!(error, Error::Read { .. }));
    assert_eq!(error.exit_code(), exit::VALIDATION);
}
