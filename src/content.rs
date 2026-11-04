//! A page's content: its title and its containers, mapped from onenote_parser's model into
//! schema "1"'s, with outline items nested as stored, paragraphs split into runs, and images'
//! and embedded files' contents named, and written, by the media directory.

use std::io::Read;
use std::ops::Range;

use onenote_parser::contents::{
    Content, EmbeddedFile, FileDataStatus, Image, List, NoteTag, Outline, OutlineElement,
    OutlineItem, ParagraphStyling, RichText, Table,
};
use onenote_parser::page::{Page, PageContent};
use onenote_parser::property::common::ColorRef;
use onenote_parser::property::embedded_file::FileType;

use crate::media::{self, Media};
use crate::model::{self, Block, Container, Item, Offsets};
use crate::section::iso8601;
use crate::{Fault, warn};

/// The page's title, when it has one, then its containers in stored order. A container of a
/// kind onenote_parser doesn't know is left out. Images' and embedded files' contents go to
/// `media` as they are read, so each is written before the page line that names it; `place`
/// names the page in warnings.
pub fn page(page: &Page, media: &mut Media, place: &str) -> Result<Vec<Container>, Fault> {
    let mut reader = Reader {
        media,
        place,
        images: 0,
        files: 0,
    };
    let mut containers = Vec::new();
    if let Some(title) = page.title() {
        let mut items = Vec::new();
        for outline in title.contents() {
            items.extend(reader.items(outline.items())?);
        }
        containers.push(Container::Title {
            offsets: Offsets {
                offset_horizontal: Some(title.offset_horizontal()),
                offset_vertical: Some(title.offset_vertical()),
            },
            items,
        });
    }
    for content in page.contents() {
        containers.push(match content {
            PageContent::Outline(outline) => Container::Outline {
                offsets: outline_offsets(outline),
                items: reader.items(outline.items())?,
            },
            PageContent::Image(image) => Container::Image {
                offsets: Offsets {
                    offset_horizontal: image.offset_horizontal(),
                    offset_vertical: image.offset_vertical(),
                },
                image: reader.image(image)?,
            },
            PageContent::EmbeddedFile(file) => Container::EmbeddedFile {
                offsets: Offsets {
                    offset_horizontal: file.offset_horizontal(),
                    offset_vertical: file.offset_vertical(),
                },
                file: reader.embedded_file(file)?,
            },
            PageContent::Ink(ink) => Container::Ink(Offsets {
                offset_horizontal: ink.offset_horizontal(),
                offset_vertical: ink.offset_vertical(),
            }),
            PageContent::Unknown => continue,
        });
    }
    Ok(containers)
}

fn outline_offsets(outline: &Outline) -> Offsets {
    Offsets {
        offset_horizontal: outline.offset_horizontal(),
        offset_vertical: outline.offset_vertical(),
    }
}

/// One page's read: the media its images and embedded files go to, and the counts that name
/// each in a warning, in the order they are read.
struct Reader<'a> {
    media: &'a mut Media,
    place: &'a str,
    images: usize,
    files: usize,
}

impl Reader<'_> {
    fn items(&mut self, items: &[OutlineItem]) -> Result<Vec<Item>, Fault> {
        items
            .iter()
            .map(|item| match item {
                OutlineItem::Element(element) => self.element(element),
                OutlineItem::Group(group) => Ok(Item::Group {
                    items: self.items(group.outlines())?,
                }),
            })
            .collect()
    }

    /// An outline element. Its list marker goes on its first block when that is a paragraph:
    /// onenote_parser keeps the marker on the element, and an element holds one paragraph in
    /// every file seen.
    fn element(&mut self, element: &OutlineElement) -> Result<Item, Fault> {
        let mut list = element.list_contents().first().and_then(marker);
        let mut blocks = Vec::new();
        for (index, content) in element.contents().iter().enumerate() {
            blocks.push(match content {
                Content::RichText(text) => {
                    let marker = if index == 0 { list.take() } else { None };
                    Block::Paragraph(paragraph(text, marker))
                }
                Content::Table(table) => Block::Table(self.table(table)?),
                Content::Image(image) => Block::Image(self.image(image)?),
                Content::EmbeddedFile(file) => Block::EmbeddedFile(self.embedded_file(file)?),
                Content::Ink(_) => Block::Ink,
                Content::Unknown => continue,
            });
        }
        Ok(Item::Element {
            blocks,
            children: self.items(element.children())?,
        })
    }

    fn table(&mut self, table: &Table) -> Result<model::Table, Fault> {
        let mut rows = Vec::new();
        for row in table.contents() {
            let mut cells = Vec::new();
            for cell in row.contents() {
                cells.push(model::Cell {
                    items: cell
                        .contents()
                        .iter()
                        .map(|element| self.element(element))
                        .collect::<Result<_, _>>()?,
                });
            }
            rows.push(cells);
        }
        Ok(model::Table {
            tags: tags(table.note_tags()),
            rows,
        })
    }

    /// An image. Its file is named by its own stored extension, else its filename's.
    fn image(&mut self, image: &Image) -> Result<model::Image, Fault> {
        self.images += 1;
        let place = format!("{}, image {}", self.place, self.images);
        let extension = image
            .extension()
            .map(|ext| ext.strip_prefix('.').unwrap_or(ext))
            .filter(|ext| !ext.is_empty());
        let filename = text(image.image_filename());
        let available = image.data_status() == FileDataStatus::Available;
        let file = self.file(
            image.data_status(),
            image.read(),
            extension.or_else(|| filename.as_deref().and_then(media::extension_of)),
            &place,
        )?;
        Ok(model::Image {
            file,
            size: image.size().filter(|_| available),
            extension: extension.map(str::to_owned),
            filename,
            alt_text: text(image.alt_text()),
            ocr_text: text(image.text()),
            hyperlink: text(image.hyperlink_url()),
            printout_page: image.displayed_page_number(),
        })
    }

    /// An embedded file. Its file is named by its filename's extension.
    fn embedded_file(&mut self, file: &EmbeddedFile) -> Result<model::EmbeddedFile, Fault> {
        self.files += 1;
        let place = format!("{}, embedded file {}", self.place, self.files);
        let filename = text(Some(file.filename()));
        let available = file.data_status() == FileDataStatus::Available;
        Ok(model::EmbeddedFile {
            file: self.file(
                file.data_status(),
                Some(file.read()),
                filename.as_deref().and_then(media::extension_of),
                &place,
            )?,
            filename,
            file_type: match file.file_type() {
                FileType::Audio => Some("audio"),
                FileType::Video => Some("video"),
                FileType::Unknown => None,
            },
            size: available.then(|| file.size()),
        })
    }

    /// A record's file: its content stored in the media and named. Data that is missing,
    /// invalid, or can't be read gives no file and a warning at `place`.
    fn file(
        &mut self,
        status: FileDataStatus,
        read: Option<Box<dyn Read>>,
        extension: Option<&str>,
        place: &str,
    ) -> Result<Option<String>, Fault> {
        let problem = match (status, read) {
            (FileDataStatus::Available, Some(mut read)) => {
                let mut bytes = Vec::new();
                match read.read_to_end(&mut bytes) {
                    Ok(_) => return self.media.store(&bytes, extension).map(Some),
                    Err(err) => format!("its data can't be read: {err}"),
                }
            }
            (FileDataStatus::Invalid, _) => "its data is invalid".to_owned(),
            _ => "its data is missing".to_owned(),
        };
        warn(place, &format!("no file: {problem}"));
        Ok(None)
    }
}

/// A stored string, or none when it is empty.
fn text(value: Option<&str>) -> Option<String> {
    value.filter(|text| !text.is_empty()).map(str::to_owned)
}

fn paragraph(text: &RichText, list: Option<model::List>) -> model::Paragraph {
    model::Paragraph {
        style: text
            .paragraph_style()
            .style_id()
            .filter(|style| !style.is_empty())
            .map(str::to_owned),
        list,
        tags: tags(text.note_tags()),
        runs: runs(text),
    }
}

/// A paragraph's visible runs. Each stored formatting covers the text up to its run index,
/// counted in UTF-16 code units, and the last covers the rest. Hidden runs, which hold a
/// link's target among others, are left out, and neighbours left with the same formatting are
/// joined into one run.
fn runs(text: &RichText) -> Vec<model::Run> {
    let units: Vec<u16> = text.text().encode_utf16().collect();
    let styles = text.text_run_formatting();
    let default = ParagraphStyling::default();
    // A paragraph with text and no formatting is one run, unformatted.
    let styles: Vec<&ParagraphStyling> = if styles.is_empty() {
        vec![&default]
    } else {
        styles.iter().collect()
    };
    let mut pieces: Vec<Piece> = ranges(units.len(), text.text_run_indices(), styles.len())
        .into_iter()
        .zip(styles)
        .filter(|(range, style)| !range.is_empty() && !style.hidden())
        .map(|(range, style)| Piece {
            run: model::Run {
                text: String::from_utf16_lossy(&units[range.clone()]),
                bold: style.bold(),
                italic: style.italic(),
                underline: style.underline(),
                strikethrough: style.strikethrough(),
                superscript: style.superscript(),
                subscript: style.subscript(),
                highlight: style.highlight().and_then(color),
                font_color: style.font_color().and_then(color),
                link: None,
            },
            range,
            hyperlink: style.hyperlink(),
        })
        .collect();
    let links: Vec<(Range<usize>, String)> = text
        .hyperlinks()
        .iter()
        .map(|link| {
            (
                link.start() as usize..link.end() as usize,
                link.target().to_owned(),
            )
        })
        .collect();
    link(&mut pieces, &links);
    join(pieces)
}

/// A run as it is cut from the paragraph: its UTF-16 range, and whether it is styled as a
/// link.
struct Piece {
    range: Range<usize>,
    run: model::Run,
    hyperlink: bool,
}

/// The UTF-16 ranges of `count` runs over `len` code units, ended by `indices`. The last run
/// takes the rest of the text, so none of it is lost when the indices run short; ranges are
/// kept inside the text and never run backwards.
fn ranges(len: usize, indices: &[u32], count: usize) -> Vec<Range<usize>> {
    let mut start = 0;
    (0..count)
        .map(|index| {
            let end = if index + 1 == count {
                len
            } else {
                indices.get(index).map_or(len, |&end| end as usize)
            };
            let end = end.clamp(start, len);
            let range = start..end;
            start = end;
            range
        })
        .collect()
}

/// Gives each run its link target. A run that starts inside one of onenote_parser's
/// hyperlinks, whose targets come from the hidden HYPERLINK fields, takes its target. A run
/// styled as a link outside any of them is a link OneNote recognised in typed text, and its
/// target is its own text, joined across the neighbouring link-styled runs it spans.
fn link(pieces: &mut [Piece], links: &[(Range<usize>, String)]) {
    for piece in pieces.iter_mut() {
        piece.run.link = links
            .iter()
            .find(|(range, _)| range.contains(&piece.range.start))
            .map(|(_, target)| target.clone());
    }
    let mut index = 0;
    while index < pieces.len() {
        if !pieces[index].hyperlink || pieces[index].run.link.is_some() {
            index += 1;
            continue;
        }
        let mut end = index + 1;
        while end < pieces.len()
            && pieces[end].hyperlink
            && pieces[end].run.link.is_none()
            && pieces[end].range.start == pieces[end - 1].range.end
        {
            end += 1;
        }
        let target: String = pieces[index..end]
            .iter()
            .map(|piece| piece.run.text.as_str())
            .collect();
        let target = target.trim().to_owned();
        if !target.is_empty() {
            for piece in &mut pieces[index..end] {
                piece.run.link = Some(target.clone());
            }
        }
        index = end;
    }
}

/// The runs, with neighbours of the same formatting and link joined.
fn join(pieces: Vec<Piece>) -> Vec<model::Run> {
    let mut runs: Vec<model::Run> = Vec::with_capacity(pieces.len());
    for piece in pieces {
        match runs.last_mut() {
            Some(last) if same_format(last, &piece.run) => last.text.push_str(&piece.run.text),
            _ => runs.push(piece.run),
        }
    }
    runs
}

fn same_format(a: &model::Run, b: &model::Run) -> bool {
    model::Run {
        text: String::new(),
        ..a.clone()
    } == model::Run {
        text: String::new(),
        ..b.clone()
    }
}

/// A list marker. onenote_parser gives the stored format string less its length: a bullet
/// is its character; a number is U+FFFD followed by its MSONFC format code, with the text
/// written before and after the number around them.
fn marker(list: &List) -> Option<model::List> {
    let format = list.list_format();
    let restart = list.list_restart();
    match format.iter().position(|&c| c == '\u{fffd}') {
        Some(at) if at + 1 < format.len() => {
            let text =
                |chars: &[char]| Some(chars.iter().collect::<String>()).filter(|t| !t.is_empty());
            Some(model::List {
                bullet: None,
                number: Some(number_format(format[at + 1] as u32)),
                prefix: text(&format[..at]),
                suffix: text(&format[at + 2..]),
                restart,
            })
        }
        _ => {
            let bullet: String = format.iter().collect();
            (!bullet.is_empty()).then_some(model::List {
                bullet: Some(bullet),
                number: None,
                prefix: None,
                suffix: None,
                restart,
            })
        }
    }
}

/// An MSONFC numbering format code by name, as [MS-DOC] 2.9.157 lists them; a code without a
/// name here is written as "nfc-" and its number.
fn number_format(code: u32) -> String {
    let name = match code {
        0 => "decimal",
        1 => "upper-roman",
        2 => "lower-roman",
        3 => "upper-letter",
        4 => "lower-letter",
        5 => "ordinal",
        6 => "cardinal-text",
        7 => "ordinal-text",
        8 => "hex",
        22 => "decimal-zero",
        _ => return format!("nfc-{code}"),
    };
    name.to_owned()
}

fn tags(tags: &[NoteTag]) -> Vec<model::Tag> {
    tags.iter()
        .map(|tag| {
            let definition = tag.definition();
            let completed = tag.item_status().completed();
            model::Tag {
                label: definition
                    .map(|d| d.label())
                    .filter(|label| !label.is_empty())
                    .map(str::to_owned),
                shape: definition.map(|d| format!("{:?}", d.shape())),
                status: if completed { "completed" } else { "open" },
                completed_time: tag
                    .completed_at()
                    .filter(|_| completed)
                    .map(|time| iso8601(time.into())),
            }
        })
        .collect()
}

/// A COLORREF as "#rrggbb"; an automatic colour has none.
fn color(color: ColorRef) -> Option<String> {
    match color {
        ColorRef::Auto => None,
        ColorRef::Manual { r, g, b } => Some(format!("#{r:02x}{g:02x}{b:02x}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(range: Range<usize>, text: &str, hyperlink: bool) -> Piece {
        Piece {
            range,
            run: model::Run {
                text: text.to_owned(),
                ..model::Run::default()
            },
            hyperlink,
        }
    }

    #[test]
    fn runs_split_on_utf16_indices() {
        // "𝑎=𝑏" is five UTF-16 code units: a surrogate pair, "=", a surrogate pair.
        let text = "𝑎=𝑏";
        let units: Vec<u16> = text.encode_utf16().collect();
        let ranges = ranges(units.len(), &[2, 3], 3);
        assert_eq!(ranges, vec![0..2, 2..3, 3..5]);
        let parts: Vec<String> = ranges
            .into_iter()
            .map(|range| String::from_utf16_lossy(&units[range]))
            .collect();
        assert_eq!(parts, ["𝑎", "=", "𝑏"]);
    }

    #[test]
    fn the_last_run_takes_the_rest_and_ranges_stay_in_the_text() {
        assert_eq!(ranges(10, &[4], 2), vec![0..4, 4..10]);
        // Too few indices: the middle run ends at the text's end, the last is empty.
        assert_eq!(ranges(10, &[4], 3), vec![0..4, 4..10, 10..10]);
        // An index past the end, or before the previous one, is clamped.
        assert_eq!(ranges(10, &[12, 3], 3), vec![0..10, 10..10, 10..10]);
        assert_eq!(ranges(0, &[], 1), vec![0..0]);
    }

    #[test]
    fn runs_inside_a_hyperlink_take_its_target() {
        let mut pieces = vec![
            piece(0..6, "go to ", false),
            piece(6..11, "magna", true),
            piece(11..12, ".", false),
        ];
        link(&mut pieces, &[(6..11, "https://example.com".to_owned())]);
        let links: Vec<Option<&str>> = pieces.iter().map(|p| p.run.link.as_deref()).collect();
        assert_eq!(links, [None, Some("https://example.com"), None]);
    }

    #[test]
    fn a_recognised_link_targets_its_own_text() {
        let mut pieces = vec![
            piece(0..4, "see ", false),
            piece(4..11, "http://", true),
            piece(11..23, "example.com/", true),
        ];
        link(&mut pieces, &[]);
        let links: Vec<Option<&str>> = pieces.iter().map(|p| p.run.link.as_deref()).collect();
        let target = Some("http://example.com/");
        assert_eq!(links, [None, target, target]);
    }

    #[test]
    fn neighbours_with_the_same_format_join() {
        let mut bold = piece(3..6, "def", false);
        bold.run.bold = true;
        let runs = join(vec![
            piece(0..2, "ab", false),
            piece(2..3, "c", false),
            bold,
        ]);
        let texts: Vec<&str> = runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(texts, ["abc", "def"]);
    }

    #[test]
    fn colours_are_hex_and_auto_has_none() {
        assert_eq!(
            color(ColorRef::Manual {
                r: 255,
                g: 192,
                b: 0
            }),
            Some("#ffc000".to_owned())
        );
        assert_eq!(color(ColorRef::Auto), None);
    }

    /// Data that is missing or invalid gives no file, and nothing goes to the media, which a
    /// directory that can't be created would refuse; no committed fixture holds such a record.
    #[test]
    fn missing_or_invalid_data_has_no_file() {
        let mut media = Media::new(Some("/dev/null/media".into()));
        let mut reader = Reader {
            media: &mut media,
            place: "/Section, page 1",
            images: 0,
            files: 0,
        };
        for status in [FileDataStatus::Missing, FileDataStatus::Invalid] {
            let read: Box<dyn Read> = Box::new(&b"bytes"[..]);
            let file = reader.file(status, Some(read), Some("png"), "/Section, page 1, image 1");
            assert!(matches!(file, Ok(None)), "{status:?}");
        }
        let file = reader.file(FileDataStatus::Available, None, None, "/Section, image 2");
        assert!(matches!(file, Ok(None)));
    }

    #[test]
    fn number_formats_have_names() {
        assert_eq!(number_format(0), "decimal");
        assert_eq!(number_format(4), "lower-letter");
        assert_eq!(number_format(2), "lower-roman");
        assert_eq!(number_format(60), "nfc-60");
    }
}
