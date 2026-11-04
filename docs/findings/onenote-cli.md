# Finding: the OneNote CLI's fields against the public fixtures, and its time per input

This page records the CLI's part of the spike's evidence 17 (the root's `context/README.md`,
item 17): the fields it recovers, per field kind, against the public fixtures' hand-written
expectations, and its time and peak memory on every committed input and on three large sections
fetched for the run, Interop-TestSuites' `LargeData.one` (11 MB) and onenote.rs's
`Large Desktop.one` (113 MB) and `Large OneDrive.one` (161 MB). It measures the CLI alone, with
no baseline against other readers. There is no authored notebook, and so no authored hand truth:
OneNote for the web wouldn't create one, and v0.1.0 ships without it (the goal record's
escalation of 2026-11-04); a later release adds it. Processing's onenote adapter, the rest of
evidence 17, belongs to processing's onenote task.

Every number here comes from run
[`onenote-cli/runs/v0.1.0/results.json`](onenote-cli/runs/v0.1.0/results.json), on the laptop on
2026-11-04 from 18:54:19 to 18:54:30 UTC, CLI 0.1.0 at commit c9d83cc, the static musl release
binary the check builds for x86_64 (3,516,048 bytes, SHA-256 `0ea84ce5...73dae7`); that file has
every run and would list every miss. A run is kept only when it is recorded ("Rerunning").

## At a glance

| Question | Answer |
|---|---|
| Does the CLI read what the expectations say? | **Yes, every field.** The 14 committed sections against `sections.json`: 236 of 236 fields, and 34 of 34 fields expected absent were. The 9 committed notebooks against `notebooks.json`: 138 of 138, and 26 of 26 absences. The content fields of 6 sections against `content.json`: 129 of 129, and 6 of 6 absences. No mismatch, nothing missing. |
| What does that cover? | Structure (exit codes, refusals, line kinds and order, notebook, section and page fields, skipped sections, warnings, containers by kind) and content (container offsets, paragraph styles, runs and their formatting, colours and links, note tags, list markers, tables, math, images, embedded files, ink records, and recognised words). Not scored: OCR text, the SVG strokes, and the Markdown, which the tests check on their own. |
| How fast? | Every committed input in a median under 10 ms. `LargeData.one`, 11 MB: **1.30 s**. `Large Desktop.one`, 113 MB: **0.25 s**. `Large OneDrive.one`, 161 MB: **0.24 s**. Median of 3 runs each. |
| How much memory? | Every committed input at most 4.4 MiB. `LargeData.one` **19.5 MiB**; `Large Desktop.one` and `Large OneDrive.one` **135 MiB** each, with or without `--media`. |
| Does issue #48 show? | **No.** `Large OneDrive.one`, an FSSHTTPB (OneDrive-format) section of 161 MB, peaks at 135 MiB, the same as its desktop-format twin; nothing near #48's 9 GB for 375 MB ("onenote_parser issue #48"). |
| Anything else? | `LargeData.one` is the slowest per byte by far: its page holds 135 images of one 8.1 MB content, and the CLI reads and hashes each, about 1.09 GB, in 1.3 s at 19.5 MiB. A printout page's image in `Large Desktop.one`, which stores no extension, is named for its filename's, so a PNG is written as `.pdf` ("The large sections"). Every input's stdout was byte-identical across its runs and with and without `--media`. |

## The machine

The laptop: Intel Core Ultra 9 185H, 22 logical CPUs, 32 GB of memory (32,284,144 kB), Linux
7.2.15-1-omarchy (Omarchy, on Arch), the inputs on a btrfs file system on a device-mapper volume,
on an NVMe disk.

Every input was read from the page cache: the large sections had been fetched and hashed by the
run's own script before it, and each input's runs followed one another, so no run read the disk
cold (that needs root to drop the cache). `--media` writes went to the page cache too, unsynced.
No memory bound was set: no input came near the machine's memory.

## Fields recovered per kind

Each row counts the fields of one kind an expectation gives, summed over the files of its table:

- **Recovered**: the CLI wrote the expected value.
- **Mismatched**: it wrote another value. **Missing**: it wrote none.
- **Absent, as expected**: a field the expectation leaves out, or gives as null, which the CLI
  must not write; "13 of 13" means it wrote none of the 13.

The expectations are written by hand from what onenote_parser's API gives for each record, not
from the CLI's output, and the check asserts the same files (`tests/sections.rs`,
`tests/notebooks.rs`, `tests/content.rs`). This run scores the release binary itself against
them, field by field.

### The committed sections, against sections.json

`tests/fixtures/sections.json` gives each committed `.one`, six of Interop-TestSuites' and eight
of onenote.rs's, its exit code, and either its refusal's reason or its section and pages. A page
is matched by its place in the section's order.

| Field kind | Expected | Recovered | Mismatched | Missing | Absent, as expected |
|---|---|---|---|---|---|
| Exit code | 14 | 14 | 0 | 0 | |
| Refusal reason (`Encryption.one`) | 1 | 1 | 0 | 0 | |
| Line kinds, in order | 14 | 14 | 0 | 0 | |
| Warning | 0 | 0 | 0 | 0 | 13 of 13 |
| Notebook line | 13 | 13 | 0 | 0 | |
| Section id | 13 | 13 | 0 | 0 | |
| Section display name | 13 | 13 | 0 | 0 | |
| Section path | 13 | 13 | 0 | 0 | |
| Section colour | 13 | 13 | 0 | 0 | |
| Section parent | 0 | 0 | 0 | 0 | 13 of 13 |
| Page id | 17 | 17 | 0 | 0 | |
| Page section | 17 | 17 | 0 | 0 | |
| Page level | 17 | 17 | 0 | 0 | |
| Page title | 11 | 11 | 0 | 0 | 6 of 6 |
| Page created | 17 | 17 | 0 | 0 | |
| Page modified | 17 | 17 | 0 | 0 | |
| Page author | 15 | 15 | 0 | 0 | 2 of 2 |
| Containers: title | 17 | 17 | 0 | 0 | |
| Containers: outline | 10 | 10 | 0 | 0 | |
| Containers: image | 1 | 1 | 0 | 0 | |
| Containers: ink | 3 | 3 | 0 | 0 | |
| **All** | **236** | **236** | **0** | **0** | **34 of 34** |

A "Containers" row counts pages, each scored on its count of containers of that kind.
`Encryption.one`, password-protected, is refused with exit 3 and the parser's own reason,
`Unexpected end of file`, as the README's known limits say.

### The committed notebooks, against notebooks.json

`tests/fixtures/notebooks.json` gives each committed `.onetoc2`, four of Interop-TestSuites' and
five of onenote.rs's, its exit code, its notebook line, its entries in order with each section's
pages by id and title, its skipped sections, and its warnings. A section is found by its id; its
pages are the page lines after it.

| Field kind | Expected | Recovered | Mismatched | Missing | Absent, as expected |
|---|---|---|---|---|---|
| Exit code | 9 | 9 | 0 | 0 | |
| Warning | 2 | 2 | 0 | 0 | 9 of 9 |
| Notebook schema | 9 | 9 | 0 | 0 | |
| Notebook input kind | 9 | 9 | 0 | 0 | |
| Notebook name | 9 | 9 | 0 | 0 | |
| Notebook id | 9 | 9 | 0 | 0 | |
| Notebook colour | 7 | 7 | 0 | 0 | 2 of 2 |
| Line kinds, in order | 9 | 9 | 0 | 0 | |
| Section id | 10 | 10 | 0 | 0 | |
| Section path | 10 | 10 | 0 | 0 | |
| Section display name | 10 | 10 | 0 | 0 | |
| Section colour | 10 | 10 | 0 | 0 | |
| Section parent | 0 | 0 | 0 | 0 | 10 of 10 |
| Page id | 13 | 13 | 0 | 0 | |
| Page section | 13 | 13 | 0 | 0 | |
| Page title | 8 | 8 | 0 | 0 | 5 of 5 |
| Skipped section | 1 | 1 | 0 | 0 | |
| **All** | **138** | **138** | **0** | **0** | **26 of 26** |

The two warnings are Interop's `SUT/Open Notebook.onetoc2` skipping `Encryption.one`, which also
gives the one skipped line, and onenote.rs's `Open Notebook.onetoc2` skipping its recycle bin.
The "Warning" absences say no notebook wrote any other. No committed notebook lists a section
group, so "Section parent" is only ever expected absent; the tests compose one
(`tests/notebooks.rs`), which this run doesn't score.

### Content fields, against content.json

`tests/fixtures/content.json` gives content fields as JSON pointers into page lines, each with
the value it must hold, or null for a field that must be absent: the same values
`tests/content.rs`, `tests/media.rs` and `tests/ink.rs` assert. Most come from onenote.rs's test
page, which `New Section 1.one` and `handwriting_recognition.one` both hold, so its fields count
once per file.

| Field kind | Expected | Recovered | Mismatched | Missing | Absent, as expected |
|---|---|---|---|---|---|
| Container (kind and offsets) | 19 | 19 | 0 | 0 | |
| Paragraph style | 10 | 10 | 0 | 0 | |
| Run, plain | 14 | 14 | 0 | 0 | |
| Run, formatted (bold, italic, underline, strikethrough, subscript, superscript) | 12 | 12 | 0 | 0 | |
| Run, coloured (highlight, font colour) | 4 | 4 | 0 | 0 | |
| Link | 4 | 4 | 0 | 0 | |
| Note tag | 6 | 6 | 0 | 0 | |
| List marker | 18 | 18 | 0 | 0 | |
| Table | 4 | 4 | 0 | 0 | |
| Math (LaTeX) | 2 | 2 | 0 | 0 | |
| Image | 20 | 20 | 0 | 0 | 2 of 2 |
| Embedded file | 2 | 2 | 0 | 0 | |
| Ink (strokes and bounds) | 10 | 10 | 0 | 0 | 1 of 1 |
| Recognised words | 4 | 4 | 0 | 0 | 3 of 3 |
| **All** | **129** | **129** | **0** | **0** | **6 of 6** |

A run is scored as its whole object, so a run with a flag it shouldn't have is a mismatch. A
table is scored whole, rows of cells of paragraphs. The files: `New Section 1.one`,
`handwriting_recognition.one`, `non-legacy/New Section 1 2.one`,
`New Section Group/New Section 2.one`, and Interop's `OneWithFileData.one` and
`FileSyncWOPI_OneNoteWithODB.one`. The public fixtures hold one equation (`a=b`), no ink inside a
paragraph, and no coloured stroke; the CLI's unit tests cover those.

## Time and peak memory

Each run started through `bench/rusage.c`, mail's wrapper, which forks the CLI and reports its own
`wait4` rusage: peak resident memory (`ru_maxrss`) and wall time from fork to exit, the process's
start and exit included. The wrapper's floor, `/bin/true` through it, was 1,528 KiB and 0.68 ms.
stdout and stderr went to files. Every input ran 3 times without `--media` and 3 times with it,
each `--media` run into an empty directory; the tables give the medians of wall time and of peak
memory.

### The large sections

| Section | Size | `--media` | Median wall, of 3 | Runs | Median peak | Lines out | Written |
|---|---|---|---|---|---|---|---|
| Interop's `LargeData.one` | 11,288,928 B | no | 1.296 s | 1.311, 1.293, 1.296 s | 19.5 MiB | 1 section, 1 page | |
| | | yes | 1.307 s | 1.307, 1.286, 1.401 s | 19.4 MiB | the same | 2 files, 8,127,445 B |
| onenote.rs's `Large Desktop.one` | 112,965,624 B | no | 0.253 s | 0.235, 0.253, 0.261 s | 135.0 MiB | 1 section, 1 page | |
| | | yes | 0.267 s | 0.253, 0.267, 0.274 s | 134.8 MiB | the same | 11 files, 112,889,106 B |
| onenote.rs's `Large OneDrive.one` | 161,469,274 B | no | 0.240 s | 0.240, 0.233, 0.247 s | 134.9 MiB | 1 section, 1 page | |
| | | yes | 0.253 s | 0.261, 0.245, 0.253 s | 135.1 MiB | the same | 11 files, 112,889,106 B |

None wrote a warning. CPU time, per run: `LargeData.one` about 0.73 s user and 0.56 s system; the
other two about 0.12 s user and 0.11 s system. User and system time add up to the wall time: one
core does the work.

**`LargeData.one`** is `FileSyncandWOPI/Setup/SUT/LargeData.one` from
[OfficeDev/Interop-TestSuites](https://github.com/OfficeDev/Interop-TestSuites) (MIT) at
`fe87ed32`, the commit the committed Interop set is pinned at, SHA-256
`034d91a640d655a522efda1d98de4ec4c952aaa02e5ad59f7c35537ccf9c2ba1`. It is a desktop-format
section of one page, "testing - large", holding 135 images and one embedded file, each with the
filename `testing - large.docx` and no stored extension. All 135 images are one content,
8,106,671 bytes, which begins as a ZIP does (`PK`); the embedded file is another, 20,774 bytes; so
`--media` writes 2 files, both named `.docx`. The CLI reads each image's bytes to hash them for its name, about
1.09 GB in all, which is where the time goes: 1.3 s for 11 MB, against 0.25 s for the 113 MB
sections. The run doesn't profile it.

**`Large Desktop.one`** and **`Large OneDrive.one`** are `crates/parser/tests/samples/` from
[msiemens/onenote.rs](https://github.com/msiemens/onenote.rs) (MPL-2.0) at `v2.1.0` (`727389a`),
both Git LFS objects, fetched through GitHub's LFS media URL; an LFS object's SHA-256 is its
pointer's oid, `0a568f02...4604668` and `713980fd...9651a`. They are one page, "image_heavy", in
two formats: `Large Desktop.one` in the revision-store format (file format GUID
`109ADD3F-911B-49F5-A5D0-1791EDC8AED8`), and `Large OneDrive.one` in FSSHTTPB packaging
(`638DE92F-A6D4-4BC1-9A36-B3FC2511A5B7`), the format onenote_parser issue #48 is about. Each
page holds 8 images, `image_heavy_1.pdf` to `_8.pdf`, each with its printout page number, and 5
embedded PDFs: 11 distinct contents of 112,889,106 bytes, the largest 67,528,569 bytes.
`Large OneDrive.one` is 161 MB, not the 113 MB the brief gave for both.

The two reads are the same model but for the ids and the 8 images' extension.
`Large OneDrive.one` stores `png` for each printout image, and the CLI names its file `.png`;
`Large Desktop.one` stores none, so the CLI takes the extension of the image's filename,
`image_heavy_<n>.pdf`, and writes the same PNG bytes as `.pdf`. That is behavior 12 as the brief
gives it (the image's own extension, else its filename's), but a desktop printout's file is
misnamed; processing's onenote adapter shouldn't trust a printout image's extension.

### onenote_parser issue #48

[#48](https://github.com/msiemens/onenote.rs/issues/48), open, reports that opening an FSSHTTPB
section parses every cell of its storage index, not only the cells reachable from its data root,
so a 375 MB section with a long edit history needed over 9 GB; it puts the cost at about 3 MB
per cell, most of them old page versions.

`Large OneDrive.one` doesn't show it at 161 MB: it peaks at 135 MiB, as its desktop-format twin
does, and reads in 0.24 s. The peak is about twice the largest content, 64.4 MiB, and doesn't
grow with `--media`; onenote_parser 2.1 reads a file on demand, by positional reads, not whole.
By #48's account the cost follows the unreferenced cells, an edit history, which this sample may
not have; this run doesn't count the file's cells, so it can't say. So #48 stays a known limit for
large sections synced from a long-edited OneDrive or SharePoint notebook, and processing's
onenote adapter should still bound the CLI's memory.

### The committed inputs

Three runs each way; each is under 10 ms and 4.5 MiB, mostly the process's own start:

| Input | Size | Exit | Median wall, without and with `--media` | Median peak | Sections, pages, skipped | Warnings | Written with `--media` |
|---|---|---|---|---|---|---|---|
| `interop/Resources/AlternativePackaging.one` | 20,495 B | 0 | 1.7, 1.5 ms | 2.1, 2.1 MiB | 1, 1, 0 | 0 | none |
| `interop/SUT/Encryption.one` | 16,328 B | 3 | 0.8, 0.7 ms | 1.9, 1.9 MiB | refused | 1 error | none |
| `interop/SUT/FileSyncWOPI_OneNoteWithODB.one` | 898,136 B | 0 | 3.7, 2.7 ms | 4.4, 4.4 MiB | 1, 1, 0 | 0 | 1 file, 879,394 B |
| `interop/SUT/InvalidData.one` | 283,504 B | 0 | 4.2, 3.4 ms | 2.4, 2.3 MiB | 1, 1, 0 | 0 | none |
| `interop/SUT/OneWithFileData.one` | 30,336 B | 0 | 1.6, 1.7 ms | 2.1, 2.1 MiB | 1, 1, 0 | 0 | 1 file, 14,660 B |
| `interop/SUT/OneWithoutFileData.one` | 26,216 B | 0 | 2.1, 2.8 ms | 2.1, 2.1 MiB | 1, 1, 0 | 0 | none |
| `onenote-rs/New Section 1.one` | 264,421 B | 0 | 5.9, 5.6 ms | 3.1, 3.3 MiB | 1, 1, 0 | 0 | 2 files, 94,875 B |
| `onenote-rs/New Section Group/New Section 1.one` | 12,796 B | 0 | 1.2, 1.2 ms | 1.9, 2.1 MiB | 1, 1, 0 | 0 | none |
| `onenote-rs/New Section Group/New Section 2.one` | 166,743 B | 0 | 2.4, 1.8 ms | 2.4, 2.1 MiB | 1, 2, 0 | 0 | 2 files, 104,425 B |
| `onenote-rs/OneNote_RecycleBin/OneNote_DeletedPages.one` | 8,457 B | 0 | 1.1, 1.0 ms | 2.1, 2.1 MiB | 1, 1, 0 | 0 | none |
| `onenote-rs/handwriting_recognition.one` | 180,020 B | 0 | 7.2, 8.0 ms | 3.1, 3.1 MiB | 1, 2, 0 | 0 | 2 files, 144,956 B |
| `onenote-rs/non-legacy/New Section 1 2.one` | 274,127 B | 0 | 5.0, 5.6 ms | 3.3, 3.3 MiB | 1, 2, 0 | 0 | 2 files, 94,875 B |
| `onenote-rs/non-legacy/New Section 2.one` | 19,912 B | 0 | 1.6, 1.6 ms | 2.1, 2.1 MiB | 1, 2, 0 | 0 | none |
| `onenote-rs/non-legacy/New Section 3.one` | 9,266 B | 0 | 1.1, 1.0 ms | 1.9, 2.1 MiB | 1, 1, 0 | 0 | none |
| `interop/Resources/AlternativePackaging.onetoc2` | 3,711 B | 0 | 1.2, 1.1 ms | 2.5, 2.5 MiB | 0, 0, 0 | 0 | none |
| `interop/Resources/NoSection.onetoc2` | 9,553 B | 0 | 1.9, 1.9 ms | 2.5, 2.5 MiB | 0, 0, 0 | 0 | none |
| `interop/Resources/Open Notebook.onetoc2` | 18,000 B | 0 | 2.5, 3.3 ms | 2.4, 2.6 MiB | 0, 0, 0 | 0 | none |
| `interop/SUT/Open Notebook.onetoc2` | 18,088 B | 0 | 6.7, 6.2 ms | 2.9, 2.9 MiB | 3, 3, 1 | 1: `Encryption.one` skipped | 1 file, 14,660 B |
| `onenote-rs/New Section Group/Open Notebook.onetoc2` | 2,454 B | 0 | 2.3, 3.7 ms | 2.5, 2.6 MiB | 2, 3, 0 | 0 | 2 files, 104,425 B |
| `onenote-rs/non-legacy/Open Notebook.onetoc2` | 4,710 B | 0 | 6.4, 6.1 ms | 3.6, 3.6 MiB | 3, 5, 0 | 0 | 2 files, 94,875 B |
| `onenote-rs/object-revision/Open Notebook.onetoc2` | 6,464 B | 0 | 1.2, 1.4 ms | 2.5, 2.5 MiB | 0, 0, 0 | 0 | none |
| `onenote-rs/OneNote_RecycleBin/Open Notebook.onetoc2` | 2,251 B | 0 | 1.2, 1.5 ms | 2.5, 2.5 MiB | 1, 1, 0 | 0 | none |
| `onenote-rs/Open Notebook.onetoc2` | 2,245 B | 0 | 4.9, 6.1 ms | 3.7, 3.6 MiB | 1, 1, 0 | 1: the recycle bin skipped | 2 files, 94,875 B |

A notebook's size is its `.onetoc2`'s; its read reads the sections beside it. The paths are
under `tests/fixtures/`. A section's ink drawing is among the files `--media` writes. The three
`Resources` notebooks and `object-revision`'s give their notebook line alone, as
`notebooks.json` expects.

## Known limits

The CLI's limits, as its README lists them, and what run v0.1.0 shows of each:

- **A password-protected section is refused**, and skipped inside a notebook: Interop's
  `Encryption.one`, both ways, above.
- **No OneNote-written `.onepkg` is tested**, and none was measured: the tests build their own.
- **No authored hand truth, and no desktop-format hand truth**: every expectation here is from
  the public fixtures, written from onenote_parser's API, so a field the parser misreads the
  expectations would share. The content fields come mostly from one test page.
- **Merged table cells**, **page history**, and **onenote_parser's #39 and #50**: no input here
  shows them. **#48**: not shown at 161 MB, above.
- **Skipped lines come after the notebook's entries; a section failing inside a section group is
  lost silently; a missing table-of-contents entry gets no warning**: the one skipped section here
  is at the top level, after its notebook's entries, as expected.

The finding has limits of its own: the times are warm (the page cache held every input); only
three inputs are larger than 1 MB, two of them one page in two formats; and the large sections
were scored for reading and timing alone, not against any expectation.

## Rerunning

From this repository's root, on a laptop (never in CI, never in the check):

```sh
mise run findings
```

`mise run findings` (`scripts/findings.sh`) fetches the three large sections into
`.state/findings/large/` at the pinned commits and refuses each unless its SHA-256 is the pinned
one, builds the release binary, the scorer (`bench/findings.rs`, a Cargo example the check
compiles and never runs), and `bench/rusage.c` (with `cc`), then scores the fixtures and times
every input, printing the tables. A plain run keeps nothing: it writes
`.state/findings/results.json`, which the next run overwrites, and `.state/` is ignored. To keep
a run, record it:

```sh
mise run findings -- --record [name]
```

That copies the results to `docs/findings/onenote-cli/runs/<name>/results.json`, `<name>` by
default `<date>-<n>`, the first such directory that doesn't exist; a name is letters, digits,
`.`, `_`, and `-`, not starting with `.`, and an existing run directory is never overwritten.
Run `v0.1.0`'s scoring and timing took 11 s, after the builds and the fetch (about 285 MB).
`FINDINGS_RUNS` (3) sets the runs of every input.

In `results.json` (schema 1): `cli` is the binary's version, commit, SHA-256, and size; `machine`
the CPU, logical CPUs, memory, and kernel; `recovery` holds, for each table (`sections`,
`notebooks`, `content`), its files, each kind's counts, and every miss with its file, record,
kind, outcome, and expected and written values; `rusage_floor` the wrapper's own; `large` each
large section's source, SHA-256, and size; and `timing` each input's runs (status, exit code, peak
memory, wall, user, and system time), median wall, median and most peak memory, stdout size,
whether stdout was the same every run, its lines by kind, its stderr lines, and what `--media`
wrote.
