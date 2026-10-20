# paradox-workshop-publish

Publish Paradox Interactive game mods (Europa Universalis IV first, then other Clausewitz and Jomini games) to the Steam Workshop from GitHub Actions or from your own computer, using the game's own mod descriptor instead of hand-written upload files.

**Status: in development.** Nothing here is usable yet. The design is in the [v1 specification](docs/spec/v1.md), which also lists the implementation sequence; the reasoning is in the [planning map](https://github.com/happy-dwarf-team/paradox-workshop-publish/issues?q=label%3Awayfinder%3Amap) and its tickets. Terms are defined in [GLOSSARY.md](GLOSSARY.md).

## Development

The `pdx-ws` CLI is a Rust workspace; `rust-toolchain.toml` pins the toolchain. CI runs the same checks on Linux and Windows:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Release archives are configured for [cargo-dist](https://axodotdev.github.io/cargo-dist/) in `dist-workspace.toml` (`dist plan` lists them); nothing is published yet. Versions and the changelog are managed by release-please from [Conventional Commits](https://www.conventionalcommits.org/) in pull request titles.
