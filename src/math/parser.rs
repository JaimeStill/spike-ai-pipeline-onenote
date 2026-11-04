// Ported from one2html 2.0.1, src/page/math/parser.rs (https://github.com/msiemens/one2html),
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
// Changed from one2html: errors are plain strings; an object's alignment value is read only where
// the LaTeX uses it, so one the LaTeX ignores no longer fails the equation; an op-char is text
// without a log warning; a matrix of no columns fails rather than panics in the renderer.

//! The parser: tokens to an equation. Each math object reads its fixed number of arguments,
//! separated and closed by markers of its own type; an object left open, a stray separator or
//! end, or an argument count the object doesn't take fails the equation.

use onenote_parser::contents::MathObjectType;

use super::Object;
use super::ast::{
    BoxSize, BracketSize, Equation, MatrixBrackets, NAryDisplay, Op, PhantomKind,
    StretchStackPosition,
};
use super::lexer::{Lexer, Token};

type Result<T> = std::result::Result<T, String>;

pub(super) struct Parser {
    lexer: Lexer,
    cur: Token,
}

impl Parser {
    pub(super) fn new(runs: Vec<(String, Object)>) -> Parser {
        let mut lexer = Lexer::new(runs);
        let cur = lexer.next();
        Parser { lexer, cur }
    }

    pub(super) fn parse(&mut self) -> Result<Equation> {
        let mut equation = Vec::new();
        while self.cur != Token::Eof {
            equation.push(self.parse_op()?);
        }
        Ok(equation)
    }

    fn parse_op(&mut self) -> Result<Op> {
        match &self.cur {
            Token::Text(_) => self.parse_text(),
            Token::Start(object) => match object.kind {
                MathObjectType::Accent => self.parse_accent(),
                MathObjectType::Box => self.parse_box(),
                MathObjectType::BoxedFormula => self.parse_boxed_formula(),
                MathObjectType::Brackets => self.parse_brackets(),
                MathObjectType::BracketsWithSeps => self.parse_brackets_with_seps(),
                MathObjectType::EquationArray => self.parse_equation_array(),
                MathObjectType::Fraction => self.parse_fraction(),
                MathObjectType::FunctionApply => self.parse_function_apply(),
                MathObjectType::LeftSubSup => self.parse_left_sub_sup(),
                MathObjectType::LowerLimit => self.parse_lower_limit(),
                MathObjectType::Matrix => self.parse_matrix(),
                MathObjectType::Nary => self.parse_nary(),
                MathObjectType::OpChar => self.parse_op_char(),
                MathObjectType::Overbar => self.parse_over_bar(),
                MathObjectType::Phantom => self.parse_phantom(),
                MathObjectType::Radical => self.parse_radical(),
                MathObjectType::SlashedFraction => self.parse_slashed_fraction(),
                MathObjectType::Stack => self.parse_stack(),
                MathObjectType::StretchStack => self.parse_stretch_stack(),
                MathObjectType::Subscript => self.parse_subscript(),
                MathObjectType::SubSup => self.parse_sub_sup(),
                MathObjectType::Superscript => self.parse_superscript(),
                MathObjectType::Underbar => self.parse_under_bar(),
                MathObjectType::UpperLimit => self.parse_upper_limit(),
                kind @ (MathObjectType::SimpleText | MathObjectType::PlainText) => Err(format!(
                    "a math object starts with no object type ({kind:?})"
                )),
            },
            Token::Sep(kind) => Err(format!("a {kind:?} separator outside its object")),
            Token::End(kind) => Err(format!("a {kind:?} end outside its object")),
            Token::Eof => Err("the equation ends inside a math object".to_owned()),
        }
    }

    fn parse_text(&mut self) -> Result<Op> {
        match self.bump() {
            Token::Text(text) => Ok(Op::Text(text)),
            token => Err(format!("{} where text was expected", describe(&token))),
        }
    }

    fn parse_accent(&mut self) -> Result<Op> {
        let (object, body) = self.parse_object_1(MathObjectType::Accent)?;
        let char = object.char.ok_or("an accent has no accent character")?;
        Ok(Op::Accent { char, body })
    }

    fn parse_box(&mut self) -> Result<Op> {
        let (object, body) = self.parse_object_1(MathObjectType::Box)?;
        let value = object.align.unwrap_or(0);
        let size = match value & 0x60 {
            32 => BoxSize::Script,
            64 => BoxSize::ScriptScript,
            _ => BoxSize::Text,
        };
        Ok(Op::Box {
            body,
            size,
            no_break: value & 0x80 != 0,
        })
    }

    fn parse_boxed_formula(&mut self) -> Result<Op> {
        let (_, body) = self.parse_object_1(MathObjectType::BoxedFormula)?;
        Ok(Op::BoxedFormula { body })
    }

    fn parse_brackets(&mut self) -> Result<Op> {
        let (object, body) = self.parse_object_1(MathObjectType::Brackets)?;
        Ok(Op::Brackets {
            open: object.char,
            close: object.char1,
            body,
            size: object.align.and_then(BracketSize::from_align),
        })
    }

    fn parse_brackets_with_seps(&mut self) -> Result<Op> {
        let (object, segments) = self.parse_object_n(MathObjectType::BracketsWithSeps)?;
        let sep = object
            .char2
            .ok_or("brackets with separators have no separator character")?;
        Ok(Op::BracketsWithSeps {
            open: object.char,
            close: object.char1,
            sep,
            segments,
            size: object.align.and_then(BracketSize::from_align),
        })
    }

    fn parse_equation_array(&mut self) -> Result<Op> {
        let (_, rows) = self.parse_object_n(MathObjectType::EquationArray)?;
        Ok(Op::EquationArray { rows })
    }

    fn parse_fraction(&mut self) -> Result<Op> {
        let (object, num, den) = self.parse_object_2(MathObjectType::Fraction)?;
        let small = object.char == Some('\u{2298}');
        Ok(Op::Fraction { num, den, small })
    }

    fn parse_function_apply(&mut self) -> Result<Op> {
        let (_, func, body) = self.parse_object_2(MathObjectType::FunctionApply)?;
        Ok(Op::FunctionApply { func, body })
    }

    fn parse_left_sub_sup(&mut self) -> Result<Op> {
        let (_, sub, sup, body) = self.parse_object_3(MathObjectType::LeftSubSup)?;
        Ok(Op::LeftSubSup { sub, sup, body })
    }

    fn parse_lower_limit(&mut self) -> Result<Op> {
        let (_, body, limit) = self.parse_object_2(MathObjectType::LowerLimit)?;
        Ok(Op::LowerLimit { body, limit })
    }

    fn parse_matrix(&mut self) -> Result<Op> {
        let (object, items) = self.parse_object_n(MathObjectType::Matrix)?;
        let columns = object
            .column
            .filter(|&columns| columns > 0)
            .ok_or("a matrix has no columns")?;
        let brackets = match object.char.ok_or("a matrix has no brackets character")? {
            '\u{25a0}' => None,
            '\u{24a8}' => Some(MatrixBrackets::Parentheses),
            '\u{24b1}' => Some(MatrixBrackets::VerticalBars),
            '\u{24a9}' => Some(MatrixBrackets::DoubleVerticalBars),
            c => return Err(format!("a matrix's brackets character {c:?} is unknown")),
        };
        Ok(Op::Matrix {
            columns,
            brackets,
            items,
            // MatrixAlignment's ShowMatPlaceHldr: empty cells show a placeholder.
            placeholders: object.align == Some(8),
        })
    }

    fn parse_nary(&mut self) -> Result<Op> {
        let (object, sub, sup, body) = self.parse_object_3(MathObjectType::Nary)?;
        let op = object.char.ok_or("an n-ary has no operator character")?;
        Ok(Op::NAry {
            op,
            sub,
            sup,
            body,
            display: object
                .align
                .map(NAryDisplay::from_align)
                .unwrap_or_default(),
        })
    }

    fn parse_op_char(&mut self) -> Result<Op> {
        let object = self.bump_start(MathObjectType::OpChar)?;
        expect_args(&object, 0)?;
        self.bump_end(MathObjectType::OpChar)?;
        Ok(Op::Text(object.char.map(String::from).unwrap_or_default()))
    }

    fn parse_over_bar(&mut self) -> Result<Op> {
        let (_, body) = self.parse_object_1(MathObjectType::Overbar)?;
        Ok(Op::OverBar { body })
    }

    fn parse_phantom(&mut self) -> Result<Op> {
        let (object, body) = self.parse_object_1(MathObjectType::Phantom)?;
        let kind = match object.char.ok_or("a phantom has no kind character")? {
            '\u{27e1}' => PhantomKind::FullOrCustom,
            '\u{2b04}' => PhantomKind::HorizontalPhantom,
            '\u{21f3}' => PhantomKind::VerticalPhantom,
            '\u{2b06}' => PhantomKind::AscentSmash,
            '\u{2b07}' => PhantomKind::DescentSmash,
            '\u{2b0c}' => PhantomKind::HorizontalSmash,
            '\u{2b0d}' => PhantomKind::VerticalSmash,
            c => return Err(format!("a phantom's kind character {c:?} is unknown")),
        };
        Ok(Op::Phantom {
            body,
            kind,
            display: object.align.unwrap_or(0),
        })
    }

    fn parse_radical(&mut self) -> Result<Op> {
        let (_, degree, body) = self.parse_object_2(MathObjectType::Radical)?;
        Ok(Op::Radical { body, degree })
    }

    fn parse_slashed_fraction(&mut self) -> Result<Op> {
        let (object, num, den) = self.parse_object_2(MathObjectType::SlashedFraction)?;
        let linear = object.char == Some('\u{2215}');
        Ok(Op::SlashedFraction { num, den, linear })
    }

    fn parse_stack(&mut self) -> Result<Op> {
        let (_, num, den) = self.parse_object_2(MathObjectType::Stack)?;
        Ok(Op::Stack { num, den })
    }

    fn parse_stretch_stack(&mut self) -> Result<Op> {
        let (object, body) = self.parse_object_1(MathObjectType::StretchStack)?;
        let char = object.char.ok_or("a stretch stack has no character")?;
        let pos = match object.align.unwrap_or(0) {
            0 => StretchStackPosition::CharBelow,
            1 => StretchStackPosition::CharAbove,
            2 => StretchStackPosition::BaseBelow,
            3 => StretchStackPosition::BaseAbove,
            value => return Err(format!("a stretch stack's position {value} is unknown")),
        };
        Ok(Op::StretchStack { char, body, pos })
    }

    fn parse_subscript(&mut self) -> Result<Op> {
        let (_, body, sub) = self.parse_object_2(MathObjectType::Subscript)?;
        Ok(Op::Subscript { sub, body })
    }

    fn parse_sub_sup(&mut self) -> Result<Op> {
        let (_, body, sub, sup) = self.parse_object_3(MathObjectType::SubSup)?;
        Ok(Op::SubSup { sub, sup, body })
    }

    fn parse_superscript(&mut self) -> Result<Op> {
        let (_, body, sup) = self.parse_object_2(MathObjectType::Superscript)?;
        Ok(Op::Superscript { sup, body })
    }

    fn parse_under_bar(&mut self) -> Result<Op> {
        let (_, body) = self.parse_object_1(MathObjectType::Underbar)?;
        Ok(Op::UnderBar { body })
    }

    fn parse_upper_limit(&mut self) -> Result<Op> {
        let (_, body, limit) = self.parse_object_2(MathObjectType::UpperLimit)?;
        Ok(Op::UpperLimit { body, limit })
    }

    // Helpers

    fn parse_object_1(&mut self, kind: MathObjectType) -> Result<(Object, Equation)> {
        let (object, mut args) = self.parse_object_fixed(kind, 1)?;
        let arg = args.remove(0);
        Ok((object, arg))
    }

    fn parse_object_2(&mut self, kind: MathObjectType) -> Result<(Object, Equation, Equation)> {
        let (object, args) = self.parse_object_fixed(kind, 2)?;
        let [a, b] = <[Equation; 2]>::try_from(args).expect("two arguments");
        Ok((object, a, b))
    }

    fn parse_object_3(
        &mut self,
        kind: MathObjectType,
    ) -> Result<(Object, Equation, Equation, Equation)> {
        let (object, args) = self.parse_object_fixed(kind, 3)?;
        let [a, b, c] = <[Equation; 3]>::try_from(args).expect("three arguments");
        Ok((object, a, b, c))
    }

    /// An object that takes exactly `count` arguments.
    fn parse_object_fixed(
        &mut self,
        kind: MathObjectType,
        count: u32,
    ) -> Result<(Object, Vec<Equation>)> {
        let object = self.bump_start(kind)?;
        expect_args(&object, count)?;
        let args = self.parse_args(kind, count)?;
        Ok((object, args))
    }

    /// An object that takes as many arguments as it says.
    fn parse_object_n(&mut self, kind: MathObjectType) -> Result<(Object, Vec<Equation>)> {
        let object = self.bump_start(kind)?;
        let args = self.parse_args(kind, object.args)?;
        Ok((object, args))
    }

    /// `count` arguments, separated by the object's separators and closed by its end. An
    /// object of no arguments is its start and its end, an end one2html leaves unread.
    fn parse_args(&mut self, kind: MathObjectType, count: u32) -> Result<Vec<Equation>> {
        let mut args = Vec::new();
        if count == 0 {
            self.bump_end(kind)?;
        }
        for index in 0..count {
            args.push(self.parse_arg(kind)?);
            if index + 1 < count {
                self.bump_sep(kind)?;
            } else {
                self.bump_end(kind)?;
            }
        }
        Ok(args)
    }

    fn parse_arg(&mut self, kind: MathObjectType) -> Result<Equation> {
        let mut equation = Vec::new();
        while self.cur != Token::Sep(kind) && self.cur != Token::End(kind) {
            equation.push(self.parse_op()?);
        }
        Ok(equation)
    }

    fn bump(&mut self) -> Token {
        std::mem::replace(&mut self.cur, self.lexer.next())
    }

    fn bump_start(&mut self, kind: MathObjectType) -> Result<Object> {
        match self.bump() {
            Token::Start(object) if object.kind == kind => Ok(object),
            token => Err(format!(
                "{} where a {kind:?} start was expected",
                describe(&token)
            )),
        }
    }

    fn bump_sep(&mut self, kind: MathObjectType) -> Result<()> {
        match self.bump() {
            Token::Sep(found) if found == kind => Ok(()),
            token => Err(format!(
                "{} where a {kind:?} separator was expected",
                describe(&token)
            )),
        }
    }

    fn bump_end(&mut self, kind: MathObjectType) -> Result<()> {
        match self.bump() {
            Token::End(found) if found == kind => Ok(()),
            token => Err(format!(
                "{} where a {kind:?} end was expected",
                describe(&token)
            )),
        }
    }
}

fn expect_args(object: &Object, count: u32) -> Result<()> {
    if object.args == count {
        Ok(())
    } else {
        Err(format!(
            "a {:?} of {} arguments, where it takes {count}",
            object.kind, object.args
        ))
    }
}

/// A token, as an error names it.
fn describe(token: &Token) -> String {
    match token {
        Token::Text(_) => "text".to_owned(),
        Token::Start(object) => format!("a {:?} start", object.kind),
        Token::Sep(kind) => format!("a {kind:?} separator"),
        Token::End(kind) => format!("a {kind:?} end"),
        Token::Eof => "the equation's end".to_owned(),
    }
}
