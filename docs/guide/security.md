# Security

Publishing from CI means storing a Steam login in GitHub. Read this page before you create a Steam session, and decide whether the trade-off suits you.

## What the Steam session can do

The *Steam session* is steamcmd's cached login for your account: the `config.vdf` that steamcmd writes after you typed your password and Steam Guard code once. `pdx-ws session export` packs it, and `steamcmd` mode puts it back on the runner for each publish.

**Whoever has the session can:**

- log in to steamcmd as you;
- publish, update and download content with your account, for any Workshop item the account can manage, not only this mod.

**It does not contain:**

- your password;
- your Steam Guard secret.

So it cannot be used to sign in to the Steam client or the website, or to change your password. It lasts until Steam expires it or you revoke it.

`pdx-ws` treats it as a secret:

- It reads the session from the environment variable `PDX_WS_STEAM_SESSION`, never from the command line.
- It never prints the session.
- It masks your SteamID in steamcmd's output.
- It deletes the session file after each steamcmd run, even on failure.
- It never writes back the session that steamcmd refreshes on the runner.

## How to revoke it

Either of these makes every saved session worthless:

- **"Deauthorize all devices"** on the *Manage Steam Guard* page of your Steam account.
- **Changing your Steam password.**

Both also sign you out of Steam everywhere else. Afterwards, create a new session with `pdx-ws session export` and replace the `STEAM_SESSION` secret.

## Keeping it safe

- **Keep the session and user name as environment secrets**, in a `steam-workshop` environment, not as repository secrets. Only jobs that name the environment can read them.
- **Publish only from tags or releases.** Limit the environment's deployment branches and tags to your release tags (e.g. `v*`, or `main` for the release-please setup), so a pushed branch cannot use the secrets. Add **required reviewers**, so a person approves each publish.
- **Never run the action on `pull_request_target`.** It refuses to, because that event runs code from a pull request with the repository's secrets. Use `pull_request` for previews; those runs are always dry runs and need no secrets.
- **Renew the session when collaborators change.** Anyone who could change the workflows, or approve the environment, could have used it. When someone leaves:
  1. revoke the session ("Deauthorize all devices");
  2. export a new one;
  3. replace the secret.
- **Consider a separate Steam account** that owns the game and is a contributor on the item, if you prefer not to store your main account's session.

## `client` mode on self-hosted runners

On a self-hosted runner, `client` mode publishes through the Steam client that is logged in there. **Any workflow that runs on that runner can publish as that Steam user.** Use it only with **private repositories**, where outside pull requests cannot run workflows on your machine. See [Self-hosted runner for `client` mode](self-hosted-client-mode.md).

## The Steam Subscriber Agreement

Section 4.C of the [Steam Subscriber Agreement](https://store.steampowered.com/subscriber_agreement/) restricts "Automation" of Steam accounts. Two facts bear on it:

- The examples the clause gives do not include publishing Workshop content.
- Valve documents steamcmd for uploading Workshop content from scripts and build pipelines.

`pdx-ws` uses steamcmd and the Steamworks API only in those documented ways. It is not legal advice; whether this fits your use is your judgement.
