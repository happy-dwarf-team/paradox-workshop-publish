# Checksum manifests per game: can achievement loss be detected without the game files?

Research for [#5](https://github.com/happy-dwarf-team/paradox-workshop-publish/issues/5), part of the [v1 map](https://github.com/happy-dwarf-team/paradox-workshop-publish/issues/1). Researched 2026-10-20.

Evidence labels used below:

- **Verified**: read directly in a primary source (official wiki, patch notes, or the local EU4 1.37.5 install).
- **Mirror**: a game file read from a third-party GitHub repository that copies game files. It is the real file only if the uploader copied it faithfully; treat it as unverified until checked against an install.
- **Tool**: what an open-source tool encodes. Shows what its author believed, not what the game does.
- **Unknown**: not settled by any source found.

## Per-game summary

| Game | What defines the checksum | Checksummed scope (summary) | Changed checksum blocks achievements? | Ironman needed for achievements? | Multiplayer needs equal checksum? | Rules stable across versions? |
|---|---|---|---|---|---|---|
| **EU4** | `checksum_manifest.txt` in the install root (Verified, local EU4 1.37.5 install) | `common/` `.txt` `.lua` (recursive); `events/`, `missions/`, `decisions/`, `history/` `.txt` (recursive); `map/` top level only `.txt` `.map` `.bmp` `.csv`; `map/random/` `.lua` (top level), `.txt` `.bmp` (recursive) (Verified, local) | Yes [W-EU4-ACH], [W-EU4-CS] | Yes [W-EU4-ACH] (ironman itself works with mods since 1.14, local patch notes) | Yes [W-EU4-CS] | Mostly; changed in early versions (1.13 added `tweaks.lua`, local patch notes). Unchanged list in 2023–2024 mirrors [M-EU4] |
| **HOI4** | `checksum_manifest.txt` in the install root (Mirror [M-HOI4]; Tool [T-PAR]) | `common/` `.txt` `.lua`; `events/`, `history/` `.txt`; `map/` `.txt` `.map` `.bmp` `.csv`, all recursive (Mirror, 2018; Tool). Wiki: all of `common/`, `events/`, `history/`, most of `map/` except `map/terrain/` [W-HOI4-MOD] | Yes [W-HOI4-ACH], [W-HOI4-CS] | Yes, single-player ironman, 1936 start [W-HOI4-ACH] | Yes [W-HOI4-CS], [W-HOI4-MOD] | Unknown. Copies in 2021 and 2024 mod repositories add one `interface/*.gui` entry each [M-HOI4-B]; not confirmed as vanilla |
| **Stellaris** | `checksum_manifest.txt` (Tool [T-PAR] reads it; file contents not found in any mirror) | Unknown exactly. Irony encodes folders `common`, `events`, `map`, `localisation_synced` [T-IRONY]; the wiki says `common/` changes disable achievements [W-STE-MOD] | Yes [W-STE-ACH] | No (only the *Victorious* achievement) [W-STE-ACH] | Same mod list and load order needed [W-STE-MOD] | Unknown |
| **CK3** | `game/checksum_manifest.txt` (Mirror, 1.19.0.3 [M-CK3]) | `common`, `events`, `history`, `map_data`, `gui`, `localization`, `data_binding`, `gfx/court_scene/scene_settings` (all recursive, all extensions) plus the file `paths_checksummed.settings` (Mirror) | **No, since 1.9** [W-CK3-ACH], [W-CK3-1.9] | No, since 1.9 [W-CK3-1.9] | Presumed yes (not separately sourced); opening a multiplayer lobby removes achievement eligibility [W-CK3-1.9] | Unknown |
| **Victoria 3** | `game/checksum_manifest.txt` (Mirror, 2022 [M-VIC3]) | `common`, `events`, `map_data`, `gui`, `localization` (recursive, all extensions) plus `paths_checksummed.settings` (Mirror, 2022) | **No** [W-VIC3-ACH] (since which version: Unknown) | No [W-VIC3-ACH] | Presumed yes (not separately sourced); multiplayer itself blocks achievements [W-VIC3-ACH] | Unknown |
| **EU5** | Several manifests: `game/main_menu/checksum_manifest.txt`, `game/loading_screen/checksum_manifest.txt`, and one per DLC folder (Mirror [M-EU5]) | `common`, `events`, `map_data`, `setup`, `gui` (minus `gui_editor`), `localization` (recursive, all extensions). Which root each manifest applies to is Unknown; Irony reads them as `in_game/{common,events,map_data,setup,gui}` and `main_menu/localization` [T-IRONY] | **Conflicting.** Wiki (verified for 1.1): checksum must match [W-EU5-ACH]. Community reports: from 1.3, mods and non-ironman games no longer block achievements [C-EU5-1.3]. No official patch note found | Conflicting (see left) | Presumed yes (not sourced) | **No.** Changed between 1.0.10 and 1.2.0 (`exclude_path = dlc` removed) [M-EU5] |
| CK2 (brief) | Checksum computed from loaded files; manifest not found [W-CK2-CS] | Unknown | Yes [W-CK2-ACH] | Yes, single-player ironman [W-CK2-ACH] | Unknown | Unknown |
| Imperator (brief) | Unknown | Irony encodes `common`, `events`, `decisions`, `gui`, `localization`, `map_data`, `setup` [T-IRONY] | Unknown (wiki silent) [W-IMP-ACH] | Yes, ironman, grand campaign [W-IMP-ACH] | Unknown | Unknown |

## Answers to the ticket's questions

### 1. Which file or rule defines what affects the checksum?

Every game examined has a plain-text `checksum_manifest.txt` that lists folders, whether subfolders count, and (in Clausewitz games) file extensions. Two dialects exist:

- **Clausewitz (EU4, HOI4, Stellaris):** `directory` blocks with `name`, `sub_directories` and `file_extension`. EU4 1.37.5 writes them as `directory = { ... }` with quoted values (local EU4 1.37.5 install); HOI4 writes bare `directory` lines followed by `key = value` lines [M-HOI4]. Parsers must accept both; parallax-mod-manager's parser accepts the bare form, a BOM and quoted values [T-PAR].
- **Jomini (CK3, Victoria 3, EU5):** `directory` blocks with `name` and `sub_directories` but **no extension filter** (every file counts), optional `exclude_path`, and `file` entries naming single files such as `paths_checksummed.settings` [M-CK3], [M-VIC3], [M-EU5].

Practical consequence: in Jomini games `gui/` and `localization/` are checksummed, so even a pure translation or interface mod changes the checksum (Mirror). In EU4, `interface/`, `gfx/`, `localisation/`, `music/` and `sound/` are not listed (local EU4 1.37.5 install), which matches the wiki's statement that graphical, interface and music mods usually keep the checksum [W-EU4-ACH] and the lab's evidence E1 (map issue #1).

What blocks achievements and ironman is **not** defined by the manifest alone:

- EU4 achievements also need ironman, Normal or harder difficulty, historical lucky nations, normal province values, normal or historical nations, and mostly the 1444 start [W-EU4-ACH]. In the game files these are conditions such as `ironman = yes`, `normal_or_historical_nations = yes` and `normal_province_values = yes` in `common/achievements.txt` (373, 347 and 372 occurrences respectively; local EU4 1.37.5 install). These are player choices at game start, not properties of a mod.
- Since 1.14, EU4 ironman can be played with any mods; only achievements require the official checksum ("Ironman games can now be played with mods ... Achievements, however, will only be gained if certain criteria are fulfilled", `patchnotes/1.14 Patchnotes.txt`, local EU4 1.37.5 install; also [W-EU4-CS]).
- HOI4, CK2: game rules and console use also block [W-HOI4-ACH], [W-CK2-ACH]. CK3, Victoria 3: game rules, debug commands and multiplayer block, mods do not [W-CK3-1.9], [W-VIC3-ACH].

### 2. Is it stable across game versions?

Not guaranteed, so a bundled rule table must be versioned:

- **EU4:** The manifest grew in early versions: 1.13 "Added tweaks.lua to checksum" (`patchnotes/1.13 Patchnotes.txt`, local). The 1.37.5 file is dated 2024-08-25 and has SHA-256 `2d7c64965363d0543ec61c512c4079ebbd0521acdfce9fde2590b2db1b6b7ff1` (local EU4 1.37.5 install, version string `EU4 v1.37.5.0 Inca (491d)`). EU4 copies in mod repositories from 2023–2024 show the same entries [M-EU4]. EU4 is in late maintenance, so drift risk is low but not zero.
- **HOI4:** The 2018 (1.5) mirror [M-HOI4] lists 8 entries. Mod repositories from 2021 and 2024 contain the same 8 plus one `interface/console` or `interface/multiplayer_chat` `.gui` entry [M-HOI4-B]. Whether that entry is vanilla in some versions or a mod's own edit is Unknown.
- **EU5:** Confirmed drift. `main_menu/checksum_manifest.txt` dropped `exclude_path = dlc` from `common` and `localization` between 1.0.10 and 1.2.0, and new DLC folders brought their own manifests (`D008_fate_of_the_phoenix` lists `in_game` and `main_menu`) [M-EU5].
- **Stellaris, CK3, Victoria 3, CK2, Imperator:** Unknown; no version history found.

### 3. Can a CI run without the game warn that a mod will disable achievements?

**Yes, as a yes/no prediction, for the games where the checksum still gates achievements.** The prediction needs only the manifest rules, not the game's files:

- A mod changes the checksum if it ships at least one file that a manifest rule matches. This is the predicate Irony Mod Manager uses for its "achievement compatible" status: a mod is compatible when none of its files starts with one of the game's checksum folders [T-IRONY-SVC].
- Irony's folder-prefix check is coarser than the manifest. It ignores extensions and `sub_directories = no`, so for EU4 it would flag, for example, a `.dds` under `map/terrain/` or a `.yml` under `common/`, neither of which the EU4 1.37.5 manifest covers (local EU4 1.37.5 install; [T-IRONY]). Matching the full rule (folder, recursion, extension, `exclude_path`) avoids those false positives.
- **`replace_path` is a second trigger** (hypothesis, not tested in the lab). A `replace_path` that covers a checksummed folder removes vanilla files from the hashed set even if the mod adds nothing there. parallax-mod-manager models exactly this for Stellaris and HOI4 (and says HOI4 hashes replaced files as empty files) [T-PAR]. The lab's E1 did not test `replace_path`.
- **Edge case:** a mod file that is byte-identical to the vanilla file at the same path may leave the checksum unchanged, because the hash covers bytes and path [T-PAR]. A predicate without the game files reports this as a change (a false positive).
- **The exact 4-character checksum cannot be computed without the game.** It is an MD5 over every checksummed file's bytes and path from the game folder plus the mods, salted with the version string; HOI4 and Stellaris use different schemes and mod orders [T-PAR]. A CI run can only say "changes the checksum" or "does not", not which code players will see.

What the warning should say differs per game:

| Game | Meaning of "changes the checksum" for the modder |
|---|---|
| EU4, HOI4, Stellaris, CK2 | Players lose achievements while the mod is active, and multiplayer partners need the same mods. EU4 ironman itself still works. |
| CK3 (1.9+), Victoria 3 | Achievements are not affected. Only multiplayer compatibility (everyone needs the mod). |
| EU5 | Achievement effect depends on the game version and is Unknown after 1.3; multiplayer partners need the mod. |
| Imperator | Unknown; do not claim an achievement effect. |

### 4. Can the manifest content be redistributed or encoded?

This is sourcing, not legal advice.

- **What the content is:** the EU4 1.37.5 manifest is 1,055 bytes holding 13 entries, each a folder name, a yes/no flag and a file extension, with no copyright notice in the file (local EU4 1.37.5 install). It functions as a configuration that tells the game which files to hash.
- **US law:** copyright does not extend to "any idea", nor to procedures, processes, systems or methods of operation (17 U.S.C. §102(b), https://www.law.cornell.edu/uscode/text/17/102). "There can be no valid copyright in facts", and the copyright in a factual compilation is "thin", covering only original selection and arrangement (Feist v. Rural, 499 U.S. 340 (1991), https://www.law.cornell.edu/supremecourt/text/499/340).
- **EU law:** the CJEU's *SAS Institute v World Programming* (C-406/10, 2012) is commonly cited for the rule that a program's functionality and data file formats are not protected as expression (https://curia.europa.eu/juris/liste.jsf?num=C-406/10). The judgment text was not retrieved in this research, so this pointer is Unknown/unverified here.
- **Paradox EULA:** the end-user agreement encourages mods ("We love when our community creates user-generated content, including mods"), licenses the games as "personal, limited, non-transferable, revocable, and non-exclusive", and has no specific clause on game configuration files or on encoding them in third-party tools (https://legal.paradoxplaza.com/eula?locale=en, sections 1 and 5).
- **Precedent in established tools:** Irony Mod Manager (MIT licence, about 527 stars, maintained in 2026) hard-codes each game's checksum folders as constants [T-IRONY]. parallax-mod-manager hard-codes HOI4's rules as a fallback when the game's manifest is missing [T-PAR]. Many public repositories also copy the manifest files verbatim [M-CK3], [M-VIC3], [M-EU5]; that shows practice, not permission.
- **Lowest-exposure option for us:** encode the rules as data in our own format (folder, recursion, extensions, excludes, "verified on version X" with a source), rather than shipping Paradox's file verbatim. In local mode, when the game is installed, read the user's own `checksum_manifest.txt` instead and report drift from the bundled table.

### 5. Other "will break achievements or multiplayer" signals

- **`replace_path` / `replace_paths`** in the descriptor or metadata covering a checksummed folder (see above) [T-PAR]; field described as "Doesn't load vanilla files for the specified path" (https://ck3.paradoxwikis.com/Mod_structure).
- **`game_custom_data.multiplayer_synchronized`** in Victoria 3's `.metadata/metadata.json`, described only as information "the game engine needs to know" (https://vic3.paradoxwikis.com/Mod_structure). What it changes is Unknown; it is a candidate signal for the `metadata.json` profile but must not be interpreted until tested.
- **Files that redefine achievement eligibility**, for example EU4 `common/achievements.txt` (local EU4 1.37.5 install). These are already inside the checksummed scope, so no extra rule is needed.
- **HOI4 `-checksum` launch option** prints per-file checksum output that can be compared between users [W-HOI4-MOD]. Useful for a local-mode diagnostic, not for CI (needs the game).
- **Mod repositories that ship their own `checksum_manifest.txt`** exist [M-HOI4-B]. Whether the game ever reads a manifest from a mod folder is Unknown; a publisher could warn when a mod contains one.
- **Not useful as signals:** difficulty, start date, lucky nations, game rules and console use. They block achievements but are chosen by the player at game start, not shipped by the mod [W-EU4-ACH], [W-HOI4-ACH], [W-CK3-1.9].

## Implications for v1 (input to the spec, not a decision)

1. Ship a versioned per-game rule table in our own format, with each entry marked Verified (checked on an install) or Mirror/Tool. Only EU4 1.37.5 is Verified today.
2. Predicate: "changes the checksum" when any mod file matches a rule, or a `replace_path` covers checksummed scope. Report the matching paths so the modder sees why.
3. Wording per game from the table above. Never promise achievements are kept; say "does not touch checksummed files".
4. In local mode, prefer the installed `checksum_manifest.txt` and warn if it differs from the bundled table.
5. Before claiming support for HOI4, Stellaris, CK3, Victoria 3 and EU5, verify their manifests against a real install (and EU5's root semantics and post-1.3 achievement rules in game).

## Unknowns

- Stellaris, CK2 and Imperator manifest contents; current HOI4, CK3 and Victoria 3 manifests on a verified install.
- How EU5's several manifests map to `in_game/`, `main_menu/` and DLC roots.
- EU5 achievement rules after 1.3 (official notes not found).
- Since which Victoria 3 version a changed checksum no longer blocks achievements.
- Whether `replace_path` changes the EU4 checksum (expected, not tested).
- Whether multiplayer in CK3, Victoria 3 and EU5 still requires an equal checksum (presumed).
- Meaning of `multiplayer_synchronized`.

## Sources

Local:

- local EU4 1.37.5 install: `checksum_manifest.txt`, `launcher-settings.json`, `common/achievements.txt`, `patchnotes/1.13 Patchnotes.txt`, `patchnotes/1.14 Patchnotes.txt`. Read only.

Official wikis and patch notes:

- [W-EU4-CS] https://eu4.paradoxwikis.com/Checksum
- [W-EU4-ACH] https://eu4.paradoxwikis.com/Achievements
- [W-HOI4-CS] https://hoi4.paradoxwikis.com/Checksum
- [W-HOI4-ACH] https://hoi4.paradoxwikis.com/Achievements
- [W-HOI4-MOD] https://hoi4.paradoxwikis.com/Modding
- [W-STE-ACH] https://stellaris.paradoxwikis.com/Achievements
- [W-STE-MOD] https://stellaris.paradoxwikis.com/Modding
- [W-CK3-ACH] https://ck3.paradoxwikis.com/Achievements
- [W-CK3-1.9] https://ck3.paradoxwikis.com/Patch_1.9 ("Achievements are no longer blocked by mod usage"; also Dev Diary #128, https://forum.paradoxplaza.com/forum/developer-diary/dev-diary-128-ck3-1-9-0-lance-update.1583293/, not retrievable by the fetcher)
- [W-VIC3-ACH] https://vic3.paradoxwikis.com/Achievements
- [W-EU5-ACH] https://eu5.paradoxwikis.com/Achievements
- EU5 patch history and 1.3 notes: https://eu5.paradoxwikis.com/Patches, https://eu5.paradoxwikis.com/Patch_1.3 ("Game rules that modify ironman or achievements are no longer changeable after game start"), https://patchbot.io/games/europa-universalis-v/articles/754-beta-version-patch-notes-1310
- [W-CK2-CS] https://ck2.paradoxwikis.com/Checksum
- [W-CK2-ACH] https://ck2.paradoxwikis.com/Achievements
- [W-IMP-ACH] https://imperator.paradoxwikis.com/Achievements

Community (not official):

- [C-EU5-1.3] https://steamcommunity.com/app/3450310/discussions/0/569288155748942530/ (user post, 3 June, citing the 1.3 open beta thread)

Tools:

- [T-IRONY] Irony Mod Manager checksum folders: https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Shared/Constants.cs
- [T-IRONY-SVC] Irony achievement compatibility check: https://github.com/bcssov/IronyModManager/blob/6a8861d734382a9e98f7394ea404fac7a2b884b3/src/IronyModManager.Services/ModService.cs
- [T-PAR] parallax-mod-manager checksum package (new project, created 2026-09, licence not stated as SPDX): https://github.com/Official-Husko/parallax-mod-manager/tree/d958863273883dedb8f52bbc22e714f9e55181f4/internal/checksum

Mirrors of game files (third-party copies, unverified):

- [M-HOI4] https://github.com/Killeritch/Hearts-of-Iron-IV/blob/5acd37e06d8a09cfb0246e3bcffc3812b2c8659e/checksum_manifest.txt (HOI4 1.5, 2018)
- [M-HOI4-B] https://github.com/LuciferHorst/TemporaryHFU/blob/HEAD/checksum_manifest.txt (2021), https://github.com/AuroraK1ng/GDU-2.0/blob/HEAD/checksum_manifest.txt (2024); mod repositories
- [M-EU4] https://github.com/AlejandroGiner/eu4/blob/HEAD/checksum_manifest.txt (2024), https://github.com/Marblistical/CNmod/blob/HEAD/checksum_manifest.txt (2023)
- [M-CK3] https://github.com/jesec/ck3-mod-base/blob/9a5d0ff6e105297755e19d574cbf4eacdd972709/base/game/checksum_manifest.txt (CK3 1.19.0.3)
- [M-VIC3] https://github.com/tjysdsg/Victoria-3-files/blob/0e8b62590c36bc9fdd2c8cba3cb5dda255092656/game/checksum_manifest.txt (2022)
- [M-EU5] https://github.com/eu5versions/game (one commit per version): 1.0.10 https://github.com/eu5versions/game/blob/a2d405e0146f35c5b11dac3aac34ec4ac77c36e9/main_menu/checksum_manifest.txt, 1.2.0 https://github.com/eu5versions/game/blob/2240d9bf10c0b064b8cd06a5195ac441cff893ad/main_menu/checksum_manifest.txt, 1.4 open beta https://github.com/eu5versions/game/blob/5e24b1a83d0003f470bfa44dcaca3364d71dff12/main_menu/checksum_manifest.txt

Legal sourcing (not legal advice):

- 17 U.S.C. §102: https://www.law.cornell.edu/uscode/text/17/102
- Feist Publications v. Rural Telephone Service, 499 U.S. 340 (1991): https://www.law.cornell.edu/supremecourt/text/499/340
- CJEU C-406/10 SAS Institute: https://curia.europa.eu/juris/liste.jsf?num=C-406/10 (not retrieved)
- Paradox EULA: https://legal.paradoxplaza.com/eula?locale=en
