# paradox-workshop-publish

Publishing mods for Paradox Interactive games to the Steam Workshop.

## Language

**Mod**:
A folder with a game's mod descriptor (`descriptor.mod` or `metadata.json`); the source that gets published.
_Avoid_: package, addon

**Workshop item**:
The Steam object, identified by its `publishedfileid`, that one mod corresponds to one-to-one.
_Avoid_: upload, listing, Workshop mod

**Publish**:
Create or update a Workshop item from a mod.
_Avoid_: upload, deploy, release (as synonyms)

**Game profile**:
What publishing needs to know about one game: its Steam app ID, its descriptor format, where its thumbnail lives and its game-specific checks.
_Avoid_: game config, game settings

**Publish plan**:
Everything one publish will do, computed before any upload: the files, the Workshop metadata, the Workshop item it targets, the warnings, and the changes to write back into the mod.
_Avoid_: manifest, job, build

**Mode**:
How a publish plan reaches Steam: `steamcmd` (a cached steamcmd session, for hosted CI runners) or `client` (the logged-in Steam client, for a modder's computer or a self-hosted runner). Modes are named by mechanism, never by where they run.
_Avoid_: CI mode, local mode, backend

**Published copy**:
The temporary copy of a mod that is actually sent to Steam: excluded files left out, and the descriptor cleaned of local-only keys and given the Workshop item's ID. The mod in the repository is never published directly.
_Avoid_: staging folder, build output

**Steam session**:
A modder's cached steamcmd login, created once interactively and stored as a CI secret, which lets `steamcmd` mode log in without a password or Steam Guard code.
_Avoid_: config.vdf, sentry file, token, credentials
