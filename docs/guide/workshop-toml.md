# `workshop.toml` reference

`workshop.toml` says which mods a repository publishes and how. `pdx-ws` looks for it in the current folder; `--config <path>` (or the action's `config` input) names another file. Paths inside it are relative to its own folder.

```toml
schema = 1

[defaults]                       # optional; any [[mod]] key except id, path and item_id
game = "eu4"
checksum = "warn"

[[mod]]
id = "my-mod"                    # required only with several mods
game = "eu4"                     # required, here or in [defaults]
path = "."                       # default "."
visibility = "private"           # unset: publishing never changes the visibility
exclude = ["docs/**"]            # globs, added to the defaults and the built-in excludes
description = "workshop/description.md"   # .md (converted) or .bbcode (sent as it is)
changelog = "CHANGELOG.md"
checksum = "warn"                # "warn" | "keep" | "ignore"; EU4 only in v1
# item_id = 123                  # Victoria 3 and EU5 only
```

Keys are strict: an unknown key or a missing `schema` is an error, so a typo cannot silently do nothing.

## Keys

| Key | Default | Meaning |
| --- | --- | --- |
| `schema` | required | The file format version. Always `1`. |
| `id` | none | A short slug (lowercase letters, digits, hyphens) such as `anbennar-2k-ui`. Required when the file has more than one `[[mod]]`. Used by `--mod`, the action's `mod` input and release tags. |
| `game` | required | `eu4`, `hoi4`, `stellaris`, `imperator`, `ck3`, `vic3` or `eu5`. `ck2` is rejected: CK2 uses legacy single-file Workshop items, which `pdx-ws` does not support. |
| `path` | `"."` | The mod root, relative to the folder of `workshop.toml`. |
| `visibility` | unset | `public`, `friends`, `private` or `unlisted`. Unset, publishing never changes the item's visibility. `pdx-ws init` uses `private` when it is unset. |
| `exclude` | `[]` | Globs of files to leave out of the published copy, relative to the mod root. `*` stays within one folder and `**` crosses folders; `docs/**` leaves out the whole `docs` folder. Lists in `[defaults]` and `[[mod]]` are combined. |
| `description` | unset | A `.md` file (converted to BBCode) or a `.bbcode` file (sent as it is). Unset, the Workshop description is never touched. With it, **every publish replaces the description.** See [Description and change notes](description-and-change-notes.md). |
| `changelog` | unset | A Markdown changelog. The section for the descriptor's `version` becomes the change note. |
| `checksum` | `"warn"` | What happens when an EU4 mod changes the game's checksum. See [The checksum warning](checksum.md). |
| `item_id` | unset | The Workshop item ID, for games whose descriptor is `.metadata/metadata.json` (Victoria 3, EU5). For `descriptor.mod` games it is an error: the ID belongs in `remote_file_id`. |

A value in `[[mod]]` overrides the same key in `[defaults]`, except `exclude`, where both lists apply.

## What comes from the descriptor

There are no metadata overrides in `workshop.toml`; the descriptor is the single source.

| Workshop field | `descriptor.mod` games (EU4, HoI4, Stellaris, Imperator, CK3) | `metadata.json` games (Victoria 3, EU5) |
| --- | --- | --- |
| Title | `name` | `name` |
| Tags | `tags`, sent as written; set in `client` mode only | `tags`, sent as written; set in `client` mode only |
| Preview | `thumbnail.png` in the mod root | `.metadata/thumbnail.png` |
| Version (for change notes) | `version` | `version` |
| Item ID | `remote_file_id` | `item_id` in `workshop.toml` |

- **Preview:** it must be smaller than 1 MB (1,048,576 bytes). Without one, the Workshop preview is left as it is, and the plan warns.
- **Tags:** the plan warns above the launcher's limits: 10 tags for Stellaris and 5 for Victoria 3. In `steamcmd` mode tags are not sent at all.
- **Item ID:** if both `remote_file_id` and `item_id` are set and differ, that is an error.
- **Encoding:** `descriptor.mod` must be UTF-8 without a byte order mark.

## The published copy

Every publish uploads a temporary copy of the mod, never the repository folder itself:

- **Always left out:** `.git*`, `.github/`, `workshop.toml` and `*.vdf`, at any depth.
- **Also left out:** your `exclude` globs.
- **Errors:** symbolic links, and an `exclude` that would leave out the descriptor.
- **`descriptor.mod` in the copy:**
  - loses `path`, `archive` and `user_dir`, which only make sense on your computer;
  - gains `remote_file_id` when the item exists.

Your repository is never changed by a publish. The only write-back is the item ID that `pdx-ws init` writes once.

`pdx-ws plan` lists the copy's files, what was excluded and how the descriptor changes.
