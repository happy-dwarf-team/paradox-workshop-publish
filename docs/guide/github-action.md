# The GitHub Action

The action downloads the `pdx-ws` release of its own tag and checks its build attestation (or, without `gh`, its SHA-256). Pin a release such as <!-- x-release-please-start-version -->`@v0.2.0`<!-- x-release-please-end -->; from 1.0 there is also a floating `@v1` tag.

```yaml
- uses: happy-dwarf-team/paradox-workshop-publish@v0.2.2 # x-release-please-version
  with:
    steam-username: ${{ secrets.STEAM_USERNAME }}
    steam-session: ${{ secrets.STEAM_SESSION }}
    change-note: ${{ github.event.release.body }}
```

Run it after `actions/checkout`, in a job that names the `steam-workshop` environment (see [First-time setup](setup.md)).

## Inputs

| Input | Default | Meaning |
| --- | --- | --- |
| `mode` | `steamcmd` | How the plan reaches Steam. `client` needs a [self-hosted runner](self-hosted-client-mode.md). |
| `config` | `workshop.toml` | The path of `workshop.toml`, relative to the repository root. |
| `mod` | all mods | The `id` of the mod to publish. Separate several with spaces, commas or new lines. |
| `steam-username` | none | The Steam user name. Keep it in a secret. |
| `steam-session` | none | The Steam session from `pdx-ws session export`. Keep it in a secret. |
| `change-note` | from the changelog | The change note in Markdown, e.g. `${{ github.event.release.body }}`. It applies to every selected mod. |
| `dry-run` | `false` | `true` plans and reports without sending anything to Steam. Always true on `pull_request`. |
| `pdx-ws-version` | the action's tag | The `pdx-ws` release to use, e.g. `v0.2.2`, or `v1` for the newest 1.x. |
| `pdx-ws-path` | none | **For testing this action only.** A `pdx-ws` binary to use instead of downloading a release. It is not verified. |

## Outputs

| Output | Value |
| --- | --- |
| `item-id` | The Workshop item ID of the selected mod; empty when several mods are selected. |
| `item-url` | The item's Workshop page; empty when several mods are selected. |
| `plan-json` | The path of the publish plan of every selected mod, as `pdx-ws plan --json` writes it. |

## What it does

1. **Checks the event.**
   - On `pull_request` the run is always a dry run, so a pull request can never publish.
   - On `pull_request_target` the action refuses to run: that event gives code from the pull request access to the repository's secrets.
2. **Installs `pdx-ws`.**
   - It downloads the release archive for the runner (Linux x64, Windows x64, macOS) and verifies its build attestation with `gh attestation verify`.
   - Without `gh` on the runner, it checks the archive's SHA-256 from the release instead and prints a warning.
3. **Plans:** it runs `pdx-ws plan` and saves the JSON for `plan-json`.
4. **Publishes**, unless it is a dry run.
   - The user name and session reach `pdx-ws` only through the environment variables `PDX_WS_STEAM_USERNAME` and `PDX_WS_STEAM_SESSION`, never on the command line.
   - A failed mod does not stop the others.
5. **Reports.**
   - **Job summary:** a table of the mods with their items and results. For each mod: the tags, the visibility, the warnings, the checksum matches, the rendered description and change note, and steamcmd's error lines on failure.
   - **Annotations:** warnings on the run. A checksum change is annotated on the matching files and on the descriptor for `replace_path`.
6. **Cleans up.**
   - `pdx-ws` deletes the session after each steamcmd run and the steamcmd folder when it exits.
   - Composite actions have no post step, so the action gives `pdx-ws` its own temporary folder. A final step that runs even after failures and cancellations deletes that folder, in case `pdx-ws` was stopped before it could.
7. **Fails the job** with `pdx-ws`'s exit code when a mod was not published (1, or 3 for an expired session).

The rendered description in a pull request's summary is the easiest way to review a description change before it reaches Steam.

## steamcmd on hosted runners

- **Runners:** use `ubuntu-latest` or `windows-latest`. `steamcmd` mode does not run on macOS.
- **Linux:** `pdx-ws` installs the 32-bit `lib32gcc-s1` package that steamcmd needs. GitHub-hosted runners allow this; a self-hosted Linux runner needs it installed already, or passwordless `sudo`.
- **Download:** steamcmd comes from Valve's CDN on every run and lives only in the run's temporary folder.

## Examples

- [`publish-on-release.yml`](../examples/publish-on-release.yml): one mod, when a release is published.
- [`preview-pull-request.yml`](../examples/preview-pull-request.yml): a dry run on every pull request.
- [`release-please-monorepo.yml`](../examples/release-please-monorepo.yml): several mods with release-please.
