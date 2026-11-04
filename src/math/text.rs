// Ported from one2html 2.0.1, src/page/math/text.rs (https://github.com/msiemens/one2html),
// under its MIT licence:
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
// Changed from one2html: the ranges are a list, not a phf map, since none overlaps; the Unicode
// blocks are their ranges, not unicode-blocks'; the sans bold italic Greek range ends at
// U+1D7C9, where one2html's U+17C9 left it empty; an unknown character is an identifier without
// a log warning; and the classification can't fail.

//! Math text's characters by kind, which decides how the renderer writes each run of them.

use finl_unicode::categories::CharacterCategories;

/// The Mathematical Alphanumeric Symbols' styled ranges, and the operator and arrow blocks.
const RANGES: [(char, char, TextType); 20] = [
    ('\u{1d400}', '\u{1d433}', TextType::Bold),
    ('\u{1d434}', '\u{1d467}', TextType::Identifier),
    ('\u{1d468}', '\u{1d49b}', TextType::BoldItalic),
    ('\u{1d49c}', '\u{1d4cf}', TextType::Script),
    ('\u{1d4d0}', '\u{1d503}', TextType::BoldScript),
    ('\u{1d504}', '\u{1d537}', TextType::Fraktur),
    ('\u{1d538}', '\u{1d56b}', TextType::Double),
    ('\u{1d56c}', '\u{1d59f}', TextType::FrakturBold),
    ('\u{1d5a0}', '\u{1d5d3}', TextType::Sans),
    ('\u{1d5d4}', '\u{1d607}', TextType::SansBold),
    ('\u{1d608}', '\u{1d63b}', TextType::SansItalic),
    ('\u{1d63c}', '\u{1d66f}', TextType::SansBoldItalic),
    ('\u{1d670}', '\u{1d6a3}', TextType::Mono),
    ('\u{1d6a8}', '\u{1d6e1}', TextType::Bold),
    ('\u{1d6e2}', '\u{1d71b}', TextType::Identifier),
    ('\u{1d71c}', '\u{1d755}', TextType::BoldItalic),
    ('\u{1d756}', '\u{1d78f}', TextType::SansBold),
    ('\u{1d790}', '\u{1d7c9}', TextType::SansBoldItalic),
    ('\u{2200}', '\u{22ff}', TextType::Operator),
    ('\u{2190}', '\u{21ff}', TextType::Operator),
];

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub(super) enum TextType {
    Bold,
    BoldItalic,
    BoldScript,
    Double,
    DoubleOperator,
    Fraktur,
    FrakturBold,
    Identifier,
    Mono,
    Normal,
    Numeric,
    Operator,
    Raw,
    Sans,
    SansBold,
    SansBoldItalic,
    SansItalic,
    Script,
    Space,
}

impl TextType {
    pub(super) fn from_char(c: char) -> TextType {
        if c == '&' {
            return TextType::Raw;
        }
        if c.is_ascii_punctuation() {
            return TextType::Operator;
        }
        if c.is_ascii_alphabetic() {
            return TextType::Normal;
        }
        if c.is_separator_space() {
            return TextType::Space;
        }
        if c.is_numeric() {
            return TextType::Numeric;
        }
        if c.is_symbol_math() {
            return TextType::Operator;
        }
        if c.is_format() {
            if c == '\u{2061}' {
                return TextType::Operator;
            }
            return TextType::Raw;
        }
        if c == 'ⅅ' || c == 'ⅆ' {
            return TextType::DoubleOperator;
        }
        if let Some(&(_, _, kind)) = RANGES
            .iter()
            .find(|(start, end, _)| (*start..=*end).contains(&c))
        {
            return kind;
        }
        match c as u32 {
            // Greek and Coptic, Letterlike Symbols, Mathematical Alphanumeric Symbols.
            0x370..=0x3ff | 0x2100..=0x214f | 0x1d400..=0x1d7ff => TextType::Identifier,
            // Latin-1 Supplement.
            0x80..=0xff => TextType::Normal,
            // General Punctuation.
            0x2000..=0x206f => TextType::Operator,
            _ => TextType::Identifier,
        }
    }
}
