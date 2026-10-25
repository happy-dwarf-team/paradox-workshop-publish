# paradox-workshop-publish: repository rules

- Publishes Paradox game mods to the Steam Workshop from GitHub Actions (`steamcmd` mode) or a
  modder's PC (`client` mode), driven by the mod descriptor. The design is the accepted
  [v1 specification](docs/spec/v1.md), decisions are in [docs/adr/](docs/adr/), terms in
  [GLOSSARY.md](GLOSSARY.md), and the user guide in [docs/guide/](docs/guide/README.md).
- Run the checks from the README's Development section (`cargo fmt`, `cargo clippy`,
  `cargo test`) before finishing a change; CI runs them on Linux and Windows.
- Pull request runs are dry runs that send nothing to Steam. Only the `e2e` workflow
  publishes, to the test item, and every run waits for a reviewer of its environment; start
  it only when the user asks.
- Titles follow Conventional Commits; release-please computes versions and the changelog
  from them. English in code and docs. Do not imply affiliation with Paradox Interactive.
- The agent owns the pull request flow: it works on a branch with Conventional Commits,
  pushes it and opens the PR as a draft. Once ready (green CI, the change tested as above,
  no blocking review findings, not put on hold by the user) it marks the PR ready and
  squash-merges it; GitHub does not allow auto-merge on a draft. Pull requests from other
  contributors get a review and a recommendation; the maintainer decides on merging them.
  Do not bypass CI or branch protection, and do not change repository settings beyond what
  auto-merge needs.
