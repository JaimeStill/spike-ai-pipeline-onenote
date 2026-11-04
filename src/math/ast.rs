// Ported from one2html 2.0.1, src/page/math/ast.rs (https://github.com/msiemens/one2html),
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
// Changed from one2html: only what the LaTeX renderer reads is kept. Alignment bits it never
// reads (a box's alignment and spacing, a boxed formula's, an equation array's, a sub-sup's) are
// left out, and the bitflags and primitive enums are plain fields.

//! The parsed equation: a list of operations, each math object with its arguments.

pub(super) type Equation = Vec<Op>;

#[derive(Debug)]
pub(super) enum Op {
    Text(String),
    Accent {
        char: char,
        body: Equation,
    },
    Box {
        body: Equation,
        size: BoxSize,
        no_break: bool,
    },
    BoxedFormula {
        body: Equation,
    },
    Brackets {
        open: Option<char>,
        close: Option<char>,
        body: Equation,
        size: Option<BracketSize>,
    },
    BracketsWithSeps {
        open: Option<char>,
        close: Option<char>,
        sep: char,
        segments: Vec<Equation>,
        size: Option<BracketSize>,
    },
    EquationArray {
        rows: Vec<Equation>,
    },
    Fraction {
        num: Equation,
        den: Equation,
        small: bool,
    },
    FunctionApply {
        func: Equation,
        body: Equation,
    },
    LeftSubSup {
        sub: Equation,
        sup: Equation,
        body: Equation,
    },
    LowerLimit {
        body: Equation,
        limit: Equation,
    },
    Matrix {
        columns: u8,
        brackets: Option<MatrixBrackets>,
        items: Vec<Equation>,
        placeholders: bool,
    },
    NAry {
        op: char,
        sub: Equation,
        sup: Equation,
        body: Equation,
        display: NAryDisplay,
    },
    OverBar {
        body: Equation,
    },
    Phantom {
        body: Equation,
        kind: PhantomKind,
        display: u8,
    },
    Radical {
        body: Equation,
        degree: Equation,
    },
    SlashedFraction {
        num: Equation,
        den: Equation,
        linear: bool,
    },
    Stack {
        num: Equation,
        den: Equation,
    },
    StretchStack {
        char: char,
        body: Equation,
        pos: StretchStackPosition,
    },
    Subscript {
        sub: Equation,
        body: Equation,
    },
    SubSup {
        sub: Equation,
        sup: Equation,
        body: Equation,
    },
    Superscript {
        sup: Equation,
        body: Equation,
    },
    UnderBar {
        body: Equation,
    },
    UpperLimit {
        body: Equation,
        limit: Equation,
    },
}

/// A box's size: its alignment value's bits 5 and 6.
#[derive(Debug, Copy, Clone, PartialEq)]
pub(super) enum BoxSize {
    Text,
    Script,
    ScriptScript,
}

/// A bracket's size, from its alignment value; brackets with none grow with their content.
#[derive(Debug, Copy, Clone, PartialEq)]
pub(super) enum BracketSize {
    DontGrow,
    TeXbig,
    TeXBig,
    TeXbigg,
    TeXBigg,
}

impl BracketSize {
    pub(super) fn from_align(value: u8) -> Option<BracketSize> {
        Some(match value {
            64 => BracketSize::DontGrow,
            32 => BracketSize::TeXbig,
            96 => BracketSize::TeXBig,
            160 => BracketSize::TeXbigg,
            224 => BracketSize::TeXBigg,
            _ => return None,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub(super) enum MatrixBrackets {
    Parentheses,
    VerticalBars,
    DoubleVerticalBars,
}

/// An n-ary's alignment value: where its limits go, and its options. An n-ary with no value
/// has the default limits and no options.
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub(super) struct NAryDisplay {
    pub(super) limits: NAryLimits,
    pub(super) limits_opposite: bool,
    pub(super) lower_placeholder: bool,
    pub(super) upper_placeholder: bool,
}

#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub(super) enum NAryLimits {
    #[default]
    Default,
    UnderOver,
    SubSup,
    UpperAsSuperscript,
}

impl NAryDisplay {
    pub(super) fn from_align(value: u8) -> NAryDisplay {
        NAryDisplay {
            limits: match value & 0x3 {
                0 => NAryLimits::Default,
                1 => NAryLimits::UnderOver,
                2 => NAryLimits::SubSup,
                _ => NAryLimits::UpperAsSuperscript,
            },
            limits_opposite: value & 4 != 0,
            lower_placeholder: value & 8 != 0,
            upper_placeholder: value & 16 != 0,
        }
    }
}

/// A phantom's display bits.
pub(super) const PHANTOM_SHOW: u8 = 1;
pub(super) const PHANTOM_ZERO_WIDTH: u8 = 2;
pub(super) const PHANTOM_ZERO_ASCENT: u8 = 4;
pub(super) const PHANTOM_ZERO_DESCENT: u8 = 8;
pub(super) const PHANTOM_TRANSPARENT: u8 = 16;

#[derive(Debug, Copy, Clone, PartialEq)]
pub(super) enum PhantomKind {
    FullOrCustom,
    HorizontalPhantom,
    VerticalPhantom,
    AscentSmash,
    DescentSmash,
    HorizontalSmash,
    VerticalSmash,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub(super) enum StretchStackPosition {
    CharBelow,
    CharAbove,
    BaseBelow,
    BaseAbove,
}
