# First-time setup

You do these steps once per mod, or once per maintainer for the session. Afterwards, every release is published from CI.

## 1. Install `pdx-ws`

Download the archive for your system from the [releases](https://github.com/happy-dwarf-team/paradox-workshop-publish/releases): `pdx-ws-x86_64-pc-windows-msvc.zip`, `pdx-ws-x86_64-unknown-linux-gnu.tar.xz`, `pdx-ws-aarch64-apple-darwin.tar.xz` (Apple silicon) or `pdx-ws-x86_64-apple-darwin.tar.xz` (Intel Mac). You can check where it was built:

```sh
gh attestation verify pdx-ws-x86_64-pc-windows-msvc.zip --repo happy-dwarf-team/paradox-workshop-publish
```

Unpack it and keep its files together: the Steamworks `steam_api` library next to `pdx-ws` is what `client` mode and `pdx-ws init` talk to Steam through, and on Linux and macOS `pdx-ws` does not start without it. Put the folder on your `PATH`. `THIRD_PARTY_NOTICES` in the archive says whose that library is.

To build from source instead, use Rust (`rustup` picks the pinned toolchain) and copy `steam_api` from `target/release/build/steamworks-sys-*/out/` next to the binary:

```sh
git clone https://github.com/happy-dwarf-team/paradox-workshop-publish
cd paradox-workshop-publish
cargo build --release --locked
```

## 2. Write `workshop.toml`

Put a `workshop.toml` next to the mod, usually at the repository root:

```toml
schema = 1

[[mod]]
game = "eu4"
```

The [reference](workshop-toml.md) lists every key. Then run `pdx-ws plan` and read what it would publish: the files, the title and tags from the descriptor, the preview, the description and change note, and any warnings.

## 3. Create the Workshop item in `client` mode

Run this once, on a computer where:

- Steam is running and logged in to the account that will own the item;
- that account owns the game.

```sh
pdx-ws init
```

`pdx-ws init` does the following:

- It creates the item as **private**, unless `visibility` is set in `workshop.toml`.
- It uploads the mod, title, tags, preview and description.
- It writes the new ID back into the repository: `remote_file_id` in `descriptor.mod`, or `item_id` in `workshop.toml` for Victoria 3 and EU5, whose `metadata.json` has no place for it.
- If Steam says the account must first accept the Workshop legal agreement, it prints the agreement's URL.

**Commit the ID.** CI never commits anything. The ID in the repository is how every later publish finds the item.

Why `client` mode for creation? It is the only mode that can set tags, and a first upload should have them. `steamcmd` mode can create an item with `pdx-ws publish --allow-create`, but without tags; it prints the new ID for you to commit.

## 4. Save a Steam session

`steamcmd` mode logs in with a *Steam session*: steamcmd's cached login, created once with your password and Steam Guard code, then reused without them. Read [Security](security.md) before you create one.

```sh
pdx-ws session export --user my_steam_name | gh secret set STEAM_SESSION --env steam-workshop
gh secret set STEAM_USERNAME --env steam-workshop --body my_steam_name
```

`pdx-ws session export` does the following:

1. It installs steamcmd in a per-user folder: `%LOCALAPPDATA%\pdx-ws` on Windows, `~/.local/share/pdx-ws` elsewhere.
2. It runs the interactive login, where you type your password and the Steam Guard code.
3. It checks that a second login works without them.
4. It prints the session, gzipped and base64-encoded, **to stdout only**. Everything else goes to stderr.
5. It deletes its local copy, so the secret is the only one.

When stdout is a terminal, `pdx-ws` refuses to print the session there unless you pass `--stdout`. Pipe it into the secret instead.

Run it on Windows or Linux; steamcmd mode does not support macOS.

The `gh secret set … --env steam-workshop` commands need the environment to exist; create it first (next step). To share one session between several repositories, see [Several repositories in one organization](#several-repositories-in-one-organization).

When a session stops working, `publish` exits with code 3 and names `pdx-ws session export`. Run it again to replace the secret. How long a session lasts is still being measured ([#22](https://github.com/happy-dwarf-team/paradox-workshop-publish/issues/22)).

## 5. Create the `steam-workshop` environment

A GitHub *environment* keeps the secrets away from jobs that do not name it, and can require a person to approve each publish.

In the repository, open **Settings → Environments → New environment** and name it `steam-workshop`. Then:

- **Required reviewers** (recommended): add yourself or a co-maintainer. Every publish job then waits for an approval.
- **Deployment branches and tags**: choose *Selected branches and tags* and allow only your release tags, e.g. `v*`, or `*-v*` for [several mods](several-mods.md). A branch pushed by someone else then cannot reach the secrets.
- **Environment secrets**: `STEAM_SESSION` and `STEAM_USERNAME`, which the commands in step 4 set.

With the GitHub CLI, you can also create the environment without the web interface:

```sh
gh api --method PUT repos/OWNER/REPO/environments/steam-workshop
```

Then add reviewers and tag rules in the web interface.

### Several repositories in one organization

One Steam session can publish every Workshop item of its account, so an organization that publishes from several repositories can keep it once, as **organization secrets** shared with selected repositories, and renew it in one place when it expires:

```sh
pdx-ws session export --user my_steam_name | gh secret set STEAM_SESSION --org ORG --visibility selected --repos repo-a,repo-b
gh secret set STEAM_USERNAME --org ORG --visibility selected --repos repo-a,repo-b --body my_steam_name
```

Keep the `steam-workshop` environment in each repository, without its own secrets: its reviewers and tag rules still gate every publish job. An environment secret of the same name would take precedence over the organization's, so delete those. The trade-off: an organization secret reaches every workflow of the selected repositories, not only the jobs of the environment, so anyone who can push a workflow there can read the session. Choose it only when the people with write access are the people you would trust with the session; pull requests from forks and Dependabot get no secrets either way.

## 6. Add the workflows

Copy [`publish-on-release.yml`](../examples/publish-on-release.yml) and [`preview-pull-request.yml`](../examples/preview-pull-request.yml) into `.github/workflows/`. Pull requests then show a dry-run summary with the rendered description, and publishing a GitHub release publishes the mod. See [The GitHub Action](github-action.md) for every input.
