//! Ink as SVG, written by the CLI's own code. onenote_parser gives each stroke's path in ink
//! units of 0.01 mm, and a container's offsets in half-inch units; a drawing here is its
//! strokes in ink units, moved to where they are drawn, and its SVG is one path per stroke with
//! the stroke's colour, width, transparency and pen tip. one2html's MIT ink renderer was read as
//! a reference for the colour, transparency and pen-tip mappings; no code is taken from it.
//!
//! The SVG is a function of the strokes alone, its numbers rounded to two decimals in a fixed
//! form, so the same ink always gives the same bytes, and so the same file name.

use std::fmt::Write as _;

use onenote_parser::contents::{EmbeddedObject, Ink, InkStroke};

use crate::model::Bounds;

/// Ink units, 0.01 mm each, in the half inch that offsets count in.
const UNITS_PER_HALF_INCH: f32 = 1270.0;

/// One stroke, its points absolute and in ink units.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub points: Vec<(f32, f32)>,
    /// The pen's width, in ink units.
    pub width: f32,
    /// A COLORREF, 0x00BBGGRR; none is the default, black.
    pub color: Option<u32>,
    /// 0 is opaque, 255 fully transparent, as ISF's drawing attribute and one2html read it.
    pub transparency: Option<u8>,
    /// 0 is a round ball, 1 a rectangle, as in ISF.
    pub pen_tip: Option<u8>,
}

impl Stroke {
    fn read(stroke: &InkStroke, (dx, dy): (f32, f32)) -> Self {
        Self {
            points: stroke
                .absolute_path()
                .iter()
                .map(|point| (point.x() + dx, point.y() + dy))
                .collect(),
            width: stroke.width(),
            color: stroke.color(),
            transparency: stroke.transparency(),
            pen_tip: stroke.pen_tip(),
        }
    }
}

/// An ink object's strokes, nested groups' among them, added to `out`. Each group is moved by
/// its offsets from its parent; the object itself is moved by its own offsets only when
/// `placed`, as a page's ink is from the page's origin. Ink in a paragraph or an outline is
/// drawn alone, at its own coordinates.
pub fn strokes(ink: &Ink, placed: bool, out: &mut Vec<Stroke>) {
    let origin = if placed {
        offset(ink, (0.0, 0.0))
    } else {
        (0.0, 0.0)
    };
    collect(ink, origin, out);
}

fn collect(ink: &Ink, origin: (f32, f32), out: &mut Vec<Stroke>) {
    out.extend(
        ink.ink_strokes()
            .iter()
            .map(|stroke| Stroke::read(stroke, origin)),
    );
    for child in ink.child_groups() {
        collect(child, offset(child, origin), out);
    }
}

fn offset(ink: &Ink, (x, y): (f32, f32)) -> (f32, f32) {
    (
        x + ink.offset_horizontal().unwrap_or(0.0) * UNITS_PER_HALF_INCH,
        y + ink.offset_vertical().unwrap_or(0.0) * UNITS_PER_HALF_INCH,
    )
}

/// What a paragraph's embedded objects lay out: an ink's strokes, a space, or a line break.
#[derive(Debug)]
pub enum Piece {
    Ink(Vec<Stroke>),
    /// A space's width, in half-inch units, as one2html reads it.
    Space(f32),
    LineBreak,
}

/// A paragraph's embedded objects as pieces to lay out.
pub fn pieces(objects: &[EmbeddedObject]) -> Vec<Piece> {
    objects
        .iter()
        .map(|object| match object {
            EmbeddedObject::Ink(container) => {
                let mut out = Vec::new();
                strokes(container.ink(), false, &mut out);
                Piece::Ink(out)
            }
            EmbeddedObject::InkSpace(space) => Piece::Space(space.width()),
            EmbeddedObject::InkLineBreak => Piece::LineBreak,
        })
        .collect()
}

/// Lays a paragraph's ink out as it reads: each ink, from the left edge of its extent, after
/// the one before it, and a space's width between; each line below the one before it. Within
/// a line the inks keep their own vertical coordinates, which keeps a common baseline.
pub fn flow(pieces: Vec<Piece>) -> Vec<Stroke> {
    let mut out = Vec::new();
    let mut top = 0.0;
    for line in pieces.split(|piece| matches!(piece, Piece::LineBreak)) {
        let mut x = 0.0;
        let mut drawn: Vec<Stroke> = Vec::new();
        for piece in line {
            match piece {
                Piece::Ink(strokes) => {
                    let Some(extent) = extent(strokes) else {
                        continue;
                    };
                    drawn.extend(moved(strokes, x - extent.x, 0.0));
                    x += extent.width;
                }
                Piece::Space(width) => x += width * UNITS_PER_HALF_INCH,
                Piece::LineBreak => {}
            }
        }
        if let Some(extent) = extent(&drawn) {
            out.extend(moved(&drawn, 0.0, top - extent.y));
            top += extent.height;
        }
    }
    out
}

fn moved(strokes: &[Stroke], dx: f32, dy: f32) -> Vec<Stroke> {
    strokes
        .iter()
        .map(|stroke| Stroke {
            points: stroke
                .points
                .iter()
                .map(|&(x, y)| (x + dx, y + dy))
                .collect(),
            ..stroke.clone()
        })
        .collect()
}

/// The box the strokes' points span, in ink units; none when they have no points.
pub fn extent(strokes: &[Stroke]) -> Option<Bounds> {
    let mut points = strokes.iter().flat_map(|stroke| stroke.points.iter());
    let &(x, y) = points.next()?;
    let (mut x0, mut y0, mut x1, mut y1) = (x, y, x, y);
    for &(x, y) in points {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    Some(Bounds {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    })
}

/// A drawing's SVG, and the extent it draws; none for strokes with no points. The view box is
/// the extent, widened by half the widest pen on every side so no stroke is cut, and the
/// image's size is the view box's in millimetres.
pub fn svg(strokes: &[Stroke]) -> Option<(Vec<u8>, Bounds)> {
    let bounds = extent(strokes)?;
    let pad = strokes.iter().map(|s| s.width).fold(0.0, f32::max) / 2.0;
    let (x, y) = (bounds.x - pad, bounds.y - pad);
    let (width, height) = (bounds.width + 2.0 * pad, bounds.height + 2.0 * pad);
    let mut svg = String::new();
    svg.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" \
         viewBox=\"{} {} {} {}\">",
        number(width / 100.0),
        number(height / 100.0),
        number(x),
        number(y),
        number(width),
        number(height),
    );
    for stroke in strokes.iter().filter(|stroke| !stroke.points.is_empty()) {
        path(&mut svg, stroke);
    }
    svg.push_str("</svg>\n");
    Some((svg.into_bytes(), bounds))
}

/// One stroke's path. A stroke of one point is drawn as a dot, a line of no length, which its
/// pen's cap shows.
fn path(svg: &mut String, stroke: &Stroke) {
    let mut d = String::new();
    for (index, &(x, y)) in stroke.points.iter().enumerate() {
        let command = match index {
            0 => "M",
            1 => " L",
            _ => "",
        };
        let _ = write!(d, "{command} {} {}", number(x), number(y));
    }
    if stroke.points.len() == 1 {
        d.push_str(" l 0 0");
    }
    let (cap, join) = match stroke.pen_tip.unwrap_or(0) {
        0 => ("round", "round"),
        _ => ("square", "bevel"),
    };
    let _ = write!(
        svg,
        "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" \
         stroke-linecap=\"{cap}\" stroke-linejoin=\"{join}\"",
        d.trim_start(),
        color(stroke.color),
        number(stroke.width),
    );
    if let Some(transparency) = stroke.transparency.filter(|&t| t > 0) {
        let opacity = f32::from(255 - transparency) / 255.0;
        let _ = write!(svg, " stroke-opacity=\"{}\"", number(opacity));
    }
    svg.push_str("/>\n");
}

/// A COLORREF as "#rrggbb"; none is black, the pen's default.
fn color(color: Option<u32>) -> String {
    let value = color.unwrap_or(0);
    let [r, g, b, _] = value.to_le_bytes();
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// A number as the SVG writes it: rounded to two decimals, with no exponent, no trailing
/// zeros, and no negative zero.
fn number(value: f32) -> String {
    let rounded = (f64::from(value) * 100.0).round() / 100.0 + 0.0;
    format!("{rounded}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(points: &[(f32, f32)]) -> Stroke {
        Stroke {
            points: points.to_vec(),
            width: 10.0,
            color: None,
            transparency: None,
            pen_tip: None,
        }
    }

    fn text(strokes: &[Stroke]) -> String {
        String::from_utf8(svg(strokes).unwrap().0).unwrap()
    }

    #[test]
    fn numbers_are_rounded_to_two_decimals_without_negative_zero() {
        assert_eq!(number(1363.0), "1363");
        assert_eq!(number(18.75), "18.75");
        assert_eq!(number(0.123_456), "0.12");
        assert_eq!(number(-0.001), "0");
        assert_eq!(number(-2.5), "-2.5");
        assert_eq!(number(1.0e-9), "0");
        assert_eq!(number(165.63), "165.63");
    }

    #[test]
    fn colours_are_colorrefs_and_none_is_black() {
        // 0x00BBGGRR: red is the low byte.
        assert_eq!(color(Some(0x0000_00ff)), "#ff0000");
        assert_eq!(color(Some(0x00c0_8040)), "#4080c0");
        assert_eq!(color(None), "#000000");
    }

    #[test]
    fn a_drawing_is_one_path_per_stroke_in_a_padded_view_box() {
        let svg = text(&[
            stroke(&[(100.0, 200.0), (150.0, 260.0)]),
            stroke(&[(300.0, 210.0)]),
        ]);
        assert_eq!(
            svg,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"2.1mm\" height=\"0.7mm\" \
             viewBox=\"95 195 210 70\">\n\
             <path d=\"M 100 200 L 150 260\" fill=\"none\" stroke=\"#000000\" \
             stroke-width=\"10\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\n\
             <path d=\"M 300 210 l 0 0\" fill=\"none\" stroke=\"#000000\" \
             stroke-width=\"10\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\n\
             </svg>\n"
        );
    }

    #[test]
    fn a_stroke_keeps_its_colour_transparency_and_pen_tip() {
        let mut highlighter = stroke(&[(0.0, 0.0), (10.0, 0.0), (20.0, 5.0)]);
        highlighter.color = Some(0x0000_ffff);
        highlighter.transparency = Some(128);
        highlighter.pen_tip = Some(1);
        highlighter.width = 450.0;
        let svg = text(&[highlighter]);
        assert!(
            svg.contains(
                "<path d=\"M 0 0 L 10 0 20 5\" fill=\"none\" stroke=\"#ffff00\" \
                 stroke-width=\"450\" stroke-linecap=\"square\" stroke-linejoin=\"bevel\" \
                 stroke-opacity=\"0.5\"/>"
            ),
            "{svg}"
        );
        // An opaque stroke has no opacity attribute.
        let mut opaque = stroke(&[(0.0, 0.0)]);
        opaque.transparency = Some(0);
        assert!(!text(&[opaque]).contains("opacity"));
    }

    #[test]
    fn strokes_without_points_draw_nothing() {
        assert!(svg(&[]).is_none());
        assert!(svg(&[stroke(&[])]).is_none());
        // An empty stroke beside a drawn one gives no path.
        assert_eq!(
            text(&[stroke(&[]), stroke(&[(1.0, 1.0)])])
                .matches("<path")
                .count(),
            1
        );
    }

    #[test]
    fn the_same_strokes_give_the_same_bytes() {
        let strokes = [stroke(&[(0.1, 0.2), (1.0 / 3.0, 2.0 / 3.0)])];
        assert_eq!(svg(&strokes).unwrap().0, svg(&strokes).unwrap().0);
    }

    /// Two inks side by side with a space between, then a line break and a third ink: the
    /// second starts where the first ends, plus the space, and the third line starts below the
    /// first at the left.
    #[test]
    fn a_paragraphs_ink_flows_left_to_right_then_down() {
        let flowed = flow(vec![
            Piece::Ink(vec![stroke(&[(500.0, 1000.0), (600.0, 1100.0)])]),
            Piece::Space(0.1),
            Piece::Ink(vec![stroke(&[(40.0, 1050.0), (90.0, 1200.0)])]),
            Piece::LineBreak,
            Piece::Ink(vec![stroke(&[(7.0, 3.0), (17.0, 53.0)])]),
        ]);
        let points: Vec<&[(f32, f32)]> = flowed.iter().map(|s| s.points.as_slice()).collect();
        // The first line spans y 1000 to 1200; it moves to the top, 200 high.
        assert_eq!(
            points,
            [
                &[(0.0, 0.0), (100.0, 100.0)][..],
                &[(227.0, 50.0), (277.0, 200.0)][..],
                &[(0.0, 200.0), (10.0, 250.0)][..],
            ]
        );
    }

    #[test]
    fn empty_ink_and_empty_lines_take_no_room() {
        let flowed = flow(vec![
            Piece::Ink(Vec::new()),
            Piece::LineBreak,
            Piece::LineBreak,
            Piece::Ink(vec![stroke(&[(5.0, 5.0)])]),
        ]);
        assert_eq!(flowed, [stroke(&[(0.0, 0.0)])]);
    }

    #[test]
    fn the_extent_spans_every_point() {
        let bounds = extent(&[stroke(&[(3.0, 9.0), (-1.0, 4.0)]), stroke(&[(7.0, 2.0)])]);
        assert_eq!(
            bounds,
            Some(Bounds {
                x: -1.0,
                y: 2.0,
                width: 8.0,
                height: 7.0
            })
        );
        assert_eq!(extent(&[]), None);
    }
}
