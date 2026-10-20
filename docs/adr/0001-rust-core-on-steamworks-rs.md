# Rust core on steamworks-rs; the GitHub Action downloads a release binary

The core and its CLI are written in Rust on [`steamworks-rs`](https://github.com/Noxime/steamworks-rs), and the GitHub Action is a composite action that downloads a pinned, checksum-verified release binary for the runner's OS. We chose this over a JavaScript action on [`steamworks.js`](https://github.com/ceifa/steamworks.js), the binding proven in lab experiment E3, because `steamworks.js` is a thin layer over an old pinned commit of `steamworks-rs` with no release since 2024-08. Using `steamworks-rs` directly gives the `client` mode per-language titles and descriptions, item deletion and metadata, and one standalone tool that works without Node.

## Consequences

- Releases build binaries for Windows, Linux and macOS (planned with `cargo-dist`), and the action needs no Docker.
- `steamworks-rs` links `steam_api` dynamically, so every release archive ships the redistributable `steam_api` library next to the binary, even for `steamcmd` mode. Whether a third-party tool may redistribute it is checked before the `client` mode is implemented.
- Annotations and the job summary are written through workflow commands on stdout rather than `@actions/core`.
