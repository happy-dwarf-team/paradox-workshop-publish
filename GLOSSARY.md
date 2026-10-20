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
