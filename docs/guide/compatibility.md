# Compatibility

From 1.0, `pdx-ws` and the action follow [Semantic Versioning](https://semver.org/). Everything under [Stable](#stable) keeps working through every 1.x release, so `@v1` will not break a workflow that follows these rules. A change that breaks it is version 2, which gets its own `@v2` tag; `@v1` then stays on the last 1.x release.

## Stable

**`workshop.toml`:**

- `schema = 1`, and every key in the [reference](workshop-toml.md) with its meaning, default and allowed values;
- what paths are relative to;
- strict keys: an unknown key stays an error.

**The CLI:**

- the commands `plan`, `publish`, `init` and `session export`, with their flags and defaults;
- the [exit codes](README.md#the-cli-in-one-screen) 0, 1, 2 and 3;
- the environment variables `PDX_WS_STEAM_USERNAME` and `PDX_WS_STEAM_SESSION`;
- the session on stdout of `session export`, and everything else on stderr.

**The JSON of `plan --json` and `publish --json`:**

- `schema: 1`, and the names, types and meaning of the fields;
- the values of these fields:

  | Field | Values |
  | --- | --- |
  | `outcome` | `published`, `unchanged`, `created`, `failed`, `skipped` |
  | `checksum.status` | `ignored`, `unavailable`, `unchanged`, `changes` |
  | `item.source` | `remote_file_id`, `workshop_toml` |
  | `metadata.change_note_source.kind` | `explicit`, `changelog`, `fallback` |

- the meaning of each warning `code`: `no-preview`, `tag-limit`, `no-item-id`, `checksum-unavailable`, `checksum-change`, `localized-client-only`, `unchanged`.

**The action:**

- the inputs and outputs in the [guide](github-action.md) with their defaults, except `pdx-ws-path`;
- dry runs on `pull_request` and the refusal on `pull_request_target`;
- failing the job with `pdx-ws`'s exit code when a mod is not published.

**The releases:**

- the tag `vX.Y.Z` and the floating major tag `v1`;
- the archive names `pdx-ws-<target>.zip` and `pdx-ws-<target>.tar.xz` for the four targets in [setup](setup.md), with the binary and `steam_api` inside;
- `SHA256SUMS.txt` and the build attestations.

## What a 1.x release can add

These are not breaking changes, so write your scripts and files with them in mind:

- **New optional keys in `workshop.toml`.** Strict keys mean that an older `pdx-ws` rejects a file that uses one, so update `pdx-ws` first.
- **New flags, and new action inputs and outputs**, with defaults that keep today's behaviour.
- **New fields in the JSON.** Ignore fields you do not know.
- **New warning codes.** Treat an unknown code as a warning, not an error.
- **New games.**

## Not stable

These can change in any release:

- **Text for people:** the text output of every command, warning and error messages, the job summary and the annotations. Parse `--json` and match on warning codes instead.
- **Internals:** the hidden `github-report` command, the action's `pdx-ws-path` input, and the other files next to `plan-path`.
- **The checksum rules.** They follow the game. A new rule table can make a mod that passed before warn, or fail with `checksum = "keep"`.
- **The rendered BBCode.** A fix to the converter can change how a description or change note comes out; `pdx-ws plan` shows the result.
- **What Steam and steamcmd do.** A change on Valve's side can break a mode, and we can only follow it.
- **The Rust toolchain and the Steamworks SDK** used to build `pdx-ws`.

A bug fix that makes `pdx-ws` do what these pages say is not a breaking change, even if something came to rely on the bug. A security fix may have to break a promise here; its release notes will say so.
