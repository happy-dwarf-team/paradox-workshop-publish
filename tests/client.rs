//! `client` mode against a fake `ISteamUGC`. Nothing here talks to Steam:
//! CI has no Steam client, and a real call would publish on someone's
//! account.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use pdx_ws::change_note::ChangeNoteInput;
use pdx_ws::client::{
    self, Created, Error, Event, ItemUpdate, LEGAL_AGREEMENT_URL, Submitted, Workshop, drift,
};
use pdx_ws::config::{Config, Visibility};
use pdx_ws::item_id::ItemId;
use pdx_ws::plan::{ModPlan, Plan};

/// What one `submit_update` got, with the published copy's descriptor read
/// while the copy still existed.
#[derive(Debug)]
struct Submission {
    app_id: u32,
    item: ItemId,
    update: ItemUpdate,
    descriptor: Option<String>,
}

#[derive(Default)]
struct FakeWorkshop {
    next_id: u64,
    needs_agreement: bool,
    fail_submit: bool,
    install_dir: Option<PathBuf>,
    created: RefCell<Vec<u32>>,
    submissions: RefCell<Vec<Submission>>,
}

impl Workshop for FakeWorkshop {
    fn create_item(&mut self, app_id: u32) -> Result<Created, Error> {
        self.created.borrow_mut().push(app_id);
        Ok(Created {
            item_id: ItemId::try_from(self.next_id).unwrap(),
            needs_agreement: self.needs_agreement,
        })
    }

    fn submit_update(
        &mut self,
        app_id: u32,
        item: ItemId,
        update: &ItemUpdate,
        events: &mut dyn FnMut(Event),
    ) -> Result<Submitted, Error> {
        if self.fail_submit {
            return Err(Error::Steam {
                action: "upload the item",
                reason: "k_EResultFail".to_owned(),
            });
        }
        events(Event::Progress {
            stage: client::Stage::UploadingContent,
            done: 50,
            total: 100,
        });
        self.submissions.borrow_mut().push(Submission {
            app_id,
            item,
            update: update.clone(),
            descriptor: fs::read_to_string(update.content.join("descriptor.mod")).ok(),
        });
        Ok(Submitted {
            needs_agreement: false,
        })
    }

    fn install_dir(&self, _app_id: u32) -> Option<PathBuf> {
        self.install_dir.clone()
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A scratch copy of a fixture, because `init` writes into the repository.
fn scratch(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture(name), dir.path());
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn load(dir: &Path) -> (Config, ModPlan) {
    let config = Config::load(&dir.join("workshop.toml")).unwrap();
    let mut plan = Plan::build(&config, &[]).unwrap();
    (config, plan.mods.remove(0))
}

fn id(value: u64) -> ItemId {
    ItemId::try_from(value).unwrap()
}

#[test]
fn init_creates_writes_back_and_uploads_a_descriptor_mod() {
    let repo = scratch("eu4-unpublished");
    let (config, plan) = load(repo.path());
    let mut workshop = FakeWorkshop {
        next_id: 3_900_000_001,
        ..FakeWorkshop::default()
    };
    let mut events = Vec::new();
    let created = client::init(
        &mut workshop,
        &config,
        &plan,
        &ChangeNoteInput::default(),
        &mut |event| events.push(event.to_string()),
    )
    .unwrap();

    assert_eq!(created.item_id, id(3_900_000_001));
    assert_eq!(created.write_back, "descriptor.mod");
    assert!(!created.needs_agreement);
    assert_eq!(*workshop.created.borrow(), [236850]);

    // The repository's descriptor got the ID, as the launcher writes it.
    let descriptor = fs::read_to_string(repo.path().join("descriptor.mod")).unwrap();
    assert!(
        descriptor.ends_with("remote_file_id=\"3900000001\"\n"),
        "{descriptor}"
    );

    let submissions = workshop.submissions.borrow();
    let [submission] = &submissions[..] else {
        panic!("one upload expected");
    };
    assert_eq!(submission.app_id, 236850);
    assert_eq!(submission.item, id(3_900_000_001));
    let update = &submission.update;
    assert_eq!(update.title, "Unpublished Fixture");
    assert_eq!(update.tags, ["Graphics"]);
    // A new item is private unless workshop.toml says otherwise.
    assert_eq!(update.visibility, Some(Visibility::Private));
    assert_eq!(update.preview, None);
    assert_eq!(update.change_note.as_deref(), Some("Version 0.1.0"));
    assert!(update.content.is_absolute());
    // The published copy's descriptor carries the new ID too.
    let copied = submission.descriptor.as_deref().unwrap();
    assert!(copied.contains("remote_file_id=\"3900000001\""), "{copied}");

    assert!(events[0].starts_with("created Workshop item 3900000001"));
    assert!(
        events
            .iter()
            .any(|event| event.contains("uploading the content: 50 of 100 bytes (50%)"))
    );
}

#[test]
fn init_writes_item_id_into_workshop_toml_for_metadata_json_games() {
    let repo = scratch("eu5");
    let (config, plan) = load(repo.path());
    let mut workshop = FakeWorkshop {
        next_id: 77,
        needs_agreement: true,
        ..FakeWorkshop::default()
    };
    let mut events = Vec::new();
    let created = client::init(
        &mut workshop,
        &config,
        &plan,
        &ChangeNoteInput::default(),
        &mut |event| events.push(event.to_string()),
    )
    .unwrap();
    assert_eq!(created.write_back, "workshop.toml");
    assert!(created.needs_agreement);
    assert!(
        events
            .iter()
            .any(|event| event.contains(LEGAL_AGREEMENT_URL)),
        "{events:?}"
    );

    let toml = fs::read_to_string(repo.path().join("workshop.toml")).unwrap();
    assert!(toml.contains("item_id = 77"), "{toml}");
    // The configured visibility wins over the private default.
    let submissions = workshop.submissions.borrow();
    assert_eq!(submissions[0].update.visibility, Some(Visibility::Private));
    assert_eq!(submissions[0].update.tags, ["Gameplay", "Map", "1.0"]);
    // The repository now plans to the new item.
    let (_, replanned) = load(repo.path());
    assert_eq!(replanned.item.id, Some(id(77)));
}

#[test]
fn a_failed_upload_after_create_keeps_the_written_id() {
    let repo = scratch("eu4-unpublished");
    let (config, plan) = load(repo.path());
    let mut workshop = FakeWorkshop {
        next_id: 9,
        fail_submit: true,
        ..FakeWorkshop::default()
    };
    let error = client::init(
        &mut workshop,
        &config,
        &plan,
        &ChangeNoteInput::default(),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(error, Error::AfterCreate { .. }), "{error}");
    assert_eq!(error.exit_code(), 1);
    let message = error.to_string();
    assert!(message.contains("Workshop item 9 exists"), "{message}");
    assert!(
        message.contains("pdx-ws publish --mode client"),
        "{message}"
    );
    let descriptor = fs::read_to_string(repo.path().join("descriptor.mod")).unwrap();
    assert!(descriptor.contains("remote_file_id=\"9\""));
}

#[test]
fn init_refuses_a_mod_that_has_an_item() {
    let (config, plan) = load(&fixture("eu4"));
    let mut workshop = FakeWorkshop::default();
    let error = client::init(
        &mut workshop,
        &config,
        &plan,
        &ChangeNoteInput::default(),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(error, Error::AlreadyExists(_)), "{error}");
    assert_eq!(error.exit_code(), 2);
    assert!(workshop.created.borrow().is_empty());
}

#[test]
fn publish_updates_the_existing_item_with_tags() {
    let (_, plan) = load(&fixture("eu4"));
    let mut workshop = FakeWorkshop::default();
    let published = client::publish(&mut workshop, &plan, &mut |_| {}).unwrap();
    assert_eq!(published.item_id, id(2948217283));
    assert!(!published.created);
    assert!(workshop.created.borrow().is_empty());

    let submissions = workshop.submissions.borrow();
    let update = &submissions[0].update;
    assert_eq!(submissions[0].item, id(2948217283));
    assert_eq!(update.title, "Fixture Mod");
    assert_eq!(update.tags, ["Gameplay", "Missions and Decisions"]);
    assert_eq!(update.visibility, Some(Visibility::Public));
    assert_eq!(update.description, plan.metadata.description);
    assert_eq!(update.change_note, plan.metadata.change_note);
    let preview = update.preview.as_deref().unwrap();
    assert!(preview.is_absolute() && preview.ends_with("thumbnail.png"));
    // The copy, not the repository, is uploaded.
    let copied = submissions[0].descriptor.as_deref().unwrap();
    assert!(!copied.lines().any(|line| line.starts_with("path=")));
}

#[test]
fn publish_needs_an_item_and_keeps_visibility_when_unset() {
    let (_, plan) = load(&fixture("eu4-unpublished"));
    let error = client::publish(&mut FakeWorkshop::default(), &plan, &mut |_| {}).unwrap_err();
    assert!(matches!(error, Error::NoItemId));
    assert_eq!(error.exit_code(), 2);

    let (_, plan) = load(&fixture("vic3"));
    let mut workshop = FakeWorkshop::default();
    client::publish(&mut workshop, &plan, &mut |_| {}).unwrap();
    assert_eq!(workshop.submissions.borrow()[0].update.visibility, None);
}

#[test]
fn one_game_per_run() {
    let config = Config::load(&fixture("multi").join("workshop.toml")).unwrap();
    let plan = Plan::build(&config, &[]).unwrap();
    let error = client::single_game(&plan).unwrap_err();
    assert_eq!(
        error.to_string(),
        "client mode publishes the mods of one game per run, and this selection has eu4, \
         hoi4; pick mods with --mod"
    );
    let plan = Plan::build(&config, &["alpha".to_owned()]).unwrap();
    assert_eq!(
        client::single_game(&plan).unwrap(),
        Some(pdx_ws::game::Game::Eu4)
    );
}

#[test]
fn the_checksum_cross_check_reads_the_installed_manifest() {
    let (_, plan) = load(&fixture("eu4"));
    let install = tempfile::tempdir().unwrap();
    let mut workshop = FakeWorkshop {
        install_dir: Some(install.path().to_owned()),
        ..FakeWorkshop::default()
    };
    let warnings = drift::cross_check(&workshop, &plan);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].starts_with("cannot read "), "{warnings:?}");

    fs::write(
        install.path().join(drift::MANIFEST),
        "directory = {\n\tname = \"common\"\n\tsub_directories = yes\n\tfile_extension = \".txt\"\n}\n",
    )
    .unwrap();
    let warnings = drift::cross_check(&workshop, &plan);
    assert!(
        warnings
            .iter()
            .all(|warning| warning.starts_with("the bundled EU4 1.37 rules count")),
        "{warnings:?}"
    );
    assert_eq!(warnings.len(), 12);

    workshop.install_dir = None;
    assert_eq!(
        drift::cross_check(&workshop, &plan),
        [
            "Europa Universalis IV is not installed, so its checksum_manifest.txt was not \
          compared with the checksum rules"
        ]
    );

    // Games without checksum rules are not checked.
    let (_, vic3) = load(&fixture("vic3"));
    assert!(drift::cross_check(&workshop, &vic3).is_empty());
}
