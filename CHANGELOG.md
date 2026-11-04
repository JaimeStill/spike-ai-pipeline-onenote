# Changelog

All notable changes to `spike-ai-pipeline-onenote` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the CLI adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). The JSON Lines of schema `"1"`, the
`schema` version included, the arguments, the exit codes, the warning format, the media file
names, and the Markdown rules are the CLI's output contract: a change to any of them is a change
here.

## [Unreleased]

## [v0.1.0] - 2026-11-04

### Added

- `spike-ai-pipeline-onenote [--media <dir>] <input>`, which reads a OneNote `.one` section, in
  desktop or OneDrive format, a `.onetoc2` notebook, a notebook directory through its one
  `.onetoc2`, or a `.onepkg` package, through onenote_parser 2.1.0, used unmodified, sniffed by
  content, never by name, and writes JSON Lines of schema `"1"` to stdout, which carries them and
  nothing else. Exit codes: 0 read, even with sections skipped; 1 stdout or a media file not
  written; 2 usage (usage on stderr); 3 refused (one reason line on stderr, nothing on stdout);
  and 4 a fault of the CLI's own. `--version` prints the bare version.
- The initial output contract, schema `"1"`: a `notebook` line first (input kind `"section"`,
  `"notebook"`, or `"package"`, name, and a notebook's id and colour); then `section_group` and
  `section` lines depth-first in the table of contents' order (id, parent group, path, display
  name, and a section's colour), each section followed by its `page` lines; then a `skipped` line
  for each section the parser couldn't read. A field with no value is left out, never written as
  `null`, and the same input always gives byte-identical output.
- Pages, at their latest revision: id, section, level, title, creation and modification times in
  ISO 8601 UTC to the second, author as stored, content, recognised ink text, and a Markdown
  body.
- Content: the title area, outlines, images, embedded files, and ink containers with their
  offsets; outline items nested as stored; paragraphs with their style, list marker (bullet, or
  numbering format with its prefix, suffix, and restart), note tags (label, shape, status,
  completed time), runs, and handwriting; runs with bold, italic, underline, strikethrough,
  superscript, and subscript flags, highlight and font colour as hex, and link target, hidden
  runs left out; tables of rows of cells, a cell holding outline items, an empty cell `{}`.
- Math: an equation is one run of kind math, `"display"` or `"inline"`, with its LaTeX, from a
  port of one2html 2.0.1's MIT math lexer, parser, and LaTeX renderer that keeps its notice. An
  equation that doesn't parse keeps the LaTeX of its text, with a warning.
- Media: images (file, size, extension, filename, alt text, OneNote's text from the picture,
  hyperlink, printout page) and embedded files (file, filename, `"audio"` or `"video"` type,
  size). `--media <dir>` writes each distinct content once, named by the lowercase hex SHA-256 of
  its bytes and its stored extension when that is 1 to 8 ASCII letters and digits; a record whose
  data is missing or invalid has no file, with a warning. Stored filenames are never paths.
- Ink as SVG, written by the CLI: one per page and one per paragraph that holds handwriting, each
  stroke a path with its colour, width, transparency, and pen tip; an ink record names its file,
  stroke count, and bounds in ink units. A page's recognised text is its lines of words, each with
  its best guess, alternatives, and language.
- Markdown: containers top to bottom, then left to right; the title as `#` and `hN` one level
  down; bold, italic, strikethrough, and links marked; code fenced and quotes quoted; lists nested
  and numbered from their restart; To Do tags as task items and other tags as `[Label]`; GFM
  tables; math as `$…$` and `$$…$$`; images, files, and ink linked by their media names; literal
  text escaped by spike-ai-pipeline-mail's rules, and `$`.
- Skips and warnings: a section that fails inside a notebook is a `skipped` line and a warning,
  and the read exits 0; the recycle bin, in a directory or a package, is skipped with a warning;
  every warning of onenote_parser's reports goes to stderr as
  `spike-ai-pipeline-onenote: warning: <where>: <what>`.
- Refusals: a missing or unreadable input, a ZIP (extract the OneDrive download first), a
  directory with no `.onetoc2` or more than one, anything else, and a section, notebook, or
  package onenote_parser can't read or that panics it, with its reason.
- Fixtures: Interop-TestSuites' small MS-ONESTORE files (MIT, at `fe87ed32`) and onenote.rs's
  maintainer samples (MPL-2.0, at `v2.1.0`), each set pinned and listed by SHA-256 with its
  LICENSE, read against hand-written structural expectations; `.onepkg` packages built by the
  tests through cab 0.6.0's MSZIP writer, each read as its directory.
- Releases: `spike-ai-pipeline-onenote-vX.Y.Z-linux-x64.tar.gz` and `-linux-arm64.tar.gz`, each a
  static musl binary, LICENSE, and `THIRD-PARTY-NOTICES.md`, and `SHA256SUMS`. The notices give
  onenote_parser's MPL-2.0 licence and where its source is, one2html's MIT notice for the ported
  math code, and the notices of every other crate the binary links.

### Known limits

- Password-protected sections are refused, and skipped inside a notebook, with the parser's
  `Unexpected end of file`: the encryption scheme is undocumented.
- No OneNote-written `.onepkg` is tested; the tests build their own.
- No authored hand truth: every expectation comes from the public Interop-TestSuites (MIT) and
  onenote.rs (MPL-2.0) fixtures. A later release adds an authored notebook.
- No desktop-format hand truth.
- Merged table cells aren't exposed by onenote_parser.
- Page history isn't read.
- onenote_parser issues #39 (a copied text container fails a section), #48 (a 375 MB
  OneDrive-format section needs over 9 GB), and #50 (a missing page space fails a section).
- Skipped lines come after the notebook's entries, not in their place.
- A section that fails inside a section group is lost silently.
- A table-of-contents entry whose file is missing gets no warning.
- onenote_parser's stdout is redirected to `/dev/null`, and what it prints there is lost.

[Unreleased]: https://github.com/JaimeStill/spike-ai-pipeline-onenote/compare/v0.1.0...HEAD
[v0.1.0]: https://github.com/JaimeStill/spike-ai-pipeline-onenote/releases/tag/v0.1.0
