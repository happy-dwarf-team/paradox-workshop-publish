# Mod metadata across Paradox games

Research for [#4](https://github.com/happy-dwarf-team/paradox-workshop-publish/issues/4) (part of the map [#1](https://github.com/happy-dwarf-team/paradox-workshop-publish/issues/1)). Checked 2026-10-20.

It covers, for each Paradox game with a Steam Workshop: the Steam app ID, the descriptor format and its fields, the thumbnail, the Workshop tags, what the uploader writes back after publishing, and how a downloaded Workshop item becomes a playable mod. EU4 is the reference game, and the map's evidence E2 and E3 are the starting point.

Each claim carries a label:

- **Verified**: observed directly, either in the local EU4 install or through a public Steam API response.
- **Documented**: stated by an official wiki or Valve's documentation, and not re-checked here.
- **Code**: read from the source of an established mod manager.
- **Inferred**: a conclusion drawn from the labelled facts it rests on.
- **Unknown**: not established.

## Per-game table

| Game | Steam app ID | Descriptor | Thumbnail used as the Workshop preview | Workshop tags (browse filter) | Written back after publishing | Downloaded item → playable mod |
|---|---|---|---|---|---|---|
| **Europa Universalis IV** | 236850 | `descriptor.mod` (Clausewitz script) in the mod root, plus a `<name>.mod` with `path` in `<Documents>/…/mod` | `thumbnail.png` in the mod root, under 1 MB. Steam stores it byte-identical (Verified in 50 of 51 items) | 21 tags plus `Converted from CKII`, with Steam's own casing (`events`, `graphics`, `trade`, `Missions and Decisions`) | `remote_file_id="<id>"` added to `descriptor.mod` and to the outer `.mod` (Verified, E2); also `remoteSteamId` in `launcher-v2.sqlite` (Verified) | The launcher writes `mod/ugc_<id>.mod`: the item's descriptor with `path` pointing to `workshop/content/236850/<id>`; `dlc_load.json` lists `mod/ugc_<id>.mod` (Verified) |
| **Hearts of Iron IV** | 394360 | `descriptor.mod` plus `<name>.mod` (same syntax as EU4) | `thumbnail.png` in the mod root, PNG, under 1 MB, 1:1 recommended (Documented) | 15 tags: Alternative History, Balance, Events, Fixes, Gameplay, Graphics, Historical, Ideologies, Map, Military, National Focuses, Sound, Technologies, Translation, Utilities | `remote_file_id` (Documented) | `.mod` named after the ID in `mod/`, content in `workshop/content/394360/<id>` (Documented) |
| **Stellaris** | 281990 | `descriptor.mod` plus `<name>.mod`; falls back to `<name>.mod` if no descriptor (Documented) | `thumbnail.png` in the mod root, under 1 MB, at least 512×512 recommended (Documented) | 21 category tags plus 4 legacy version tags (`3.0.8`–`3.0.11`); the launcher caps an item at 10 tags (Documented) | `remote_file_id`, "added by the launcher" (Documented) | Same as EU4: `ugc_<id>.mod` in `mod/` (Code: Irony) |
| **Crusader Kings II** | 203770 | `<name>.mod` (Clausewitz script, with `archive`/`path`, `user_dir`, `picture`) (Documented) | `picture`, shown at 268×268 JPEG (Documented) | **No declared mod tags.** Steam declares only guide tags for app 203770 (Verified) | Unknown | **Legacy single-file items**: in 28 of 28 sampled items the Workshop file is `mod/<name>.zip`, including 2026 uploads (Verified). Not a folder-based `ISteamUGC` item |
| **Imperator: Rome** | 859580 | `descriptor.mod` plus `<name>.mod` (Code: Irony; Documented) | `thumbnail.png` in the mod folder, at least 200×200 (Documented) | 11 tags: Characters, Flags, Historical, Interface, Ironman Compatible, Overhaul, Portraits, Silly, Total Conversions, Unit Graphics, Unit Types | The first upload creates a `descriptor.mod` that "links to your Steam mod" (Documented); the key is presumably `remote_file_id` (Inferred) | `ugc_<id>.mod` (Code: Irony) |
| **Crusader Kings III** | 1158310 | **`descriptor.mod`** plus `<name>.mod`, **not** `metadata.json` (Code: Irony current master; Documented for 1.1) | `thumbnail.png`; "Steam ignores [`picture`] and always looks for thumbnail.png" (Documented) | 20 tags: Alternative History, Balance, Character Focuses, Character Interactions, Culture, Decisions, Events, Fixes, Gameplay, Graphics, Historical, Map, Religion, Sound, Total Conversion, Translation, Utilities, Warfare, Interface, Submod; plus a "Compatible Version" group (1.15–1.20, Older) | `remote_file_id`, "set automatically on upload" (Documented) | `ugc_<id>.mod` (Code: Irony) |
| **Victoria 3** | 529340 | `.metadata/metadata.json` (Documented; Code) | `.metadata/thumbnail.png` (Documented); Steam's 1 MB limit applies | 24 tags (list below) plus an "Update Compatibility" group (Deprecated, 1.9–1.13); the wiki allows at most 5 tags | **No Steam ID field** in 7 of 7 sampled published `metadata.json` files (Verified on GitHub). Where the launcher keeps the link is Unknown; `launcher-v2.sqlite` has a `remoteSteamId` column (Verified schema) | No `ugc_` file. The game loads mods by folder path from `content_load.json` (Code: Irony) |
| **Europa Universalis V** | 3450310 | `.metadata/metadata.json` (Documented); "JsonMetadataV2" with no Paradox launcher (Code: Irony) | `.metadata/thumbnail.png`, under 1 MB, 512×512 recommended (Documented) | 26 tags (list below), no version group | **No Steam ID field** in 8 of 8 sampled `metadata.json` files (Verified on GitHub); the uploader is in the game's own mod tools (Documented). Where it keeps the link is Unknown | No `ugc_` file. Mods are enabled through `playsets.json` (Code: Irony) |

Notes on the table:

- **CK3 contradicts the map.** The map's Notes put CK3 in the `metadata.json` profile, but every source checked here says CK3 still uses `descriptor.mod`.
- **CK2 is a different model.** Its Workshop items are single zip files (the legacy Remote Storage style), not content folders.

Paths in this document use `<Documents>` for the user's Documents folder and `<Steam>` for the Steam library. No private paths are reproduced.

## Sources

Shorthand used below:

- **EU4 wiki**: <https://eu4.paradoxwikis.com/Mod_structure>
- **HOI4 wiki**: <https://hoi4.paradoxwikis.com/Mod_structure>
- **Stellaris wiki**: <https://stellaris.paradoxwikis.com/Modding>
- **CK2 wiki**: <https://ck2.paradoxwikis.com/Modding>
- **Imperator wiki**: <https://imperator.paradoxwikis.com/Modding>
- **CK3 wiki**: <https://ck3.paradoxwikis.com/Mod_structure> (marked "verified for 1.1", last edited October 2023)
- **Vic3 wiki**: <https://vic3.paradoxwikis.com/Mod_structure>
- **EU5 wiki**: <https://eu5.paradoxwikis.com/Mod_structure> and <https://eu5.paradoxwikis.com/Modding>
- **Workshop browse**: `https://steamcommunity.com/workshop/browse/?appid=<appid>&section=readytouseitems`. The tag lists come from the `declaredTags.readytouse_tags` object embedded in each page, which drives the "Browse by tag" filter.
- **Workshop hub**: `https://steamcommunity.com/app/<appid>/workshop/`. The page title names the game for each app ID. Store pages at `https://store.steampowered.com/app/<appid>/` agree for all eight.
- **SteamDB** (`https://steamdb.info/app/<appid>/`) returned HTTP 403 to scripted requests, so the app IDs are confirmed through Steam's own pages and Irony instead.
- **Steam API**: `https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/`. A public call that needs no key and returns tags, `preview_url` and `filename` per item.
- **ISteamUGC**: <https://partner.steamgames.com/doc/api/ISteamUGC>
- **Irony**: Irony Mod Manager at commit [`6a8861d`](https://github.com/bcssov/IronyModManager/tree/6a8861d734382a9e98f7394ea404fac7a2b884b3). Links below point at exact lines.
- **PWM**: kaiser-chris/pdx-workshop-manager at commit [`c32792c`](https://github.com/kaiser-chris/pdx-workshop-manager/tree/c32792c61068419565d092766d2f67d79cea2b91). It is prior art: an `ISteamUGC` uploader for Victoria 3 and EU5.
- **Local EU4 install**: EU4 1.37.5 on Steam, read-only. It covers the game folder, `<Documents>/Paradox Interactive/Europa Universalis IV/mod` (4 local and 51 `ugc_*.mod` files), `<Steam>/steamapps/workshop/content/236850` (51 subscribed items), and the launcher database `launcher-v2.sqlite`, opened read-only and immutable.

## 1. Steam app IDs

| Game | App ID | Evidence |
|---|---|---|
| Europa Universalis IV | 236850 | Workshop hub and store titles (Verified); Irony [`Constants.cs` L345](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Shared/Constants.cs#L345); local EU4 install (Workshop folder `content/236850`) |
| Hearts of Iron IV | 394360 | Workshop hub and store (Verified); Irony [L471](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Shared/Constants.cs#L471); HOI4 wiki (`workshop/content/394360`) |
| Stellaris | 281990 | Workshop hub and store (Verified); Irony [L586](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Shared/Constants.cs#L586); Stellaris wiki |
| Crusader Kings II | 203770 | Workshop hub and store (Verified) |
| Imperator: Rome | 859580 | Workshop hub and store (Verified); Irony [L521](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Shared/Constants.cs#L521) |
| Crusader Kings III | 1158310 | Workshop hub and store (Verified); Irony [L281](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Shared/Constants.cs#L281) |
| Victoria 3 | 529340 | Workshop hub and store (Verified); Irony [L684](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Shared/Constants.cs#L684); Vic3 wiki (`workshop/content/529340`) |
| Europa Universalis V | 3450310 | Workshop hub and store (Verified); Irony [L393](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Shared/Constants.cs#L393); EU5 wiki (`workshop/content/3450310`) |

The consumer app ID of a Workshop item equals the game's app ID. In the Steam API, `creator_app_id` and `consumer_app_id` are both 236850 for EU4 items (Verified).

## 2. Descriptor formats

### 2a. `descriptor.mod` family: EU4, HOI4, Stellaris, Imperator, CK3

Irony assigns `ModDescriptorType.DescriptorMod` to each of these games:

- [CK3 L104](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Services/Registrations/GameRegistration.cs#L104)
- [EU4 L139](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Services/Registrations/GameRegistration.cs#L139)
- [HOI4 L285](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Services/Registrations/GameRegistration.cs#L285)
- [Imperator L321](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Services/Registrations/GameRegistration.cs#L321)
- [Stellaris L356](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Services/Registrations/GameRegistration.cs#L356)

Irony does not support CK2.

**Two files (Documented: EU4, HOI4, Stellaris and CK3 wikis):**

- **`descriptor.mod`** sits inside the mod folder and is the file that gets published. It carries the same keys as the outer file, minus `path`.
- **`<anything>.mod`** sits in `<Documents>/Paradox Interactive/<Game>/mod/` and is for local use. Its `path` points at the mod folder, either relative to the game's user folder or absolute.

HOI4 adds two rules: the `.mod` file "is not uploaded" to the Workshop, and `descriptor.mod` must not be UTF-8 with a BOM (HOI4 wiki). The local EU4 install backs the BOM rule: none of the 42 shipped `descriptor.mod` files starts with a BOM, and none uses CRLF line endings.

**Keys** (Clausewitz script, `key="value"`, lists as `key={ "a" "b" }`):

| Key | Meaning | Sources |
|---|---|---|
| `name` | Display name; the launcher uses it as the Workshop title | all wikis; Irony [`ModParser.cs` L120](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Parser/Mod/ModParser.cs#L120) |
| `version` | The mod's own version, as free text | HOI4, Stellaris and CK3 wikis |
| `supported_version` | The supported game version, with `*` wildcards (e.g. `v1.37.*`) | EU4, HOI4, Stellaris and CK3 wikis |
| `tags` | A list of Workshop tags | all wikis; Irony L122 |
| `picture` | The thumbnail file name. It is optional, and in practice `thumbnail.png` is what gets used (section 3) | all wikis |
| `dependencies` | Names of mods to load first (soft dependency) | EU4, HOI4 and Stellaris wikis; Irony L124 |
| `replace_path` | Repeated key; stops vanilla files in that folder from loading | EU4 and CK3 wikis; Irony L111 |
| `path` | Mod folder. Only in the outer `.mod`; the wikis say to leave it out of `descriptor.mod` | EU4, Stellaris and CK3 wikis |
| `archive` | A zip instead of a folder (legacy; HOI4 before 1.9, CK2) | HOI4 and CK2 wikis; Irony L113 |
| `user_dir` | Separate save and temp folder (CK2, Irony) | CK2 wiki; Irony L112 |
| `remote_file_id` | The Workshop item ID (`publishedfileid`), written by the launcher | HOI4, Stellaris and CK3 wikis; Irony L123; E2 |

The local EU4 install shows which keys appear in practice. Of the 55 `.mod` files in the user mod folder, the number containing each key is:

- `name`, `path`, `supported_version` and `tags`: 55
- `remote_file_id`: 52
- `version`: 49
- `picture`: 38
- `dependencies`: 30
- `replace_path`: 5 files, with 70 lines between them

**Shipped descriptors can leak the uploader's private path (Verified).** Of the 51 EU4 items, 14 ship a `.mod` file whose `path=` holds the uploader's absolute local path, including a Windows user name. Nine items have no `descriptor.mod` at all and ship only a `<name>.mod`.

The launcher accepts both cases. It falls back to "any `*.mod`" when `descriptor.mod` is missing, as described in the Stellaris wiki, implemented in Irony [`ModService.cs` L803–L811](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Services/ModService.cs#L803-L811), and verified in the local EU4 install.

### 2b. CK3 is `descriptor.mod`, not `metadata.json`

- The CK3 wiki describes only `.mod` plus `descriptor.mod`, with `version`, `tags`, `name`, `supported_version`, `path`, `remote_file_id`, `picture` and `replace_path`. It never mentions `.metadata` (Documented, page verified for 1.1).
- Irony's current master still maps CK3 to `DescriptorMod` ([L104](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Services/Registrations/GameRegistration.cs#L104)) (Code).
- No source checked here shows CK3 moving to `metadata.json`. **CK3 belongs to the `descriptor.mod` profile** (Inferred).
- **Unknown:** whether a recent CK3 patch (1.18–1.20) changed this. A CK3 Workshop item downloaded today would settle it.

### 2c. `.metadata/metadata.json` family: Victoria 3, EU5

Irony maps Victoria 3 to `JsonMetadata` ([L426](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Services/Registrations/GameRegistration.cs#L426)) and EU5 to `JsonMetadataV2` ([L175](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Services/Registrations/GameRegistration.cs#L175)). The parser comments that V2 "has not changed but they dropped pdx launcher in EU5" ([`ModParser.cs` L82](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Parser/Mod/ModParser.cs#L82)) (Code). EU5's Paradox game ID and launcher settings are empty in Irony ([`Constants.cs` L426–L436](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Shared/Constants.cs#L426-L436)). The EU5 wiki describes in-game mod tools rather than a launcher (Documented).

Both games require `<mod>/.metadata/metadata.json`. Without it the game does not load the mod (Vic3 and EU5 wikis). Fields (Vic3 and EU5 wikis; Irony [`ModParser.cs` L259–L316](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Parser/Mod/ModParser.cs#L259-L316)):

| Field | Meaning |
|---|---|
| `name` | Display name |
| `id` | Stable unique ID chosen by the modder (Vic3 suggests reverse-domain). Other mods reference it, so it must not change |
| `version` | The mod's version |
| `game_id` | `"victoria3"` or `"eu5"` (in the Vic3 wiki; seen in all EU5 samples) |
| `supported_game_version` | Game version, with `*` (and `+` on Vic3) |
| `short_description` | Short text. On EU5 it is "used as initial description in Steam upload" (EU5 wiki) |
| `tags` | A JSON array. Vic3 wiki: at most 5, and "local" tags do not guarantee backend listing |
| `relationships` | `[{rel_type, id, display_name, resource_type, version}]`; `rel_type` is `dependency` (Vic3 also tests `incompatible_with`, `load_before`, `load_after`) |
| `game_custom_data` | `multiplayer_synchronized` (Vic3); `replace_paths` (EU5; also seen on Vic3) |
| `picture` | Not in either wiki, but present in 4 of 7 Vic3 and 3 of 8 EU5 sampled files, usually `"thumbnail.png"` (Verified on GitHub, see section 5) |

Mod-specific extra keys also occur, for example `cmf_last_updated_game_version` in EU5 samples, so a parser must ignore unknown keys (Verified on GitHub). PWM strips an optional UTF-8 BOM before parsing ([`manager.go` L98–L108](https://github.com/kaiser-chris/pdx-workshop-manager/blob/c32792c61068419565d092766d2f67d79cea2b91/manager/manager.go#L98-L108)) (Code).

### 2d. CK2: `.mod` file and legacy zip items

- The CK2 wiki documents `name`, `path`, `archive`, `user_dir`, `replace_path`, `dependencies`, `picture`, `tags` and `supported_checksums`. It says Workshop downloads use `archive`, while an upload "must use the path structure". It does not mention `descriptor.mod` or `remote_file_id` (Documented, page verified for 3.2).
- Every sampled CK2 Workshop item (28 of 28, the newest updated 2026-10-18) has `filename` = `mod/<name>.zip` in the Steam API response (Verified). EU4, HOI4, Stellaris, Imperator, CK3, Vic3 and EU5 items have an empty `filename` (30 of 30 per game; 2 of 2 for CK3), which is what folder-based `ISteamUGC` content returns.
- So CK2 items are single zip files uploaded through the older API, not content folders (Inferred). Publishing them would need a different code path.

## 3. Thumbnails

**Steam limit (Documented, ISteamUGC):**

- `SubmitItemUpdate` fails with `k_EResultLimitExceeded` when "the preview image is too large, it must be less than 1 Megabyte".
- Suggested formats are JPG, PNG and GIF.
- Preview images count against the user's Steam Cloud quota.

**EU4: preview = `thumbnail.png`, byte-identical (Verified, local EU4 install plus Steam API).** Each of the 51 subscribed items was checked by comparing the SHA-256 of `preview_url` with the images in the item folder:

- In 50 items the Steam preview equals the folder's `thumbnail.png`. This includes items whose descriptor has no `picture` key, and two items whose file is named `Thumbnail.png`.
- In the remaining item, created in 2014, the preview equals the file named by `picture="100_Loadingscreens.jpg"`.
- The thumbnail is also left inside the published content folder.
- Observed sizes range from 412×307 to 1024×1024, mostly 512×512 or 594×594, with 3 of 51 not square. All are PNG except that one JPEG, and all are under 1 MB (largest 990,807 bytes).

**Per game:**

| Game | Rule | Source |
|---|---|---|
| EU4 | `thumbnail.png` in the mod folder since the 1.29 launcher, "1 MB or smaller" | EU4 wiki; Verified above |
| HOI4 | `thumbnail.png`, PNG, under 1 MB (or it cannot go to Paradox Mods), 1:1 recommended; only shown for downloaded mods | HOI4 wiki |
| Stellaris | Must be `thumbnail.png` with launcher 2.4+, under 1 MB, at least 512×512 recommended, JPEG and PNG accepted | Stellaris wiki |
| CK2 | `picture`, displayed as 268×268 JPEG | CK2 wiki |
| Imperator | `thumbnail.png` in the mod folder, at least 200×200 to avoid black borders | Imperator wiki |
| CK3 | `picture` is for the launcher; "Steam ignores this key and always looks for thumbnail.png" | CK3 wiki |
| Victoria 3 | `.metadata/thumbnail.png`, used by the launcher; size not stated | Vic3 wiki |
| EU5 | `.metadata/thumbnail.png`, "both inside the launcher and for the workshop", under 1 MB, 512×512 recommended | EU5 wiki |

All 5 Vic3 and 7 EU5 GitHub samples that ship a thumbnail keep it at `.metadata/thumbnail.png` (Verified on GitHub). PWM defaults to `thumbnail.png` relative to the mod root but makes the path configurable ([`config.go` L96–L97](https://github.com/kaiser-chris/pdx-workshop-manager/blob/c32792c61068419565d092766d2f67d79cea2b91/config/config.go#L96-L97), [`manager.go` L119–L121](https://github.com/kaiser-chris/pdx-workshop-manager/blob/c32792c61068419565d092766d2f67d79cea2b91/manager/manager.go#L119-L121)) (Code).

**Unknown:** whether the Paradox launcher reads `picture` when it differs from `thumbnail.png`. Only one EU4 item, created in 2014 and last updated in 2024, uses a different name.

## 4. Workshop tags

### 4a. What Steam accepts (Documented, ISteamUGC)

`SetItemTags` "sets arbitrary developer specified tags". Each tag is at most 255 characters of printable characters, excluding `,`.

A query result returns tags in a comma-separated buffer of `k_cchTagListMax` = 1025 bytes. The constant comes from the Steamworks SDK headers as mirrored in [Steamworks.NET `SteamConstants.cs`](https://github.com/rlabrecque/Steamworks.NET/blob/master/com.rlabrecque.steamworks.net/Runtime/autogen/SteamConstants.cs). That file also gives a title limit of 128+1 and a description limit of 8000.

**Undeclared tags are accepted and kept (Verified, Steam API).** EU4 items carry tags outside the declared list, such as `Fantasy`, `Ironman`, `Music`, `Theatrum Orbis Terrarum` and `interface`. The same holds for HOI4 (`Economy`, `AI`), Stellaris (`Megastructures`, `Origins`), Imperator (`Map`, `Balance`, `Translation`) and Vic3 (`Economy`, `Military`). Only declared tags show up in the browse filter.

**Steam normalises tag casing (Verified).** For 49 of 51 EU4 items, the descriptor's tags equal the Steam tags once each tag is case-folded to the declared spelling. For example, descriptor `"Graphics"` becomes Steam `graphics` and `"Missions And Decisions"` becomes `Missions and Decisions`. The undeclared `"Interface"` came back as `interface`, so casing is shared across items even for undeclared tags. The two remaining mismatches are items whose Steam tags were changed outside the descriptor.

### 4b. Declared lists per game (Verified, Workshop browse `declaredTags.readytouse_tags`, 2026-10-20)

| Game | Declared tags |
|---|---|
| EU4 (22) | Alternative History, Balance, events, Expansion, Fixes, Gameplay, graphics, Guide, Historical, Loading Screen, Map, Military, Missions and Decisions, National Ideas, New Nations, Religion, Sound, Technologies, trade, Translation, Utilities, Converted from CKII |
| HOI4 (15) | Alternative History, Balance, Events, Fixes, Gameplay, Graphics, Historical, Ideologies, Map, Military, National Focuses, Sound, Technologies, Translation, Utilities |
| Stellaris (25) | Balance, Buildings, Diplomacy, Economy, Events, Fixes, Font, Galaxy Generation, Gameplay, Graphics, Leaders, Loading Screen, Military, Overhaul, Sound, Spaceships, Species, Technologies, Total Conversion, Translation, Utilities, 3.0.8, 3.0.9, 3.0.10, 3.0.11 |
| CK2 | none for mods (only guide tags are declared) |
| Imperator (11) | Characters, Flags, Historical, Interface, Ironman Compatible, Overhaul, Portraits, Silly, Total Conversions, Unit Graphics, Unit Types |
| CK3 (20 + 7) | Alternative History, Balance, Character Focuses, Character Interactions, Culture, Decisions, Events, Fixes, Gameplay, Graphics, Historical, Map, Religion, Sound, Total Conversion, Translation, Utilities, Warfare, Interface, Submod; group "Compatible Version": 1.15 'Crown', 1.16 'Chamfron', 1.17 'Ascendant', 1.18 'Crane', 1.19 'Scribe', 1.20 'Crozier', Older |
| Victoria 3 (24 + 6) | Alternative History, Balance, Cultures and Religions, Diplomacy, Events, Expansion, Economy and Buildings, Fixes, Flags, Gameplay, Graphics, Historical, Interest Groups, Journal Entries, Map, New Nations, Pops, Sound, Technologies, Total Conversion, Trade, Translation, Utilities, Warfare; group "Update Compatibility": Deprecated, 1.9, 1.10, 1.11, 1.12, 1.13 |
| EU5 (26) | Alternative History, Balance, Events, Fixes, Gameplay, Historical, Loading Screen, Map, Military, Situations, Religion, Sound, Utilities, Translation, Culture & Art, Diplomacy, Government and Estates, Characters, International Organizations, User Interface, Flags, Textures, Advancements, Missions, Overhaul, Trade and Economics |

The wikis' lists differ from Steam's. The EU4 wiki lists 21 tags without `Converted from CKII` and with title-case spellings. The CK3 wiki's launcher list includes Bookmarks, Portraits and Schemes, which Steam does not declare, and leaves out Interface and Submod.

### 4c. How the launcher maps descriptor tags

- **No translation table was found.** In EU4, descriptor tags reach Steam verbatim and Steam case-folds them (section 4a, Verified).
- In the launcher database, tags of Steam items are stored as Steam returns them (`["Map","Missions and Decisions"]`), while local mods keep the descriptor spelling (`"Missions And Decisions"`) (Verified, `launcher-v2.sqlite`, local EU4 install).
- **Launcher-side limits:** Stellaris caps an item at 10 tags since launcher 2022.10 and warns that non-predefined tags may block Paradox Mods uploads (Stellaris wiki). Vic3 allows at most 5 (Vic3 wiki).
- Largest tag counts observed per item: EU4 7, HOI4 10, Stellaris 7, Vic3 5, EU5 9 (Verified, Steam API sample). One EU5 `metadata.json` on GitHub lists 19 tags (Verified on GitHub), so EU5's tool does not enforce 5 on the file. **Unknown:** what the EU5 in-game uploader does with them.
- Vic3 and EU5 mods often put game versions such as `"1.13"` in `tags` (GitHub samples). These match Vic3's "Update Compatibility" group but are undeclared on EU5 (Verified).

## 5. What the uploader writes back after publishing

**EU4 (Verified, E2 and local EU4 install):**

- After the first launcher upload, `remote_file_id="<publishedfileid>"` appears as the last line of the mod folder's `descriptor.mod` and of the outer `<name>.mod`.
- The launcher database row for the local mod gains `remoteSteamId`. Four local mods in `launcher-v2.sqlite` have it set, `source='local'`.
- Every one of the 42 shipped EU4 `descriptor.mod` files contains its own `remote_file_id`, so the ID is in the descriptor that gets published on every update after the first.
- One locally developed mod tracked in Git shows the launcher's write-back committed as a separate change after the first upload.
- E3 showed the reverse also works: a `descriptor.mod` with `remote_file_id` written before an `ISteamUGC` upload is accepted and delivered byte-identical.

**HOI4, Stellaris, CK3 (Documented):** `remote_file_id` is the Workshop ID and is "added by the launcher" (Stellaris) or "set automatically on upload" (CK3). The HOI4 wiki also lists it as an ID that appears in both files.

**Imperator (Documented):** "the first upload creates a descriptor.mod file in your mod folder that links to your Steam mod; updates depend on it" (Imperator wiki). The key is not named there; that it is `remote_file_id`, as in every other `descriptor.mod` game, is Inferred.

**Victoria 3, EU5 (Verified absence; mechanism Unknown):**

- None of 7 Vic3 and 8 EU5 `metadata.json` files from public GitHub repositories of published mods has a Steam ID field. Irony's JSON model has none either ([`ModParser.cs` L259–L316](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Parser/Mod/ModParser.cs#L259-L316)).
- One Vic3 mod uses its Workshop ID as `id`, but that was the modder's choice.
- The launcher's database schema, shared across games, has `remoteSteamId`, `metadataId`, `metadataGameId` and `metadataStatus` columns (Verified schema). This suggests the Vic3 launcher keeps the link in its database rather than in the file (Inferred).
- EU5 has no Paradox launcher (Code: Irony). Where its in-game uploader keeps the item ID is Unknown.
- PWM keeps the item ID in its own config (`"id": 0` creates a new item and is replaced by the new ID) (Code: [README](https://github.com/kaiser-chris/pdx-workshop-manager/blob/c32792c61068419565d092766d2f67d79cea2b91/README.md)).
- The `picture` key found in some Vic3 and EU5 files may be launcher- or tool-written. Unknown.

**CK2:** Unknown. The wiki does not cover upload write-back.

## 6. How a downloaded item becomes a playable mod

**EU4 (Verified, local EU4 install):**

1. Steam downloads the item to `<Steam>/steamapps/workshop/content/236850/<id>/`.
2. The Paradox launcher writes `<Documents>/Paradox Interactive/Europa Universalis IV/mod/ugc_<id>.mod`. Its content is the item's `descriptor.mod`, or its only `*.mod` when there is no descriptor (9 of 51 items), with `path=` rewritten to the absolute Workshop folder and placed just before `remote_file_id`. Any uploader `path=` in the shipped file is replaced. Otherwise the content is identical line by line, apart from the final newline. This was checked for all 42 items with a `descriptor.mod` and for 2 of the 9 items with only a `<name>.mod`.
3. The launcher database records the item with `source='steam'`, `steamId`, `gameRegistryId='mod/ugc_<id>.mod'`, `dirPath` set to the Workshop folder, and a thumbnail cached from Steam's `preview_url`.
4. The enabled playset goes to `dlc_load.json` as `{"enabled_mods":["mod/ugc_<id>.mod", …]}`.

**HOI4, Stellaris, Imperator, CK3:** The same `ugc_<id>.mod` convention applies. Steam items are registered as `mod/ugc_<RemoteId>.mod` with `RemoteId` taken from the Workshop folder name (Code: Irony [`ModService.cs` L846–L862](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Services/ModService.cs#L846-L862); `Steam_mod_id = "ugc_"` in [`Services/Constants.cs` L39](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Services/Constants.cs#L39)). The HOI4 wiki also says the `.mod` files of Workshop mods "appear in the mod folder under the ID name". Paradox Mods items use `pdx_<id>.mod` instead (Irony; Stellaris wiki).

**Victoria 3:** There is no `ugc_` descriptor. The game's `content_load.json` lists enabled mods by folder path (`enabledMods[].path`) (Code: Irony [`JsonExporter.cs` L262–L281](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.IO/Mods/Exporter/JsonExporter.cs#L262-L280), [`IO/Mods/Constants.cs` L34](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.IO/Mods/Constants.cs#L34)). Irony's own `irony-mod/ugc_<id>.json` files are Irony's convention, not the game's.

**EU5:** There is no `ugc_` descriptor. Enabled mods and DLC go into `playsets.json` (Code: Irony [`JsonExporter.cs` L130–L136](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.IO/Mods/Exporter/JsonExporter.cs#L130-L136), [`IO/Mods/Constants.cs` L54](https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.IO/Mods/Constants.cs#L54)).

**CK2:** Workshop items are zips (`mod/<name>.zip`, Verified). The CK2 wiki says Workshop downloads use the `archive` key. The exact files the game or launcher creates are Unknown.

## 7. Implications for the v1 spec (Inferred, for the deciding tickets)

- **Two profiles hold, but CK3 belongs to the `descriptor.mod` profile:**
  - `descriptor.mod`: EU4, HOI4, Stellaris, Imperator, CK3.
  - `metadata.json`: Victoria 3, EU5.
  - CK2 needs a separate legacy zip path and fits neither profile.
- **The item ID lives in different places.** In the `descriptor.mod` profile, `remote_file_id` in `descriptor.mod` is the native place, and the launcher already ships it in the published content (E2, E3). In the `metadata.json` profile nothing in the file carries it, so the action needs its own place, such as a config input or file, as PWM does.
- **Preview:** use `thumbnail.png` in the mod root (`descriptor.mod` profile) or `.metadata/thumbnail.png` (`metadata.json` profile), and fail before upload when it is 1 MB or larger, since Steam rejects it anyway.
- **Tags:** pass them through verbatim, because Steam keeps undeclared tags and fixes case.
  - Optionally warn about tags outside the declared list for the app ID.
  - Optionally enforce the launcher limits that are documented (Stellaris 10, Vic3 5).
- **Hygiene:** strip `path=` (and warn about `archive`/`user_dir`) from the published `descriptor.mod`, because 14 of 51 EU4 items leak a private path this way. Reject a BOM in `descriptor.mod`.

## Unknowns

- Whether CK3 has switched to `metadata.json` in a 2025–2026 patch. All sources say `descriptor.mod`, but the CK3 wiki page is from 1.1.
- Where the Victoria 3 launcher and the EU5 in-game uploader store the Workshop item ID for a local mod. The launcher database is likely; the EU5 location is not known.
- Whether the Paradox launcher honours `picture` when it differs from `thumbnail.png`.
- Tag-count limits enforced by the EU4, HOI4, Imperator, CK3 and EU5 uploaders.
- CK2's write-back and download handling.
- The `picture` key in `metadata.json`: whether the launcher writes it or modders add it by hand.
