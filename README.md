# spike-ai-pipeline-onenote

The OneNote tool of [spike-ai-pipeline](https://github.com/JaimeStill/spike-ai-pipeline): a
command-line utility that reads OneNote `.one` sections, from desktop OneNote 2016 and later or
downloaded from OneDrive, `.onetoc2` notebooks, notebook directories, and `.onepkg` packages,
through [onenote_parser](https://crates.io/crates/onenote_parser) 2.1, used unmodified, into one
JSON model: the notebook, its section groups and sections, and each page at its latest revision,
with its rich text, lists, tables, math, and note tags, a Markdown body, and its images, embedded
files, and ink (recognised text, and strokes as SVG), written out as files on request.

It is published as one static musl binary each for linux-x64 and linux-arm64, which
spike-ai-pipeline-processing runs like any other system tool. It reads files only, never
Microsoft 365 or the network, and calls no model.

It is released by tag, `vX.Y.Z`, with a [CHANGELOG](CHANGELOG.md). Its finding for the spike's
evidence 17, the fields it recovers per kind and its time and peak memory per input, is
[`docs/findings/onenote-cli.md`](docs/findings/onenote-cli.md), summarised in "Finding" below.
The spike's question, evidence, and answer are in the root repository's README.

## Usage

```sh
spike-ai-pipeline-onenote [--media <dir>] <input>
spike-ai-pipeline-onenote --version
```

It reads one input and writes JSON Lines of schema `"1"` to stdout, and nothing else there. The
arguments may come in any order, and a path needn't be UTF-8. `--version`, alone, prints the bare
version, `0.1.0`.

The input's kind is sniffed from its content, never from its name:

- a cabinet's signature, `MSCF`, is a `.onepkg` package;
- a OneStore header, of the desktop revision store format or of OneDrive's packaging format, is
  a section or a notebook, as onenote_parser's store type says: a `.one` section or a `.onetoc2`
  table of contents;
- a directory is a notebook, read through the one `.onetoc2` it holds.

With `--media <dir>`, each image, embedded file, and ink drawing is written into `<dir>`,
created if missing, each distinct content once, before the line that names it. Without it nothing
is written to disk, and the model names the files all the same.

| Exit code | Meaning |
| --- | --- |
| 0 | Read, even when sections were skipped. |
| 1 | stdout, the media directory, or a file in it couldn't be written. |
| 2 | A usage error: no input, more than one, an unknown option, `--media` without a directory, given twice, or naming something that isn't a directory, or `--version` with other arguments. Usage goes to stderr. |
| 3 | Refused, before any output: the input is missing or unreadable; a ZIP (`a ZIP archive; extract the OneDrive download first`); a directory with no `.onetoc2`, or with more than one; anything else (`not a OneNote section, notebook, or package`); or a section, notebook, or package onenote_parser can't read, with its reason, or that panics it (`the parser panicked: <message>`). |
| 4 | A fault of the CLI's own, never reported as any other code. |

An error, for exit codes 1 to 4, is one line on stderr,
`spike-ai-pipeline-onenote: error: <reason>`, a refusal's reason led by the input's path; a
usage error's is followed by the usage. A password-protected section is refused with
onenote_parser's own reason, `Unexpected end of file`: the parser can't decrypt it.

Every warning is one line on stderr:

```text
spike-ai-pipeline-onenote: warning: <where>: <what>
```

`<where>` names the record: a section by its path from the notebook's root (`/Group/Section`),
followed by `, page <id>` and then `, image <n>`, `, embedded file <n>`, or `, equation <n>`, each
numbered from 1 within its page; `/OneNote_RecycleBin` for the recycle bin; and `/` for a
notebook-wide warning of the parser's. Control characters in a name are escaped, so a warning
stays one line. Every warning in onenote_parser's section and notebook reports is written this
way.

A section that fails to parse inside a notebook is skipped: a warning,
`<where>: the section is skipped: <reason>`, a `skipped` line, and the read goes on and exits 0. A
password-protected section inside a notebook is skipped this way. A notebook's recycle bin, a
`OneNote_RecycleBin` entry beside its `.onetoc2`, in the directory or in the package's cabinet, is
skipped with a warning, and its sections and pages are never written.

onenote_parser prints debug dumps to stdout as it fails on some malformed sections, so the CLI
writes its lines to a duplicate of stdout and points file descriptor 1 at `/dev/null` while it
reads: stdout carries the lines and nothing else.

## Schema "1"

Every line is one JSON object, its `line` field naming its kind. A field with no value is left
out, never written as `null`, and so is an empty list; the one empty object is an empty table
cell, `{}`. Times are ISO 8601 UTC, to the second, a fraction cut, not rounded. The same input
always gives byte-identical output.

The `notebook` line comes first. Then the notebook's entries depth-first, in its table of
contents' order: a `section_group` line before the entries it holds, and a `section` line
followed by its `page` lines, in the order of its page series. Then a `skipped` line for each
section the read left out, in the order the parser reported them. A section input gives the
notebook line, one section line, and its pages. A package reads like its notebook directory, in
every line but the notebook line's `input`.

### notebook

| Field | Value |
| --- | --- |
| `line` | `"notebook"` |
| `schema` | `"1"` |
| `input` | `"section"`, `"notebook"` (a `.onetoc2` or a directory), or `"package"`. |
| `name` | A section's file stem; a notebook's directory name, for a `.onetoc2` input too; a package's file stem. |
| `id` | A notebook's or package's: its `.onetoc2`'s FileIdentity, a lowercase hyphenated GUID. |
| `color` | A notebook's or package's colour, `"#rrggbb"`. |

### section_group

| Field | Value |
| --- | --- |
| `line` | `"section_group"` |
| `id` | The FileIdentity of the group's own `.onetoc2`. |
| `parent` | The id of the group it is in; a group at the notebook's root has none. |
| `path` | The names from the notebook's root down to this group, a list. |
| `display_name` | The group's directory name, which OneNote shows as its name. |

### section

| Field | Value |
| --- | --- |
| `line` | `"section"` |
| `id` | The section's FileIdentity. |
| `parent` | The id of the section group it is in; a section at the notebook's root, or a section input, has none. |
| `path` | The names from the notebook's root down to this section, a list. |
| `display_name` | The section's display name. |
| `color` | Its colour, `"#rrggbb"`; none when it has none. |

### page

| Field | Value |
| --- | --- |
| `line` | `"page"` |
| `id` | The page's GUID, lowercase and hyphenated. |
| `section` | The id of its section. |
| `level` | Its depth in its section's page tree: 1 for a page, 2 for its subpage, and so on. |
| `title` | Its title text. |
| `creation_time`, `modification_time` | Times. |
| `author` | The author, as stored. |
| `content` | Its containers, below: the title area first, then the rest in stored order. |
| `ink` | The page's own ink, every ink container's strokes in one drawing: an ink record. |
| `ink_text` | The page's recognised ink text: a list of lines, each a list of words. |
| `markdown` | The page as Markdown, rendered from the fields above; none when nothing on it renders. |

A page is read at its latest revision only. A recognised word holds its `text`, the recogniser's
best guess; its `alternatives`, its other guesses, best first; and its `language`, a Windows LCID
as stored (1031 is German, Germany).

An ink record holds its `file`, the SVG's name; its `strokes`, a count; and its `bounds`, the box
its strokes' points span before their pens' width is added, `{x, y, width, height}` in ink units
of 0.01 mm. A page's ink is in the page's coordinates; ink in a paragraph or an outline is in its
own.

### Containers

Each container's `kind` names it. Its `offset_horizontal` and `offset_vertical` are from the
page's origin, in half-inch units, as stored; a container without one has none.

| `kind` | Fields |
| --- | --- |
| `title` | The page's title area: its offsets and `items`, its outlines' items in order. |
| `outline` | A text container: its offsets and `items`. |
| `image` | Its offsets and an image's fields, below. |
| `embedded_file` | Its offsets and an embedded file's fields, below. |
| `ink` | One stored ink container: its offsets; `strokes`, its stroke count; and `bounds`, the box its strokes span on the page, in ink units, which strokes without points don't have. Its strokes are drawn in the page's `ink`. |

An outline's `items` nest as stored. An item's `kind` is `element`, an outline element, with its
`blocks` and then `children`, the items indented below it; or `group`, items OneNote keeps
together at their own indentation, with its `items`.

A block's `kind` is `paragraph`, `table`, `image`, `embedded_file`, or `ink`. An `image` or
`embedded_file` block holds that record's fields, below; an `ink` block holds an ink record's
(`file`, `strokes`, `bounds`), drawn alone, and ink with no strokes has none of them.

### Paragraphs and runs

| Field | Value |
| --- | --- |
| `style` | The paragraph style's id as stored: `"p"`, `"h1"` to `"h6"`, `"PageTitle"`, `"PageDateTime"`, `"Quote"`, `"code"`, and so on. |
| `list` | The element's list marker, on its first paragraph: a `bullet`, the bullet character, or a `number`, its format (`"decimal"`, `"lower-letter"`, `"upper-letter"`, `"lower-roman"`, `"upper-roman"`, `"ordinal"`, `"cardinal-text"`, `"ordinal-text"`, `"hex"`, `"decimal-zero"`, else `"nfc-<code>"`), with the `prefix` and `suffix` around it; and `restart`, the number the list restarts at. |
| `tags` | Its note tags, below. |
| `runs` | Its visible runs, in order; hidden runs are left out. |
| `ink` | Its handwriting, its inks laid out as they read in one drawing: an ink record. The parser gives such a paragraph no text, so it has no runs. |

A run is text of one formatting. A flag that isn't set, and a colour that is automatic, are left
out; neighbouring runs of the same formatting and link are one.

| Field | Value |
| --- | --- |
| `text` | Its text. |
| `math` | An equation's: `"display"` or `"inline"`, below. |
| `latex` | An equation's LaTeX, below. |
| `bold`, `italic`, `underline`, `strikethrough`, `superscript`, `subscript` | `true` when set. |
| `highlight`, `font_color` | `"#rrggbb"`. |
| `link` | The link's target. A link OneNote recognised in typed text targets its own text. |

An equation, the text of a paragraph's consecutive math-formatted runs between OneNote's math
markers, is one run of its own, with no text, flags, colours, or link: its `math`, `"display"`
when it is its paragraph's only content, else `"inline"`; and its `latex`, bare, without `$` or
`\(` around it, from a port of [one2html](https://github.com/msiemens/one2html)'s MIT math lexer,
parser, and LaTeX renderer. The LaTeX keeps Unicode symbols (α, ≤, ∂), for a Unicode-aware
backend such as MathJax, KaTeX, or unicode-math. An equation whose markers don't parse keeps its
math run, its LaTeX that of its text with the markers stripped, with a warning, and the read exits
0.

A note tag holds its `label`; its `shape`, the tag's icon by onenote_parser's NoteTagShape name
(`"BlueCheckBox"`, `"YellowStar"`); its `status`, `"completed"` or `"open"`, always written; and,
for a completed tag, its `completed_time`.

### Tables

A table holds its `tags` and its `rows`, a list of rows, each a list of cells. A cell holds its
`items`, as an outline's, so lists and tables inside a cell survive; an empty cell is `{}`, since
it holds its place in its row. The parser doesn't expose merged cells: each cell is its own.

### Images and embedded files

| Field | Value |
| --- | --- |
| `file` | The name of its content's file in the media directory; none when its data is missing or invalid, with a warning, its other fields kept. |
| `size` | The content's size in bytes, when its data is there. |
| `extension` | An image's: its own stored extension, without the dot. |
| `filename` | The name it was inserted or attached from, as stored. |
| `alt_text` | An image's alt text. |
| `ocr_text` | An image's text, as OneNote recognised it in the picture; in the model only, never in the Markdown. |
| `hyperlink` | An image's hyperlink. |
| `printout_page` | For an image of a printout, the printed page it shows. |
| `type` | An embedded file's: `"audio"` or `"video"` for a recording; other files have none. |

### skipped

| Field | Value |
| --- | --- |
| `line` | `"skipped"` |
| `kind` | `"section"` |
| `name` | Its name as the notebook's table of contents gives it, a section's file name. |
| `reason` | Why, in the parser's words. |

## Media

A file's name is the lowercase hex SHA-256 of its bytes, plus `.` and the stored extension when
that is 1 to 8 ASCII letters and digits, in its stored case: an image's own extension, else its
filename's; an embedded file's filename's; and `svg` for ink. A content with no usable extension
is named by its hash alone. The name is a function of the content, so the same bytes share one
file, written once. A stored filename is never used as a path.

## Ink

Ink is written as SVG by the CLI's own code: one SVG per page, of all its page-level ink at its
offsets, and one per paragraph that holds handwriting. Each stroke is one path with its colour (a
COLORREF, black when none is stored), its width, its transparency (0 opaque, as ISF stores it),
and its pen tip. The SVG's numbers are rounded to two decimals in a fixed form, so the same ink
gives the same bytes, and so the same name. one2html's ink renderer was read as a reference for
the colour, transparency, and pen-tip mappings; no code is taken from it. The recognised text is
the page's `ink_text`, as OneNote stored it.

## Markdown

A page's `markdown` is rendered from its model alone:

- The title area first, then the containers top to bottom, then left to right. An ink container
  is placed by its strokes' bounds, since its stored offsets don't place them, and the page's ink
  goes once, where its first container with strokes is.
- The page title as `#`; an `hN` paragraph one level down, capped at 6. The title area's date and
  time paragraphs are left out, since `creation_time` holds the time, and so are empty
  paragraphs, OneNote's blank lines.
- Bold `**`, italic `*`, strikethrough `~~`, and links `[text](url)`. Underline, highlight,
  colour, superscript, and subscript are plain text. A line break in a paragraph is a hard break.
- Code paragraphs fenced, the code paragraphs beside them in one fence, their text as stored; a
  `Quote` or `Citation` paragraph as `>`, the ones beside it in one quote.
- Lists nested, numbered from their restart, else from 1. A restart that breaks the count, or a
  container's start, parts two lists with an HTML comment, `<!-- -->`.
- A To Do tag, any tag whose shape is a check box, as a `[ ]` or `[x]` task item; any other tag
  as a `[Label]` prefix.
- Tables as GFM, the first row the header, short rows padded, and a cell's blocks joined by
  `<br>`; a table's tags on a line before it.
- Math as `$…$` inline and `$$…$$` for display.
- Images as `![alt](file)`, embedded files as `[filename](file)`, and ink as
  `![ink: <recognised text>](file)`, or `![ink](file)` with none; without a file, `[image: …]` and
  `[file: …]`. An image's recognised text is never in the Markdown.
- Literal text escaped as spike-ai-pipeline-mail's Markdown escapes it, by htmd's rules: a
  backslash before each of ``\ * _ ` [ ] | ~ &``, before `<` where it could open an HTML tag, and
  at a line's start before a `-`, `=`, `+`, `#`, or `>`, or the `.` or `)` after one to nine
  digits; and before `$`, since math is written between `$`s.

## Fixtures

The tests run the built binary over two public fixture sets, each with its sources and licence
beside it:

- **Interop-TestSuites** (`tests/fixtures/interop/`): Microsoft's
  [Interop-TestSuites](https://github.com/OfficeDev/Interop-TestSuites) MS-ONESTORE files at
  commit `fe87ed32`, under the MIT License: its small sections and notebooks, among them
  `Encryption.one`, a password-protected section, refused as an input and skipped inside its
  notebook.
- **onenote.rs** (`tests/fixtures/onenote-rs/`): onenote_parser's maintainer samples at `v2.1.0`
  (`727389a`), under the MPL-2.0: sections, a section group, a notebook with a recycle bin,
  OneDrive-format sections, and `handwriting_recognition.one`, the ink and recognition sample.
  Joplin's AGPL samples, which the same repository holds, are never taken, nor is a sample a
  third party attached to an issue with no licence grant.

Each set's `sources.json` lists every file with its source path, its pinned commit, its licence,
and its SHA-256, and the tests check them. Every file reads as `tests/fixtures/sections.json` and
`notebooks.json` expect: exit code, line kinds, sections, pages, titles, content kinds and their
counts, skipped sections, and recognised words. `tests/fixtures/content.json` gives content
fields, each a JSON pointer into a page line and its value: styles, runs, links, note tags, list
markers, tables, math, images, embedded files, ink, and recognised words. `.onepkg` packages are
built by the tests in memory, through cab 0.6.0's MSZIP writer, from the committed notebooks, and
each must read as its directory does.

## Finding

[`docs/findings/onenote-cli.md`](docs/findings/onenote-cli.md) records, from run `v0.1.0` of the
static musl release binary, the public fixtures scored against their hand-written expectations,
and the time and peak resident memory of every input, median of 3:

- Every expected field recovered: 236 of 236 over the 14 committed sections
  (`sections.json`), 138 of 138 over the 9 committed notebooks (`notebooks.json`), and 129 of
  129 content fields (`content.json`), with every expected absence absent. There is no authored
  hand truth.
- Every committed input reads in under 10 ms and 4.5 MiB. Interop-TestSuites' `LargeData.one`
  (11 MB) reads in 1.30 s at 19.5 MiB; onenote.rs's `Large Desktop.one` (113 MB) and
  `Large OneDrive.one` (161 MB) in 0.25 s at 135 MiB each.
- onenote_parser issue #48 doesn't show at 161 MB: the OneDrive-format section peaks where its
  desktop-format twin does. It stays a limit for large, long-edited OneDrive sections.
- An image with no stored extension is named for its filename's, as the contract says, so
  `Large Desktop.one`'s printout images, PNG bytes with filenames ending `.pdf`, are written as
  `.pdf`; `Large OneDrive.one` stores `png` for the same images.

## Known limits

- A password-protected section is refused as an input, and skipped inside a notebook, with the
  parser's reason, `Unexpected end of file`: [MS-ONESTORE] 2.5.19 leaves the encryption scheme
  undocumented, and no open decryptor exists.
- No OneNote-written `.onepkg` is tested: packages need desktop OneNote on Windows, so the tests
  build their own.
- No authored hand truth: every expectation comes from the public Interop-TestSuites (MIT) and
  onenote.rs (MPL-2.0) fixtures. A later release adds an authored notebook.
- No desktop-format hand truth: the desktop format rests on the public files' structural checks
  alone.
- Merged table cells aren't exposed by onenote_parser, so each cell is read as its own.
- Page history isn't read: a page is read at its latest revision only.
- onenote_parser's open issues:
  [#39](https://github.com/msiemens/onenote.rs/issues/39), a section fails to parse after a text
  container is copied in from another section;
  [#48](https://github.com/msiemens/onenote.rs/issues/48), opening a OneDrive-format section
  parses every cell of its storage index, so a 375 MB section needs over 9 GB;
  [#50](https://github.com/msiemens/onenote.rs/issues/50), a missing page space fails the whole
  section.
- Skipped lines come after the notebook's entries, in the order the parser reported them, not in
  their place in the table of contents: the parser drops a section that fails and keeps only its
  message.
- A section that fails inside a section group is lost silently: the parser discards the nested
  group's report, so it gets no skipped line and no warning.
- A table-of-contents entry whose file is missing gets no warning: the table's entries aren't
  public, and a check of the directory alone would break the equality of a package's read and its
  directory's.
- onenote_parser's stdout is redirected to `/dev/null`, so anything it prints there is lost; the
  warnings of its reports, which the CLI writes to stderr, are not.

## Development

`mise run check` lints, format-checks, checks that `THIRD-PARTY-NOTICES.md` names exactly the
crates the release binary links at their locked versions (`scripts/notices.sh`), tests, and
builds the static musl release binary; CI runs it on linux-x64 and linux-arm64, on every pull
request into main and every push to main. `scripts/release-pack.sh <target> <tag>` builds and
packs one release archive, refusing a tag that isn't the binary's version; the release workflow
runs it on each architecture for a pushed `vX.Y.Z` tag. `mise run currency` reports every crate,
mise tool, and Action pin behind its latest. `mise run findings` runs the finding
([`docs/findings/onenote-cli.md`](docs/findings/onenote-cli.md), "Rerunning").

## Licence

Apache-2.0. The release binary also holds onenote_parser 2.1.0, under the MPL-2.0, its source at
<https://github.com/msiemens/onenote.rs> (tag `v2.1.0`); code ported from one2html 2.0.1, under
the MIT License; and other crates under MIT, zlib, BSD-3-Clause, and Unicode terms.
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md), which every release archive carries, gives
each one's notice.
