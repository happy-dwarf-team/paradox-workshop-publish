# paradox-workshop-publish

Publish Paradox Interactive game mods to the Steam Workshop from GitHub Actions or from your own computer. `pdx-ws` reads the game's own mod descriptor (`descriptor.mod` or `.metadata/metadata.json`) instead of a hand-written upload file. It also:

- uploads a cleaned copy of the mod, without `.git`, workflows or local descriptor paths;
- converts a Markdown description and change note to Steam BBCode;
- warns before a release that would cost EU4 players their achievements.

Supported games: Europa Universalis IV, Hearts of Iron IV, Stellaris, Imperator: Rome, Crusader Kings III, Victoria 3 and Europa Universalis V.

> **Status: pre-release (0.x).** There are no release binaries yet, and the GitHub Action cannot download `pdx-ws` until they exist. Both are coming in 0.1.0, together with `client` mode and `pdx-ws init`. Until then you can build `pdx-ws` from source (see [Development](#development)) to try `plan`, `publish` in `steamcmd` mode and `session export`.

## Two modes

| Mode | How it reaches Steam | Where it runs | Sets tags |
| --- | --- | --- | --- |
| `steamcmd` | steamcmd with a saved *Steam session* | GitHub-hosted runners, any CI | No |
| `client` (coming in 0.1.0) | the Steam client you are logged in to | your computer, a self-hosted runner | Yes |

You create the Workshop item once on your computer with `pdx-ws init` in `client` mode. After that, CI publishes every release in `steamcmd` mode.

## Quick start

1. **Describe the mod in `workshop.toml`** at the repository root:

   ```toml
   schema = 1

   [[mod]]
   game = "eu4"
   description = "workshop/description.md"
   changelog = "CHANGELOG.md"
   ```

2. **Check what would be published.** `pdx-ws plan` sends nothing:

   ```sh
   pdx-ws plan
   ```

3. **Create the Workshop item**, once, with Steam running: `pdx-ws init` (coming in 0.1.0). It writes the item ID into the descriptor; commit that change.

4. **Save a Steam session as a secret** of a `steam-workshop` environment:

   ```sh
   pdx-ws session export --user my_steam_name | gh secret set STEAM_SESSION --env steam-workshop
   gh secret set STEAM_USERNAME --env steam-workshop --body my_steam_name
   ```

5. **Publish on every GitHub release:**

   ```yaml
   on:
     release:
       types: [published]

   jobs:
     publish:
       runs-on: ubuntu-latest
       environment: steam-workshop
       permissions:
         contents: read
       steps:
         - uses: actions/checkout@v7
           with:
             persist-credentials: false
         - uses: happy-dwarf-team/paradox-workshop-publish@v0.1.0 # coming in 0.1.0
           with:
             steam-username: ${{ secrets.STEAM_USERNAME }}
             steam-session: ${{ secrets.STEAM_SESSION }}
             change-note: ${{ github.event.release.body }}
   ```

The [user guide](docs/guide/README.md) covers each step, the [security notes](docs/guide/security.md) say what the Steam session can do and how to revoke it, and [`docs/examples/`](docs/examples) has complete workflows.

## Documentation

- [User guide](docs/guide/README.md): setup, `workshop.toml`, the action, descriptions and change notes, the checksum warning, several mods, security, and a self-hosted runner for `client` mode.
- [v1 specification](docs/spec/v1.md): the design and the implementation sequence. The reasoning is in the [planning map](https://github.com/happy-dwarf-team/paradox-workshop-publish/issues?q=label%3Awayfinder%3Amap) and its tickets.
- [GLOSSARY.md](GLOSSARY.md): the terms this project uses.

## Development

The `pdx-ws` CLI is a Rust workspace; `rust-toolchain.toml` pins the toolchain. To build it:

```sh
git clone https://github.com/happy-dwarf-team/paradox-workshop-publish
cd paradox-workshop-publish
cargo build --release --locked
```

The binary is `target/release/pdx-ws` (`pdx-ws.exe` on Windows). CI runs the same checks on Linux and Windows:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

The `action` workflow runs the GitHub Action from the checkout as a dry run on fixture mods; it sends nothing to Steam.

Release archives are configured for [cargo-dist](https://axodotdev.github.io/cargo-dist/) in `dist-workspace.toml` (`dist plan` lists them); nothing is published yet. Versions and the changelog are managed by release-please from [Conventional Commits](https://www.conventionalcommits.org/) in pull request titles.
