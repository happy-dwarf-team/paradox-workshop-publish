# Several mods in one repository

One `workshop.toml` can describe several mods, each with its own Workshop item:

```toml
schema = 1

[defaults]
game = "eu4"
# Repository files inside the mod folders that players do not need.
exclude = ["CHANGELOG.md", "version.txt"]
description = "workshop/description.md"

[[mod]]
id = "alpha"
path = "mods/alpha"
changelog = "mods/alpha/CHANGELOG.md"

[[mod]]
id = "beta"
path = "mods/beta"
changelog = "mods/beta/CHANGELOG.md"
visibility = "unlisted"
```

- **`id`** is required for every mod once there is more than one. Keep it short and stable, because release tags use it. The descriptor's name is not used: it is long, has spaces and changes when a mod is renamed.
- **`[defaults]`** holds any `[[mod]]` key except `id`, `path` and `item_id`. A mod's own value wins. `exclude` lists from both apply.
- **Paths** are relative to the folder of `workshop.toml`. In the example, both mods share `workshop/description.md`. Give each mod its own `description` if their pages differ.

## Selecting mods

- Without `--mod` (or the action's `mod` input), `plan` and `publish` take every mod in file order.
- `--mod alpha --mod beta` (CLI) or `mod: alpha, beta` (action) narrows the selection.

The checks before any upload cover only the selected mods: a missing item ID, an invalid description or a `checksum = "keep"` violation. Any one of them stops the whole run with exit code 2, before anything is sent.

## When a mod fails

The other mods are still published, and the run exits with code 1 at the end. The result table, the `--json` output and the action's summary have one row per mod:

| Result | Meaning |
| --- | --- |
| `published` | The existing item was updated. |
| `unchanged` | `steamcmd` mode: Steam found no content change and kept the item's current version, so the change note was not recorded (see [the change note](description-and-change-notes.md#when-steam-keeps-the-old-version)). A warning, not a failure: the run still exits 0. |
| `created` | `--allow-create` made a new item; commit the ID it prints. |
| `failed` | This mod failed; its error and steamcmd's error lines are shown. |
| `skipped` | Not attempted, after `--fail-fast` or a rejected session. |

`--fail-fast` stops at the first failure. An expired Steam session also stops the run, with exit code 3, because no other mod could log in either.

## Change notes

Each mod gets its change note from its own `changelog` and descriptor `version`. A `change-note` input applies to every selected mod. With several mods, leave it empty and keep one changelog per mod.

## release-please in monorepo mode

[release-please](https://github.com/googleapis/release-please) can version each mod separately from Conventional Commits:

- it opens a release pull request per mod;
- it bumps the descriptor's `version`;
- it writes the mod's `CHANGELOG.md`;
- once that pull request is merged, it tags `<id>-vX.Y.Z` (e.g. `alpha-v1.2.0`) and creates a GitHub release.

The example files set this up:

- [`release-please-config.json`](../examples/release-please-config.json) has one package per mod folder. Its `component` is the mod's `id`, so the tags are `<id>-vX.Y.Z`. A `generic` updater bumps the `version` line in `descriptor.mod`, which carries an `x-release-please-version` comment:

  ```
  version="1.2.0" # x-release-please-version
  ```

  For Victoria 3 and EU5, use a `json` updater on `.metadata/metadata.json` with `"jsonpath": "$.version"` instead.
- Start `.release-please-manifest.json` with each mod's current version, e.g. `{"mods/alpha": "1.1.0", "mods/beta": "0.4.0"}`.
- release-please's `simple` release type also keeps a `version.txt` and the `CHANGELOG.md` in each mod folder. The `exclude` in `[defaults]` above keeps both out of the upload.
- [`release-please-monorepo.yml`](../examples/release-please-monorepo.yml) runs release-please on every push to `main`. When it creates releases, the workflow publishes exactly the mods that were released.

**Why publish in the same workflow?** Releases and tags that a workflow creates with the default `GITHUB_TOKEN` do not start other workflows, so a separate `on: release` workflow would never run. The example reads release-please's `paths_released` output instead, and takes each released mod's `id` from its folder name. Keep the folder name and the `id` equal, or adapt that step.

**Environment tag rules:** these releases come from `main`, so the `steam-workshop` environment must allow `main` too. Required reviewers then approve each publish.
