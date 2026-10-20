# User guide

`pdx-ws` publishes a Paradox mod to its Steam Workshop item. The GitHub Action runs it in CI.

1. [First-time setup](setup.md): install `pdx-ws`, create the Workshop item, save a Steam session, create the `steam-workshop` environment.
2. [`workshop.toml` reference](workshop-toml.md): every key, and what each game needs.
3. [The GitHub Action](github-action.md): inputs, outputs, dry runs, the job summary.
4. [Description and change notes](description-and-change-notes.md): Markdown to BBCode, changelogs and the release body.
5. [The checksum warning](checksum.md): when an EU4 mod costs players their achievements.
6. [Several mods in one repository](several-mods.md): `id`, `[defaults]`, release-please monorepo tags.
7. [Security](security.md): what the Steam session can do, how to revoke it, and how to keep it safe.
8. [Self-hosted runner for `client` mode](self-hosted-client-mode.md).

The [examples](../examples) are complete workflows you can copy:

- [`publish-on-release.yml`](../examples/publish-on-release.yml): one mod, published when you publish a GitHub release;
- [`preview-pull-request.yml`](../examples/preview-pull-request.yml): a dry run on every pull request;
- [`release-please-monorepo.yml`](../examples/release-please-monorepo.yml) with [`release-please-config.json`](../examples/release-please-config.json): several mods, each with its own `<id>-vX.Y.Z` releases.

## The CLI in one screen

| Command | Does | Logs in to Steam |
| --- | --- | --- |
| `pdx-ws plan` | Prints the *publish plan*: files, metadata, target item, warnings. Sends nothing. | Never |
| `pdx-ws publish` | Executes the plan in `steamcmd` mode (default) or `client` mode. | Yes |
| `pdx-ws init` | Creates the Workshop item in `client` mode and writes its ID back. | Yes |
| `pdx-ws session export` | Logs in to steamcmd interactively and prints the *Steam session* to stdout. | Yes |

Common flags:

| Flag | Meaning |
| --- | --- |
| `--config <path>` | The config file. Default: `workshop.toml`. |
| `--mod <id>` | Only this mod; repeatable. Default: every mod. |
| `--mode steamcmd\|client` | How `publish` reaches Steam. Default: `steamcmd`. |
| `--change-note <text>` | The change note, in Markdown. |
| `--allow-create` | Lets `steamcmd` mode create a missing item (without tags). |
| `--fail-fast` | Stop at the first mod that fails. |
| `--json` | JSON instead of text, for `plan` and `publish`. |

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | Success; warnings allowed |
| 1 | At least one mod failed to publish |
| 2 | A configuration or validation error before any upload |
| 3 | The Steam session is missing or expired; create a new one with `pdx-ws session export` |

In `steamcmd` mode, credentials come only from the environment variables `PDX_WS_STEAM_USERNAME` and `PDX_WS_STEAM_SESSION`, never from the command line, so they do not show up in the process list.
