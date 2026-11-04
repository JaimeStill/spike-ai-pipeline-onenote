# spike-ai-pipeline-onenote

The OneNote CLI of spike-ai-pipeline, released by tag and run by spike-ai-pipeline-processing as a
system tool. This context records only working knowledge the code and the README do not express;
the spike's question and evidence live in the root repository's `context/README.md`.

## Capability map

- **sections**: `.one` sections, desktop OneNote 2016+ and OneDrive downloads, through
  onenote_parser.
- **notebooks**: `.onepkg` exports and notebook directories through their `.onetoc2`, with
  section groups; the recycle bin skipped with a warning.
- **pages**: each page at its latest revision: rich text with hyperlinks, lists, tables, math,
  note tags, and a Markdown body.
- **media**: images and embedded files written out as files named by SHA-256.
- **ink**: recognized text as stored, and strokes written as SVG for vision.
- **release**: static musl binaries for linux-x64 and linux-arm64.
