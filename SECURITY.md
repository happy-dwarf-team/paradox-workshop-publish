# Security policy

paradox-workshop-publish (`pdx-ws` and its GitHub Action) publishes mods with a
saved Steam login: the Steam session, which can publish to every Workshop item
the account manages. Please report security problems privately, not in public
issues. What the session can do and how to revoke it is in
[docs/guide/security.md](docs/guide/security.md).

## Reporting a vulnerability

Use GitHub's private vulnerability reporting:
[Report a vulnerability](https://github.com/happy-dwarf-team/paradox-workshop-publish/security/advisories/new).
Include the pdx-ws version (`pdx-ws --version` or the action tag), the mode
(`steamcmd` or `client`) and where it ran, how to reproduce the problem, and
what an attacker gains. Do not send a working Steam session.

This is a volunteer project; reports are handled on a best-effort basis. We
will confirm the report, agree on a fix and a disclosure date with you, and
credit you in the advisory unless you prefer otherwise.

## Supported versions

Only the latest release receives security fixes.

## In scope

- The Steam session leaking: printed in a log or the job summary, passed on a
  command line, left on the runner after a run, or written back.
- The action running with secrets where it should not, for example on
  `pull_request_target` or for a pull request from a fork.
- A dry run (`plan`, `pull_request` runs) that sends anything to Steam.
- Files published that `workshop.toml` and the descriptor do not select, or
  files read or written outside the mod folder and the working directory.
- The action using a `pdx-ws` binary it did not verify against the release's
  build attestation or SHA-256 (other than `pdx-ws-path`, which is for testing
  the action only).
- Release integrity: release assets that do not match `SHA256SUMS.txt` or their
  build provenance attestation.

## Out of scope

- What anyone who holds your Steam session, or who can change your workflows
  or approve the `steam-workshop` environment, can do with it. That is the
  trade-off [docs/guide/security.md](docs/guide/security.md) describes.
- `client` mode on a self-hosted runner publishing as the Steam user logged in
  there; use it only with private repositories, as the guide says.
- Vulnerabilities in Steam, steamcmd or the Steamworks SDK; report those to
  Valve. Vulnerabilities in the games themselves; report those to Paradox
  Interactive.
