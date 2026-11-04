# spike-ai-pipeline-onenote

The OneNote tool of [spike-ai-pipeline](https://github.com/JaimeStill/spike-ai-pipeline): a
command-line utility that reads OneNote `.one` sections, from desktop OneNote 2016 and later or
downloaded from OneDrive, `.onepkg` exports, and notebook directories through their `.onetoc2`,
through [onenote_parser](https://crates.io/crates/onenote_parser), into one JSON model: the
notebook, its section groups and sections, and each page at its latest revision, with its rich
text, lists, tables, math, and note tags, a Markdown body, and its images, embedded files, and
ink (recognized text and strokes as SVG) written out as files.

It is published as one static musl binary each for linux-x64 and linux-arm64, and
spike-ai-pipeline-processing runs it like any other system tool. It reads files only, never
Microsoft 365.

It is released by tag, `vX.Y.Z`, with a CHANGELOG. The spike's question, evidence, and answer are
in the root repository's README.

Apache-2.0.
