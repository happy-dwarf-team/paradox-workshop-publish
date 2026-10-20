# Description and change notes

## The description

The Workshop description comes from the file that `description` names in `workshop.toml`:

```toml
[[mod]]
game = "eu4"
description = "workshop/description.md"
```

- **`.md`** files are converted from Markdown to Steam BBCode.
- **`.bbcode`** files are sent as they are.

**Without `description`, publishing never touches the Workshop description**, so you can keep editing it on Steam. **With it, every publish replaces the description** with the file's content: the repository becomes the source, and edits made on Steam are lost at the next release. `pdx-ws plan` and the action's summary say so and show the BBCode that will be sent.

Steam takes at most 8000 bytes, which `pdx-ws` checks after the conversion. Accented letters and other non-ASCII characters take more than one byte each.

## Markdown that converts

| Markdown | BBCode |
| --- | --- |
| `# Title`, `## Part`, `### Section` | `[h1]`, `[h2]`, `[h3]` |
| `**bold**`, `*italic*`, `~~struck~~` | `[b]`, `[i]`, `[strike]` |
| `[text](https://…)`, `<https://…>` | `[url=…]text[/url]` |
| `- item` / `1. item` | `[list]` / `[olist]` with `[*]` |
| `> quote` | `[quote]` |
| fenced or indented code blocks | `[code]` |
| `---` | `[hr][/hr]` |
| tables | `[table]` with `[th]` and `[td]` |
| `![alt](https://…/image.png)` | `[img]` |

Some constructs are handled specially:

- **Inline `code`** becomes plain text, because Steam has no inline code style.
- **Line breaks inside a paragraph** become spaces, because Steam would show every new line. End a line with two spaces to force a break.
- **Text with `[`** goes in `[noparse]`, so Steam does not read it as a tag.
- **HTML comments** (`<!-- … -->`) are dropped.

Anything Steam cannot show is an error that names the file and line, so you find out before the release, not after:

- raw HTML;
- footnotes;
- `####` and deeper headings;
- images with a relative path. Steam cannot show files from your repository, so upload the image somewhere and link its `https://` URL.

## The change note

Each publish attaches a change note, shown on the item's *Change Notes* page. `pdx-ws` takes the first of:

1. **An explicit note:** `--change-note` on the CLI, or the action's `change-note` input. An empty value counts as not given, so an empty release body falls through to the next source.
2. **The changelog section for the descriptor's `version`**, when `changelog` is set. These headings are recognised, and a leading `v` is ignored:
   - release-please: `## [1.2.0](https://…) (2026-10-20)` or `## 1.2.0 (2026-10-20)`;
   - Keep a Changelog: `## [1.2.0] - 2026-10-20`.

   The section runs to the next heading of the same level. If the changelog has no section for that version, the plan fails (exit 2) instead of publishing a wrong note. The same happens when the section is empty, or when the descriptor has no `version`.
3. **`Version <version>`.** In GitHub Actions a link to the commit is added.

All three go through the same Markdown converter, with the same errors.

### When Steam keeps the old version

Steam records a change note only with a new version of the content. If the published copy is byte for byte what the item already has (for example a release that changed only files `pdx-ws` leaves out, or a first publish of what the launcher uploaded before), Steam keeps the current version: the item's update date stays and the change note is dropped. steamcmd still prints `Success`.

`pdx-ws` reads this from steamcmd's logs (`No content change detected`) and reports the mod as **`unchanged`** instead of `published`, with a warning; the action adds a warning annotation. The run still succeeds, because nothing went wrong. To publish a new version, change a file of the mod.

### Release body or changelog?

- **Release body:** if you write release notes on GitHub, pass them with `change-note: ${{ github.event.release.body }}`. GitHub's generated release notes start with an HTML comment, which is dropped.
- **Changelog:** if release-please or a hand-kept `CHANGELOG.md` already holds the notes, leave `change-note` empty and set `changelog`.
