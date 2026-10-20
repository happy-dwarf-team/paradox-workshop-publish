# The checksum warning

Some Paradox games compute a checksum over parts of the game files. A mod that changes that checksum has two effects:

- Achievements are off while it is enabled.
- In multiplayer, everyone needs the same mods.

Many modders want to know before a release whether their mod does this, because it decides whether players can use it on an achievement run.

`pdx-ws` checks the published copy against *checksum rules*: which folders and file types the game counts. For **Europa Universalis IV** the rules are verified on 1.37.5. Other games show "the checksum check is not available" until their rules are verified on a real install.

## What counts as a change

A mod changes the checksum when either of these holds:

- **A checksummed file:** a file of the published copy is in checksummed scope, e.g. `common/ideas/my_ideas.txt`.
- **A `replace_path`:** the descriptor has a `replace_path` that covers checksummed scope, even when the mod ships no file there.

Copying a vanilla file unchanged still counts as a change, so it is a known false positive.

## What you see

For EU4, the plan and the action's summary say:

> the published copy changes the EU4 checksum: players lose achievements while this mod is enabled, and multiplayer partners need the same mods; ironman itself still works

They list the matching files and `replace_path` values: about 15, then "and N more". In GitHub Actions each match is also a warning annotation on the file, or on the descriptor for `replace_path`. `pdx-ws` never claims that achievements are kept; it only reports what it found.

## The `checksum` setting

| Value | Behaviour |
| --- | --- |
| `warn` (default) | The plan, the summary and the annotations show the change; the publish goes on. |
| `keep` | A checksum change is an error (exit 2), and nothing is published. Use it for mods that promise to keep achievements. |
| `ignore` | No check and no warning. |

`keep` on a game without rules cannot be enforced, and the plan warns about that.

## Which rules are used

The rules are built into `pdx-ws`, with:

- the game version they apply to;
- the version they were verified on;
- their source.

They are picked by the descriptor's `supported_version`. When no table matches, the newest one is used, and the plan says so.

When `client` mode runs on a computer with the game installed, it also compares the installed `checksum_manifest.txt` with the built-in rules and reports any drift as a warning.
