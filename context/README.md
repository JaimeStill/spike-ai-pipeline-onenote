# spike-ai-pipeline-onenote

The OneNote CLI of spike-ai-pipeline, released by tag and built for spike-ai-pipeline-processing's
`onenote` package to run as a system tool; that package is planned (the roadmap's `onenote`
task). This context records only working knowledge the code and the README do not express; the
spike's question and evidence live in the root repository's `context/README.md`.

## Capability map

- **sections**: `.one` sections, desktop OneNote 2016+ and OneDrive downloads, through
  onenote_parser.
- **notebooks**: `.onepkg` exports and notebook directories through their `.onetoc2`, with
  section groups; the recycle bin skipped with a warning.
- **pages**: each page at its latest revision: rich text with hyperlinks, lists, tables, math,
  note tags, and a Markdown body.
- **media**: images and embedded files written out as files named by SHA-256.
- **ink**: recognised text as stored, and strokes written as SVG for vision.
- **fixtures**: Interop-TestSuites' MIT OneNote files and onenote.rs's MPL-2.0 samples, pinned
  and listed by SHA-256, with hand-written expectations in `tests/fixtures/`; `.onepkg` packages
  built by the tests through cab. An authored notebook with hand truth is planned (the roadmap's
  `onenote-fixture` task).
- **finding**: fields recovered per kind against the fixtures' expectations, and time and peak
  memory on every committed input and three large sections (`docs/findings/onenote-cli.md`).
- **release**: static musl binaries for linux-x64 and linux-arm64, with the check run in CI on
  both.
