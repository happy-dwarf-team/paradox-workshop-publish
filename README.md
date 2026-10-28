# paradox-workshop-publish

[![ci](https://github.com/happy-dwarf-team/paradox-workshop-publish/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/happy-dwarf-team/paradox-workshop-publish/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/happy-dwarf-team/paradox-workshop-publish)](https://github.com/happy-dwarf-team/paradox-workshop-publish/releases/latest)
[![License: MIT](https://img.shields.io/github/license/happy-dwarf-team/paradox-workshop-publish)](LICENSE)

Publish Paradox Interactive game mods to the Steam Workshop from GitHub Actions or from your own computer. `pdx-ws` reads the game's own mod descriptor (`descriptor.mod` or `.metadata/metadata.json`) instead of a hand-written upload file. It also:

- uploads a cleaned copy of the mod, without `.git`, workflows or local descriptor paths;
- converts a Markdown description and change note to Steam BBCode;
- warns before a release that would cost EU4 players their achievements.

Supported games: Europa Universalis IV, Hearts of Iron IV, Stellaris, Imperator: Rome, Crusader Kings III, Victoria 3 and Europa Universalis V.

> **Status: 0.x.** The configuration and the action's inputs may still change before 1.0. Download `pdx-ws` from the [releases](https://github.com/happy-dwarf-team/paradox-workshop-publish/releases); each archive holds the binary and the Steamworks `steam_api` library that `client` mode needs next to it.

## Two modes

| Mode | How it reaches Steam | Where it runs | Sets tags |
| --- | --- | --- | --- |
| `steamcmd` | steamcmd with a saved *Steam session* | GitHub-hosted runners, any CI | No |
| `client` | the Steam client you are logged in to | your computer, a self-hosted runner | Yes |

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

3. **Create the Workshop item**, once, with Steam running: `pdx-ws init`. It writes the item ID into the descriptor; commit that change.

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
         - uses: happy-dwarf-team/paradox-workshop-publish@v0.2.2 # x-release-please-version
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

The binary is `target/release/pdx-ws` (`pdx-ws.exe` on Windows). `client` mode and `pdx-ws init` also need the Steamworks `steam_api` library next to the binary (`steam_api64.dll` on Windows, `libsteam_api.so` on Linux; the other commands run without it on Windows). The build copies it into `target/release/build/steamworks-sys-*/out/`. CI runs the same checks on Linux and Windows:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

The `action` workflow runs the GitHub Action from the checkout as a dry run on fixture mods; it sends nothing to Steam.

Release archives are configured for [cargo-dist](https://axodotdev.github.io/cargo-dist/) in `dist-workspace.toml` (`dist plan` lists them); nothing is published yet. Versions and the changelog are managed by release-please from [Conventional Commits](https://www.conventionalcommits.org/) in pull request titles.

## License

MIT, see [LICENSE](LICENSE). paradox-workshop-publish is an unofficial community project, not affiliated with or endorsed by Paradox Interactive.
