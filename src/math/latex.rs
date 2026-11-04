// Ported from one2html 2.0.1, src/page/math/render/latex.rs and render/mod.rs
// (https://github.com/msiemens/one2html), under its MIT licence:
//
// Copyright 2020 Markus Siemens
//
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software
// and associated documentation files (the "Software"), to deal in the Software without
// restriction, including without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all copies or
// substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
// BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
//
// Changed from one2html: the equation is returned bare, not wrapped in \(…\) or \[…\]; the
// renderer can't fail; its log warnings for alignment it doesn't render are left out; and
// itertools' chunks and join are std's.

//! The LaTeX renderer.
//!
//! The output assumes a Unicode-aware math backend downstream: MathJax or KaTeX, or
//! unicode-math with xelatex or lualatex. It keeps no tables for the long tail of Greek letters
//! and operator symbols, since those backends render bare Unicode (α, ≤, ∂, 2π) natively.
//!
//! It does translate:
//! - structural constructs (`\frac`, `\sqrt`, `\sum`, matrix environments, brackets, accents,
//!   sub- and superscripts, `\boxed`...), which are the LaTeX rendering;
//! - styled Mathematical Alphanumeric Symbols (U+1D400 to U+1D7FF), decoded to their base
//!   ASCII or Greek so that `\mathbf{X}` works in pdflatex, where `\mathbf{𝐗}` does not;
//! - LaTeX's syntactic characters (`\`, `{`, `}`, `$`, `&`, `#`, `_`, `^`, `%`), escaped;
//! - n-ary operators, bracket delimiters and accents, from small fixed tables, since LaTeX
//!   spells them differently: `\sum` rather than a bare `∑`, for its limits; `\left\{` rather
//!   than `\left{`; `\hat{x}` rather than a combining U+0302.

use finl_unicode::categories::CharacterCategories;

use super::ast::{
    BoxSize, BracketSize, Equation, MatrixBrackets, NAryDisplay, NAryLimits, Op, PHANTOM_SHOW,
    PHANTOM_TRANSPARENT, PHANTOM_ZERO_ASCENT, PHANTOM_ZERO_DESCENT, PHANTOM_ZERO_WIDTH,
    PhantomKind, StretchStackPosition,
};
use super::text::TextType;

/// An equation's LaTeX.
pub(super) fn render(equation: Equation) -> String {
    render_eq(equation)
}

fn render_eq(eq: Equation) -> String {
    eq.into_iter().map(render_op).collect()
}

fn render_group(eq: Equation) -> String {
    match eq.len() {
        0 => "{}".to_owned(),
        1 => render_op(eq.into_iter().next().expect("one op")),
        _ => format!("{{{}}}", render_eq(eq)),
    }
}

fn render_op(op: Op) -> String {
    match op {
        Op::Text(text) => render_text(&text),
        Op::Accent { char, body } => render_accent(char, body),
        Op::Box {
            body,
            size,
            no_break,
        } => render_box(body, size, no_break),
        Op::BoxedFormula { body } => format!("\\boxed{{{}}}", render_group(body)),
        Op::Brackets {
            open,
            close,
            body,
            size,
        } => sized_brackets(open, close, render_group(body), size),
        Op::BracketsWithSeps {
            open,
            close,
            sep,
            segments,
            size,
        } => {
            let joined = segments
                .into_iter()
                .map(render_group)
                .collect::<Vec<_>>()
                .join(&format!(" \\,\\middle{}\\, ", bracket_delim_str(sep)));
            sized_brackets(open, close, joined, size)
        }
        Op::EquationArray { rows } => format!(
            "\\begin{{aligned}} {} \\end{{aligned}}",
            rows.into_iter()
                .map(render_eq)
                .collect::<Vec<_>>()
                .join(" \\\\ ")
        ),
        Op::Fraction { num, den, small } => format!(
            "{}{{{}}}{{{}}}",
            if small { "\\tfrac" } else { "\\frac" },
            render_eq(num),
            render_eq(den)
        ),
        // The function name is rendered by \operatorname{}, mathop-class, so the argument
        // after it gets TeX's automatic spacing, as `\cos x` does.
        Op::FunctionApply { func, body } => {
            format!("{}{}", render_group(func), render_group(body))
        }
        Op::LeftSubSup { sub, sup, body } => format!(
            "{{}}_{}^{}{}",
            braced_or_empty(sub),
            braced_or_empty(sup),
            render_group(body)
        ),
        Op::LowerLimit { body, limit } => {
            format!("\\underset{{{}}}{{{}}}", render_eq(limit), render_eq(body))
        }
        Op::Matrix {
            columns,
            brackets,
            items,
            placeholders,
        } => render_matrix(columns, items, brackets, placeholders),
        Op::NAry {
            op,
            sub,
            sup,
            body,
            display,
        } => render_nary(op, sub, sup, body, display),
        Op::OverBar { body } => format!("\\overline{{{}}}", render_eq(body)),
        Op::Phantom {
            body,
            kind,
            display,
        } => render_phantom(kind, display, body),
        Op::Radical { body, degree } => {
            if degree.is_empty() {
                format!("\\sqrt{{{}}}", render_eq(body))
            } else {
                format!("\\sqrt[{}]{{{}}}", render_eq(degree), render_eq(body))
            }
        }
        Op::SlashedFraction { num, den, linear } => {
            if linear {
                format!("{}/{}", render_group(num), render_group(den))
            } else {
                format!("{{}}^{{{}}}\\!/\\!_{{{}}}", render_eq(num), render_eq(den))
            }
        }
        Op::Stack { num, den } => {
            format!("\\substack{{{} \\\\ {}}}", render_eq(num), render_eq(den))
        }
        Op::StretchStack { char, body, pos } => render_stretch_stack(char, pos, body),
        Op::Subscript { sub, body } => format!("{}_{}", render_group(body), script(sub)),
        Op::SubSup { sub, sup, body } => {
            format!("{}_{}^{}", render_group(body), script(sub), script(sup))
        }
        Op::Superscript { sup, body } => format!("{}^{}", render_group(body), script(sup)),
        Op::UnderBar { body } => format!("\\underline{{{}}}", render_eq(body)),
        Op::UpperLimit { body, limit } => {
            format!("\\overset{{{}}}{{{}}}", render_eq(limit), render_eq(body))
        }
    }
}

/// A script's argument, braced; an empty one shows a placeholder.
fn script(eq: Equation) -> String {
    if eq.is_empty() {
        "{\\square}".to_owned()
    } else {
        format!("{{{}}}", render_eq(eq))
    }
}

fn braced_or_empty(eq: Equation) -> String {
    if eq.is_empty() {
        "{}".to_owned()
    } else {
        format!("{{{}}}", render_eq(eq))
    }
}

/// A format character other than the function-application operator draws nothing.
fn is_skippable_format(c: char) -> bool {
    c.is_format() && c != '\u{2061}'
}

fn render_text(text: &str) -> String {
    let mut parts: Vec<(TextType, String)> = Vec::new();
    for c in text.chars().filter(|&c| !is_skippable_format(c)) {
        let kind = TextType::from_char(c);
        match parts.last_mut() {
            Some((last, part)) if *last == kind => part.push(c),
            _ => parts.push((kind, c.to_string())),
        }
    }
    parts
        .into_iter()
        .map(|(kind, text)| render_text_part(kind, &text))
        .collect()
}

fn render_text_part(kind: TextType, text: &str) -> String {
    let decoded = || decode_letters(text, kind);
    match kind {
        TextType::Bold => format!("\\mathbf{{{}}}", decoded()),
        TextType::BoldItalic => format!("\\boldsymbol{{{}}}", decoded()),
        TextType::BoldScript => format!("\\boldsymbol{{\\mathcal{{{}}}}}", decoded()),
        TextType::Double => format!("\\mathbb{{{}}}", decoded()),
        // Double-struck italic operators (ⅅ, ⅆ): the backends render the code point; the thin
        // space keeps `dx`-style typography.
        TextType::DoubleOperator => format!("\\,{text}"),
        TextType::Fraktur => format!("\\mathfrak{{{}}}", decoded()),
        TextType::FrakturBold => format!("\\boldsymbol{{\\mathfrak{{{}}}}}", decoded()),
        TextType::Identifier => text.chars().map(decode_styled_letter).collect(),
        TextType::Mono => format!("\\mathtt{{{}}}", decoded()),
        // Upright text. \operatorname{}, not bare text, which italicises (c·o·s), nor
        // \mathrm{}, which is ord-class and leaves neighbours flush: \operatorname{} is
        // upright and mathop-class, spaced as LaTeX's own \cos.
        TextType::Normal => format!("\\operatorname{{{text}}}"),
        TextType::Numeric => text.to_owned(),
        TextType::Operator => text
            .chars()
            .map(decode_styled_letter)
            .map(escape_operator_char)
            .collect(),
        TextType::Raw => text.to_owned(),
        TextType::Sans => format!("\\mathsf{{{}}}", decoded()),
        TextType::SansBold => format!("\\boldsymbol{{\\mathsf{{{}}}}}", decoded()),
        TextType::SansBoldItalic => format!("\\boldsymbol{{\\mathsf{{{}}}}}", decoded()),
        TextType::SansItalic => format!("\\mathsf{{{}}}", decoded()),
        TextType::Script => format!("\\mathcal{{{}}}", decoded()),
        TextType::Space => "\\,".to_owned(),
    }
}

/// A styled run's letters as their base letters: offsets 0 to 25 from its range's start are
/// A to Z, 26 to 51 a to z; anything else, such as a styled Greek letter, is decoded alone.
fn decode_letters(text: &str, kind: TextType) -> String {
    let base = range_start(kind).map(|c| c as u32);
    text.chars()
        .map(|c| {
            if let Some(base) = base {
                let offset = (c as u32).wrapping_sub(base);
                if offset <= 25 {
                    return char::from_u32('A' as u32 + offset).unwrap_or(c);
                }
                if (26..=51).contains(&offset) {
                    return char::from_u32('a' as u32 + offset - 26).unwrap_or(c);
                }
            }
            decode_styled_letter(c)
        })
        .collect()
}

fn range_start(kind: TextType) -> Option<char> {
    Some(match kind {
        TextType::Bold => '\u{1d400}',
        TextType::BoldItalic => '\u{1d468}',
        TextType::BoldScript => '\u{1d4d0}',
        TextType::Double => '\u{1d538}',
        TextType::Fraktur => '\u{1d504}',
        TextType::FrakturBold => '\u{1d56c}',
        TextType::Mono => '\u{1d670}',
        TextType::Sans => '\u{1d5a0}',
        TextType::SansBold => '\u{1d5d4}',
        TextType::SansBoldItalic => '\u{1d63c}',
        TextType::SansItalic => '\u{1d608}',
        TextType::Script => '\u{1d49c}',
        _ => return None,
    })
}

/// A styled Mathematical Alphanumeric Symbol as its base character. Math-italic Latin letters
/// become ASCII, which math mode sets in italic; styled Greek letters become base Greek. Other
/// styles are decoded by their own wrappers.
fn decode_styled_letter(c: char) -> char {
    let code = c as u32;
    if (0x1d434..=0x1d44d).contains(&code) {
        return char::from_u32('A' as u32 + (code - 0x1d434)).unwrap_or(c);
    }
    if (0x1d44e..=0x1d467).contains(&code) {
        return char::from_u32('a' as u32 + (code - 0x1d44e)).unwrap_or(c);
    }
    // The italic small h, which the block leaves to the Planck constant.
    if c == '\u{210e}' {
        return 'h';
    }
    // Styled Greek: bold, italic, bold italic, sans bold and sans bold italic, 58 each.
    for start in [0x1d6a8u32, 0x1d6e2, 0x1d71c, 0x1d756, 0x1d790] {
        if (start..start + 58).contains(&code) {
            return greek_offset_to_base((code - start) as usize).unwrap_or(c);
        }
    }
    c
}

/// A styled Greek block's letter by its offset: capitals, ∇, small letters, ∂, then the
/// variant forms.
fn greek_offset_to_base(offset: usize) -> Option<char> {
    const CAPS: &str = "ΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡϴΣΤΥΦΧΨΩ";
    const LOWERS: &str = "αβγδεζηθικλμνξοπρςστυφχψω";
    const EXTRA: &str = "ϵϑϰϕϱϖ";
    match offset {
        0..=24 => CAPS.chars().nth(offset),
        25 => Some('∇'),
        26..=50 => LOWERS.chars().nth(offset - 26),
        51 => Some('∂'),
        52..=57 => EXTRA.chars().nth(offset - 52),
        _ => None,
    }
}

/// Only the characters LaTeX parses in math mode are escaped; the rest pass as Unicode.
fn escape_operator_char(c: char) -> String {
    match c {
        // The function-application operator: the parser reads it into a function apply,
        // whose rendering spaces it, so one left in text is dropped.
        '\u{2061}' => String::new(),
        '\\' => "\\backslash ".to_owned(),
        '{' => "\\{".to_owned(),
        '}' => "\\}".to_owned(),
        '$' => "\\$".to_owned(),
        '#' => "\\#".to_owned(),
        '%' => "\\%".to_owned(),
        '_' => "\\_".to_owned(),
        '^' => "\\hat{}".to_owned(),
        '&' => "\\&".to_owned(),
        _ => c.to_string(),
    }
}

fn render_accent(char: char, body: Equation) -> String {
    let body = render_group(body);
    match accent_command(char) {
        Some(command) => format!("{command}{{{body}}}"),
        None => format!("\\overset{{{char}}}{{{body}}}"),
    }
}

fn accent_command(c: char) -> Option<&'static str> {
    Some(match c {
        // Combining marks.
        '\u{0300}' => "\\grave",
        '\u{0301}' => "\\acute",
        '\u{0302}' => "\\hat",
        '\u{0303}' => "\\tilde",
        '\u{0304}' => "\\bar",
        '\u{0306}' => "\\breve",
        '\u{0307}' => "\\dot",
        '\u{0308}' => "\\ddot",
        '\u{030A}' => "\\mathring",
        '\u{030C}' => "\\check",
        '\u{20D7}' => "\\vec",
        // Their spacing variants.
        '\u{02C6}' => "\\hat",
        '\u{02DC}' => "\\tilde",
        '\u{00AF}' => "\\bar",
        '\u{02D9}' => "\\dot",
        '\u{00A8}' => "\\ddot",
        '\u{02C7}' => "\\check",
        '\u{02D8}' => "\\breve",
        '\u{02DA}' => "\\mathring",
        _ => return None,
    })
}

fn render_box(body: Equation, size: BoxSize, no_break: bool) -> String {
    let content = render_group(body);
    let content = match size {
        BoxSize::Script => format!("{{\\scriptstyle {content}}}"),
        BoxSize::ScriptScript => format!("{{\\scriptscriptstyle {content}}}"),
        BoxSize::Text => content,
    };
    if no_break {
        format!("\\nobreak {content}")
    } else {
        content
    }
}

/// Brackets around `body`, sized as stored, or growing with it when no size is.
fn sized_brackets(
    open: Option<char>,
    close: Option<char>,
    body: String,
    size: Option<BracketSize>,
) -> String {
    let open = open.map_or_else(|| ".".to_owned(), bracket_delim_str);
    let close = close.map_or_else(|| ".".to_owned(), bracket_delim_str);
    match size {
        Some(BracketSize::DontGrow) => format!("{open}{body}{close}"),
        Some(BracketSize::TeXbig) => format!("\\bigl{open} {body} \\bigr{close}"),
        Some(BracketSize::TeXBig) => format!("\\Bigl{open} {body} \\Bigr{close}"),
        Some(BracketSize::TeXbigg) => format!("\\biggl{open} {body} \\biggr{close}"),
        Some(BracketSize::TeXBigg) => format!("\\Biggl{open} {body} \\Biggr{close}"),
        None => format!("\\left{open} {body} \\right{close}"),
    }
}

fn bracket_delim_str(c: char) -> String {
    match c {
        '{' => "\\{".to_owned(),
        '}' => "\\}".to_owned(),
        '⟨' => "\\langle ".to_owned(),
        '⟩' => "\\rangle ".to_owned(),
        '⌊' => "\\lfloor ".to_owned(),
        '⌋' => "\\rfloor ".to_owned(),
        '⌈' => "\\lceil ".to_owned(),
        '⌉' => "\\rceil ".to_owned(),
        '‖' => "\\|".to_owned(),
        _ => c.to_string(),
    }
}

/// A matrix: its items in rows of `columns`, the last row padded with empty cells.
fn render_matrix(
    columns: u8,
    items: Vec<Equation>,
    brackets: Option<MatrixBrackets>,
    placeholders: bool,
) -> String {
    let env = match brackets {
        Some(MatrixBrackets::Parentheses) => "pmatrix",
        Some(MatrixBrackets::VerticalBars) => "vmatrix",
        Some(MatrixBrackets::DoubleVerticalBars) => "Vmatrix",
        None => "matrix",
    };
    let columns = usize::from(columns).max(1);
    let mut cells: Vec<String> = items
        .into_iter()
        .map(|eq| {
            if eq.is_empty() && placeholders {
                "\\square".to_owned()
            } else {
                render_eq(eq)
            }
        })
        .collect();
    let padding = (columns - cells.len() % columns) % columns;
    let pad = if placeholders { "\\square" } else { "" };
    // The last row's padding is empty cells, which show a placeholder as any empty cell does.
    cells.extend(std::iter::repeat_n(pad.to_owned(), padding));
    let rows = cells
        .chunks(columns)
        .map(|row| row.join(" & "))
        .collect::<Vec<_>>()
        .join(" \\\\ ");
    format!("\\begin{{{env}}} {rows} \\end{{{env}}}")
}

fn render_nary(
    op: char,
    sub: Equation,
    sup: Equation,
    body: Equation,
    display: NAryDisplay,
) -> String {
    let (sub_empty, sup_empty) = (sub.is_empty(), sup.is_empty());
    let mut sub_content = render_eq(sub);
    let mut sup_content = render_eq(sup);
    if display.lower_placeholder && sub_empty {
        sub_content = "\\square".to_owned();
    }
    if display.upper_placeholder && sup_empty {
        sup_content = "\\square".to_owned();
    }
    if display.limits_opposite {
        std::mem::swap(&mut sub_content, &mut sup_content);
    }
    let limits = match display.limits {
        NAryLimits::SubSup | NAryLimits::UpperAsSuperscript => "\\nolimits",
        NAryLimits::UnderOver => "\\limits",
        NAryLimits::Default => "",
    };
    let sub_part = if sub_content.is_empty() {
        String::new()
    } else {
        format!("_{{{sub_content}}}")
    };
    let sup_part = if sup_content.is_empty() {
        String::new()
    } else {
        format!("^{{{sup_content}}}")
    };
    format!(
        "{}{limits}{sub_part}{sup_part} {}",
        nary_op_command(op),
        render_group(body)
    )
}

fn nary_op_command(c: char) -> String {
    let command = match c {
        '∑' => "\\sum",
        '∏' => "\\prod",
        '∐' => "\\coprod",
        '∫' => "\\int",
        '∬' => "\\iint",
        '∭' => "\\iiint",
        '⨌' => "\\iiiint",
        '∮' => "\\oint",
        '∯' => "\\oiint",
        '∰' => "\\oiiint",
        '⋃' => "\\bigcup",
        '⋂' => "\\bigcap",
        '⨁' => "\\bigoplus",
        '⨂' => "\\bigotimes",
        '⨄' => "\\biguplus",
        '⨆' => "\\bigsqcup",
        '⋁' => "\\bigvee",
        '⋀' => "\\bigwedge",
        _ => return c.to_string(),
    };
    command.to_owned()
}

fn render_phantom(kind: PhantomKind, display: u8, body: Equation) -> String {
    let mut show = matches!(
        kind,
        PhantomKind::AscentSmash
            | PhantomKind::DescentSmash
            | PhantomKind::HorizontalSmash
            | PhantomKind::VerticalSmash
    );
    show |= display & PHANTOM_SHOW != 0;
    let transparent = display & PHANTOM_TRANSPARENT != 0;
    let mut zero_width = display & PHANTOM_ZERO_WIDTH != 0;
    let mut zero_ascent = display & PHANTOM_ZERO_ASCENT != 0;
    let mut zero_descent = display & PHANTOM_ZERO_DESCENT != 0;
    match kind {
        PhantomKind::FullOrCustom => {}
        PhantomKind::HorizontalPhantom | PhantomKind::VerticalSmash => {
            zero_ascent = true;
            zero_descent = true;
        }
        PhantomKind::VerticalPhantom | PhantomKind::HorizontalSmash => zero_width = true,
        PhantomKind::AscentSmash => zero_ascent = true,
        PhantomKind::DescentSmash => zero_descent = true,
    }

    let body = render_group(body);
    if !show {
        if zero_ascent && zero_descent && !zero_width {
            return format!("\\hphantom{{{body}}}");
        }
        if zero_width && !zero_ascent && !zero_descent {
            return format!("\\vphantom{{{body}}}");
        }
        return format!("\\phantom{{{body}}}");
    }
    let content = if zero_ascent && zero_descent {
        format!("\\smash{{{body}}}")
    } else if zero_ascent {
        format!("\\smash[t]{{{body}}}")
    } else if zero_descent {
        format!("\\smash[b]{{{body}}}")
    } else {
        body
    };
    if transparent {
        format!("{{\\color{{transparent}}{content}}}")
    } else {
        content
    }
}

fn render_stretch_stack(char: char, pos: StretchStackPosition, body: Equation) -> String {
    let body = render_group(body);
    let command = match char {
        '\u{23DE}' | '\u{FE37}' => Some("\\overbrace"),
        '\u{23DF}' | '\u{FE38}' => Some("\\underbrace"),
        '\u{23B4}' => Some("\\overbracket"),
        '\u{23B5}' => Some("\\underbracket"),
        // Stretchy arrows.
        '→' => Some("\\xrightarrow"),
        '←' => Some("\\xleftarrow"),
        '↔' => Some("\\xleftrightarrow"),
        '⇒' => Some("\\xRightarrow"),
        '⇐' => Some("\\xLeftarrow"),
        '⇔' => Some("\\xLeftrightarrow"),
        _ => None,
    };
    if let Some(command) = command {
        return format!("{command}{{{body}}}");
    }
    let setter = match pos {
        StretchStackPosition::CharBelow | StretchStackPosition::BaseAbove => "\\underset",
        StretchStackPosition::CharAbove | StretchStackPosition::BaseBelow => "\\overset",
    };
    format!("{setter}{{{char}}}{{{body}}}")
}
