// Ported from one2html 2.0.1, src/page/math/lexer.rs (https://github.com/msiemens/one2html),
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
// Changed from one2html: a run is split at every marker in it, not only at a leading start or
// a trailing separator or end, and its tokens are built up front.

//! The lexer: an equation's runs as tokens. Each marker is a token carrying its run's object;
//! the text between markers is a text token.

use onenote_parser::contents::MathObjectType;

use super::Object;

// See https://learn.microsoft.com/en-us/archive/blogs/murrays/officemath for these markers.
const START: char = '\u{fdd0}';
const SEP: char = '\u{fdee}';
const END: char = '\u{fdef}';

/// Whether `c` is one of RichEdit's math object markers.
pub(super) fn is_marker(c: char) -> bool {
    matches!(c, START | SEP | END)
}

#[derive(PartialEq, Debug)]
pub(super) enum Token {
    Text(String),
    Start(Object),
    Sep(MathObjectType),
    End(MathObjectType),
    Eof,
}

pub(super) struct Lexer {
    tokens: std::vec::IntoIter<Token>,
}

impl Lexer {
    pub(super) fn new(runs: Vec<(String, Object)>) -> Lexer {
        let mut tokens = Vec::new();
        for (text, object) in runs {
            let mut plain = String::new();
            for c in text.chars() {
                let marker = match c {
                    START => Token::Start(object),
                    SEP => Token::Sep(object.kind),
                    END => Token::End(object.kind),
                    _ => {
                        plain.push(c);
                        continue;
                    }
                };
                if !plain.is_empty() {
                    tokens.push(Token::Text(std::mem::take(&mut plain)));
                }
                tokens.push(marker);
            }
            if !plain.is_empty() {
                tokens.push(Token::Text(plain));
            }
        }
        Lexer {
            tokens: tokens.into_iter(),
        }
    }

    pub(super) fn next(&mut self) -> Token {
        self.tokens.next().unwrap_or(Token::Eof)
    }
}
