# Self-hosted runner for `client` mode

`client` mode publishes through a running, logged-in Steam client. Unlike `steamcmd` mode, it can set **tags** and run the installed-game checksum cross-check. GitHub-hosted runners have no Steam client, so `client` mode in CI needs a runner of your own.

Most modders do not need this: create the item with `pdx-ws init` on your own computer once, and publish releases from hosted runners in `steamcmd` mode. Set this up only if CI must set tags.

## Requirements

- **A private repository.** Any workflow that runs on the runner can publish as the logged-in Steam user. In a public repository, a pull request from a fork could run a workflow on your machine. See [Security](security.md).
- **A Windows or Linux machine** that stays on while releases run, with Steam installed.
- **A Steam account that owns the game** and owns the item (or is a contributor on it).

## Recipe

1. **Create a dedicated OS user**, e.g. `steam-ci`, with no administrator rights. Steam refuses to talk to a program running as a different OS user. So the Steam client and the runner must both run as this user, and nothing else of yours should.
2. **Log in to Steam as that user** and tick *Remember my password*. Set Steam to start when that user logs in, and make sure the user stays logged in, e.g. with automatic login on a machine you control.
3. **Install the GitHub Actions runner as that user** with the steps from *Settings → Actions → Runners → New self-hosted runner*. Give it a label such as `steam-client`. Run it as a service under the `steam-ci` account:
   - on Windows, accept the service option of `config.cmd` and give it the `steam-ci` account;
   - on Linux, `sudo ./svc.sh install steam-ci`.

   The Steam client must be reachable from the runner's processes. If it is not, run the runner in the logged-in `steam-ci` session instead (`run.cmd` or `./run.sh`). This can happen with a Windows service, which runs outside the user's desktop session.
4. **Install the game** if you want the checksum cross-check against the installed `checksum_manifest.txt`.
5. **Use the runner only from a private repository**, and only in jobs that publish:

   ```yaml
   jobs:
     publish:
       runs-on: [self-hosted, steam-client]
       environment: steam-workshop
       steps:
         - uses: actions/checkout@v7
           with:
             persist-credentials: false
         - uses: happy-dwarf-team/paradox-workshop-publish@v0.2.0 # x-release-please-version
           with:
             mode: client
             change-note: ${{ github.event.release.body }}
   ```

   `client` mode needs no `steam-username` or `steam-session`; the logged-in client is the credential. Keep the environment for its required reviewers and tag rules.

## Keeping the runner safe

- Do not register this runner on public repositories or organisation-wide.
- Keep Steam and the runner up to date. Restrict who can push workflow changes, because a workflow change is how someone would misuse the runner.
- If the machine or the `steam-ci` account may be compromised, use "Deauthorize all devices" in Steam Guard and change the Steam password.
