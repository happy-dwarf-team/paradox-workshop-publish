# Redistributing the steam_api library in release archives

Research for [#13](https://github.com/happy-dwarf-team/paradox-workshop-publish/issues/13), checked on 2026-10-20. [ADR 0001](../adr/0001-rust-core-on-steamworks-rs.md) plans to ship `steam_api64.dll`, `libsteam_api.so` and `libsteam_api.dylib` next to the binary in every release archive.

This is not legal advice. Each section keeps three things apart: **Documents** (what Valve's texts say, quoted), **Practice** (what established projects do), and **Inference** (our reading).

## Short answer

- The [Steamworks SDK Access Agreement](https://partner.steamgames.com/documentation/sdk_access_agreement) lets its Licensee redistribute the `redistributable_bin` files together with the Licensee's own software in object code form. A release archive that holds our binary plus `steam_api` fits that wording.
- The Licensee is whoever downloads or uses the SDK and so accepts the agreement. It is not limited to paying Steamworks partners.
- The [Steam Subscriber Agreement](https://store.steampowered.com/subscriber_agreement/) does not grant this right, and forbids copying software you got through Steam. So the files must come from the SDK, never from a game folder.
- Every binding we checked ships the binaries, in its repository, its package or both: steamworks-rs, steamworks.js, Steamworks.NET, Facepunch.Steamworks and the GodotSteam plugin. So do Workshop uploaders built on them. Only steamworks-ffi-node refuses.
- Our MIT licence covers our own code only. The Valve files must be labelled as Valve's, under Valve's terms.
- Recommendation: keep ADR 0001's plan, with the conditions under [Options ranked](#options-ranked).

## 1. What Valve's documents say

### Steamworks SDK Access Agreement

Source: <https://partner.steamgames.com/documentation/sdk_access_agreement>. Anyone can read it without logging in. The page shows no version or date. Its footer carries a generic "Valve Confidential" banner, although the page is publicly served.

- **Grant, §1.1:** "Valve hereby grants Licensee a nonexclusive, royalty-free, terminable, worldwide, nontransferable license to:"
  - "(a) use and locally reproduce the SDK in source code form, solely to develop the Licensee Software; and"
  - "(b) reproduce and distribute the part of the SDK provided inside the folder named redistributable_bin (the "SDK Redistributables") along with the Licensee Software in object code form."
- **Licensee Software, recitals:** "Licensee wishes to develop a game or application software designed to take advantage of the Steamworks Services (the "Licensee Software")". The "Steamworks Services" listed there include "user authentication, in-app purchasing and trading, leaderboards, matchmaking, stats and achievements". The list is introduced by "including, but not limited to".
- **Who is the Licensee, §3.1:** "This Agreement shall become effective as of the date Licensee downloads or installs the SDK." The preamble also says: "BY DOWNLOADING AND/OR USING THE SDK YOU INDICATE YOUR ACCEPTANCE OF THIS AGREEMENT."
- **Who can get the SDK:** the page tells a non-partner account: "Your Steam account is not currently associated with any active Steamworks partner. If you are looking for access to the Steamworks SDK and documentation, please read and agree to the Steamworks SDK Access Agreement below."
- **Other clauses that matter:**
  - §1.3: "Valve reserves all rights not explicitly granted herein."
  - §2.2: the Licensee is "solely responsible for any and all Licensee Software" and indemnifies Valve.
  - §2.3: no right to Valve trademarks, and the Licensee must avoid anything "that can be incorrectly interpreted as a cooperation or partnership between Valve and Licensee".
  - §2.4: software that interacts with Steamworks Services must talk to them "always through the application programming interface (API) provided by the SDK Redistributables".
  - §3.2: "Valve may terminate this Agreement immediately upon written (including email) notice". The agreement also ends automatically on breach.
  - §6.2: the agreement cannot be assigned without consent.
- **What it does not say:** nothing about open-source projects, public source repositories, or sublicensing to downstream users.

### Steamworks documentation

- **[Steamworks API Overview](https://partner.steamgames.com/doc/sdk/api)** describes how a game ships these files:
  - "Copy the relevant redistributable files from redistributable_bin into an appropriate place in your project folder."
  - "You must also ship the steam_api[64].dll in your run-time directory".
  - For Linux, ship `libsteam_api.so`: "You must both link to and ship this along side your executable."
- **[Distributing Open Source Applications on Steam](https://partner.steamgames.com/doc/sdk/uploading/distributing_opensource):** "Common permissive and acceptable licenses includes MIT License, BSD 3-clause and 4-clause, Apache 2.0 and WTFPL." Copyleft licences such as GPL are "problematic". The page covers apps distributed *via Steam*. It does not address redistributing `steam_api` outside Steam.
- **[Steamworks SDK](https://partner.steamgames.com/doc/sdk)** links the download at `https://partner.steamgames.com/downloads/list`.
  - Observed on 2026-10-20 with unauthenticated `curl`: [`/downloads/steamworks_sdk.zip`](https://partner.steamgames.com/downloads/steamworks_sdk.zip) and `/downloads/list` return `302` to `https://partner.steamgames.com?goto=...`, the login page.
  - So the official SDK needs a logged-in Steam account that has accepted the agreement. We did not log in.

### Steam Subscriber Agreement (SSA)

Source: <https://store.steampowered.com/subscriber_agreement/>, last updated 2026-09-10.

- **§2.G:** "you may not, in whole or in part, copy, photocopy, reproduce, publish, distribute, translate, reverse engineer, [...] the Content and Services or any software accessed via Steam without the prior consent, in writing, of Valve."
- **§2.G again:** you are not entitled to "sell, grant a security interest in or transfer reproductions of the Content and Services to other parties in any way".
- **§2.B, "Developer Tools":** this section concerns tools such as the Source SDK and Source Filmmaker, not the Steamworks SDK.
- **Related, not about redistribution (§4.C):** "You may not use any form of scripts, bots, macros, or other non-human-controlled systems ("Automation") to interact with Content and Services on Steam in any manner". The examples listed are account creation, faking statistics, earning rewards and adjudication systems. Workshop publishing from CI is not among them. This deserves its own ticket.

### Inference

- §1.1(b) is the only clause that grants redistribution, and it fits a release archive: our object-code binary plus the unmodified SDK Redistributables.
- A Workshop publisher plausibly counts as "application software designed to take advantage of the Steamworks Services", because the list of services is open-ended. The agreement does not spell this out.
- The right belongs to the person or organisation that accepted the agreement, and it cannot be assigned (§6.2). So someone on the team should accept it and be the Licensee for releases. Pulling a copy from another project's package is weaker ground.
- Nothing grants the right to sublicense these files under MIT.
- Taking `steam_api64.dll` out of a game's install folder and shipping it falls under the SSA's §2.G prohibition.

## 2. What open-source bindings do

| Project | Licence | `steam_api` binaries in the repo | In published packages |
|---|---|---|---|
| [Noxime/steamworks-rs](https://github.com/Noxime/steamworks-rs) | MIT / Apache-2.0, "except for the files in `steamworks-sys/lib/steam/`" ([README](https://github.com/Noxime/steamworks-rs#license)) | Yes, [`steamworks-sys/lib/steam/redistributable_bin`](https://github.com/Noxime/steamworks-rs/tree/master/steamworks-sys/lib/steam/redistributable_bin), SDK 1.65 ([commit 704c376](https://github.com/Noxime/steamworks-rs/commit/704c376)) | Yes, in the [`steamworks-sys` crate on crates.io](https://docs.rs/crate/steamworks-sys/latest/source/lib/steam/redistributable_bin/win64/). `build.rs` copies the library to `OUT_DIR` and exposes `DEP_STEAM_API_LIB_DIR` ([README](https://github.com/Noxime/steamworks-rs#help-i-cant-run-my-game)). |
| [ceifa/steamworks.js](https://github.com/ceifa/steamworks.js) | MIT | Yes, [`sdk/redistributable_bin`](https://github.com/ceifa/steamworks.js/tree/main/sdk/redistributable_bin) | Yes: [npm 0.4.0](https://unpkg.com/steamworks.js@0.4.0/?meta) contains `dist/win64/steam_api64.dll` and `dist/osx/libsteam_api.dylib`. The README tells Electron apps to copy them into their build. |
| [rlabrecque/Steamworks.NET](https://github.com/rlabrecque/Steamworks.NET) | MIT ([LICENSE.txt](https://github.com/rlabrecque/Steamworks.NET/blob/master/LICENSE.txt)) | Yes, `com.rlabrecque.steamworks.net/Plugins/` (`libsteam_api.so`, `steam_api.bundle`, and so on) | The Unity package is the repository folder |
| [Facepunch/Facepunch.Steamworks](https://github.com/Facepunch/Facepunch.Steamworks) | MIT ("MIT - do whatever you want", [README](https://github.com/Facepunch/Facepunch.Steamworks#license)) | Yes, `Facepunch.Steamworks/steam_api64.dll` and `UnityPlugin/redistributable_bin` | Yes, [NuGet 2.3.3](https://api.nuget.org/v3/catalog0/data/2020.02.28.20.10.29/facepunch.steamworks.2.3.3.json) contains `content/steam_api64.dll` |
| [GodotSteam](https://codeberg.org/godotsteam/godotsteam) (moved from [GitHub](https://github.com/GodotSteam/GodotSteam) to Codeberg) | MIT ([license.md](https://codeberg.org/godotsteam/godotsteam/src/branch/godot4/license.md)) | The module branch has only `sdk/put SDK here`. The [`gdextension-plugin` branch](https://codeberg.org/godotsteam/godotsteam/src/branch/gdextension-plugin/addons/godotsteam) ships `win64/steam_api64.dll`, `linux64/libsteam_api.so` and `osx/libsteam_api.dylib`. | The plugin's `.gdextension` lists them as export [`dependencies`](https://codeberg.org/godotsteam/godotsteam/src/branch/gdextension/godotsteam.gdextension) |
| [ArtyProf/steamworks-ffi-node](https://github.com/ArtyProf/steamworks-ffi-node) | MIT | No | No. The README says to download the SDK "required separately due to licensing". [STEAMWORKS_SDK_SETUP.md](https://github.com/ArtyProf/steamworks-ffi-node/blob/main/docs/STEAMWORKS_SDK_SETUP.md) says the redistributables "cannot be bundled with this package due to Valve's licensing terms". It cites no clause. |

Tools built on these bindings that publish to the Workshop:

- [nozwock/steam-workshop-uploader](https://github.com/nozwock/steam-workshop-uploader) uses steamworks-rs 0.13 and is GPL-3.0. Its [release workflow](https://github.com/nozwock/steam-workshop-uploader/blob/main/.github/workflows/release.yaml) zips "the binary and Steamworks SDK library" from the steamworks-sys build output into each release.
- [EliteMasterEric/SteamWorkshopUploader](https://github.com/EliteMasterEric/SteamWorkshopUploader) is MIT, built on GodotSteam, and vendors `third_party/GodotSteam/win64/steam_api64.dll` and others.
- [DarkbordermanModding/steam-workshop-uploader](https://github.com/DarkbordermanModding/steam-workshop-uploader) is MIT and attaches `steam_api64.dll` as a [release asset](https://github.com/DarkbordermanModding/steam-workshop-uploader/releases).

Issues and enforcement:

- No issue in these repositories discusses the redistribution licence. We searched "license" and "redistribut" in their GitHub issues with `gh search issues`.
- steamworks-rs [#63](https://github.com/Noxime/steamworks-rs/issues/63) is about the loader not finding the library. The fix it suggests is to ship the library next to the binary, with `-Wl,-rpath,$ORIGIN` on Linux.
- [github/dmca](https://github.com/github/dmca) has no Valve notice that mentions `steam_api` or Steamworks. Its Valve notices concern leaked or decompiled game and engine code. We searched with `gh search code`.

## 3. Alternatives, if we don't ship the library

**Load it from the user's Steam or game install.**
- Observed locally on Windows on 2026-10-20: the Steam client folder has `steamclient.dll` and `steamclient64.dll`, but no `steam_api*.dll`.
- Europa Universalis IV ships its own `steam_api64.dll`. Its export names include `SteamAPI_SteamUGC_v016`, `SteamAPI_SteamUser_v021` and `SteamAPI_Init`, with no `SteamAPI_InitFlat`.
- steamworks-rs master binds `SteamAPI_InitFlat`, `SteamAPI_SteamUGC_v021`, `SteamAPI_SteamUser_v023` and `SteamAPI_SteamUtils_v011` ([windows_bindings.rs](https://github.com/Noxime/steamworks-rs/blob/master/steamworks-sys/src/windows_bindings.rs)).
- So a binary built on it would fail to resolve imports against the game's copy. This comes from a `strings` scan of the DLL, not from a run.
- CI runners have no game install at all.
- Loading a local copy for personal use is not redistribution. But per the SSA, copying it into our archive would be.

**Download it at install time.**
- The only official source is behind a login (see §1). So an action or installer would have to fetch from a third-party package: the `steamworks-sys` crate, the npm package or the NuGet package.
- Those packages rely on their own maintainers' grant. We would add a network dependency without becoming the Licensee ourselves.
- The workable form is "user supplies the SDK", as steamworks-ffi-node does: a path input or an environment variable.

**`steamcmd`-only binaries.**
- SteamCMD is a public download: `https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip`, `steamcmd_linux.tar.gz` and `steamcmd_osx.tar.gz` return `200` without login.
- It publishes through `workshop_build_item`. Valve's [Workshop implementation guide](https://partner.steamgames.com/doc/features/workshop/implementation) says this "should only be used for testing purposes, as the tool requires the user to enter their Steam credentials".
- Because `steamworks-sys` declares `links = "steam_api"`, a lib-free binary must be built without the `steamworks` crate, for example behind a Cargo feature. Such a build loses `client` mode ([ADR 0002](../adr/0002-create-items-in-client-mode.md)).

## 4. Does this constrain the MIT licence?

**Documents:**
- MIT is on Valve's list of acceptable licences for Steamworks apps ([source](https://partner.steamgames.com/doc/sdk/uploading/distributing_opensource)).
- The SDK agreement grants no right to relicense the SDK Redistributables, and reserves everything else (§1.3).

**Practice:** steamworks-rs licenses its crate as MIT/Apache "except for the files in `steamworks-sys/lib/steam/`".

**Inference:**
- The repository stays MIT.
- Release archives are mixed: MIT for our binary, Valve's terms for `steam_api`.
- Say so in the README and in a `THIRD-PARTY-NOTICES` file inside each archive.
- Keep the Valve binaries out of the git tree and attach them only at release time.
- Do not use Valve marks in a way that suggests partnership (§2.3).

## Options ranked

1. **Ship the library in release archives, as ADR 0001 plans.**
   - This is the only path the SDK agreement grants expressly (§1.1(b)), and it is what every comparable Rust, JS, .NET and Godot project does.
   - Conditions:
     - a maintainer accepts the SDK Access Agreement and acts as Licensee;
     - the shipped files are byte-identical to that SDK version's `redistributable_bin` (check hashes against the `steamworks-sys` copy);
     - archives carry a third-party notice saying the files are Valve's and not MIT;
     - no binaries go into git;
     - if Valve objects, stop (§3.2).
   - Residual risk: Valve could decide that a third-party publishing tool is not "Licensee Software".
2. **Add a `steamcmd`-only build without `steam_api` as a fallback variant.** It is legally clean and fits CI. But Valve calls `steamcmd` Workshop uploads testing-only, and the variant loses `client` mode.
3. **Download at install time or let the user supply the library.** Keep this as an override (`STEAM_API_PATH` or an action input) for users who want their own SDK copy. As the default it either depends on third-party mirrors or requires every user to accept the SDK agreement.
4. **Load from the user's Steam or game install.** The Steam client has no `steam_api`. Game copies are older ABI versions that current steamworks-rs cannot use. CI has neither.
