//! Equations as LaTeX. OneNote stores an equation as a paragraph's math-formatted runs: text in
//! RichEdit's linear format, where U+FDD0 starts a math object, U+FDEE separates its arguments
//! and U+FDEF ends it, each run carrying the object, by tom.h's OBJECTTYPE, it belongs to. The
//! lexer, parser and LaTeX renderer below are a port of one2html's, MIT-licensed; each ported
//! file keeps one2html's notice. Only its LaTeX path is ported, not its MathML or HTML.
//!
//! The LaTeX is bare, without `\(…\)` or `$…$` around it, and keeps Unicode symbols (α, ≤, ∂)
//! as one2html does, for a Unicode-aware backend such as MathJax, KaTeX or unicode-math.

mod ast;
mod latex;
mod lexer;
mod parser;
mod text;

use onenote_parser::contents::{MathInlineObject, MathObjectType};

/// A run's math object: onenote_parser's MathInlineObject, whose fields can only be read, held
/// so the tests can build one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Object {
    pub kind: MathObjectType,
    /// The object's argument count, on the run that starts it.
    pub args: u32,
    pub column: Option<u8>,
    pub align: Option<u8>,
    pub char: Option<char>,
    pub char1: Option<char>,
    pub char2: Option<char>,
}

impl From<&MathInlineObject> for Object {
    fn from(object: &MathInlineObject) -> Self {
        Object {
            kind: object.object_type(),
            args: object.arg_count(),
            column: object.column(),
            align: object.align(),
            char: object.char(),
            char1: object.char1(),
            char2: object.char2(),
        }
    }
}

/// One equation's LaTeX, from its runs in order, each with its text and its object; or why
/// its markers don't parse: an object left open, an argument count its markers don't match.
pub fn latex(runs: Vec<(String, Object)>) -> Result<String, String> {
    let equation = parser::Parser::new(runs).parse()?;
    Ok(latex::render(equation))
}

/// An equation whose markers don't parse, as LaTeX of its text alone: the markers dropped,
/// the characters rendered as one2html renders plain math text.
pub fn latex_of_text(text: &str) -> String {
    let text: String = text.chars().filter(|c| !lexer::is_marker(*c)).collect();
    latex::render(vec![ast::Op::Text(text)])
}

/// Each tom.h object type the port reads, its arguments as OneNote stores them, and the LaTeX
/// it gives, written by hand from one2html's rendering rules.
#[cfg(test)]
mod tests {
    use super::*;
    use MathObjectType as T;

    type Runs = Vec<(String, Object)>;

    const START: &str = "\u{fdd0}";
    const SEP: &str = "\u{fdee}";
    const END: &str = "\u{fdef}";

    fn of(kind: MathObjectType) -> Object {
        Object {
            kind,
            ..Object::default()
        }
    }

    fn run(text: &str, object: Object) -> (String, Object) {
        (text.to_owned(), object)
    }

    /// Math text, in a run of its own.
    fn text(text: &str) -> Runs {
        vec![run(text, of(T::PlainText))]
    }

    /// An object as OneNote stores it: a run of its start, which carries the object and its
    /// argument count, its arguments separated by runs of its separator, and a run of its end.
    fn object(start: Object, args: Vec<Runs>) -> Runs {
        let kind = start.kind;
        let start = Object {
            args: args.len() as u32,
            ..start
        };
        let mut runs = vec![run(START, start)];
        for (index, arg) in args.into_iter().enumerate() {
            if index > 0 {
                runs.push(run(SEP, of(kind)));
            }
            runs.extend(arg);
        }
        runs.push(run(END, of(kind)));
        runs
    }

    fn with_char(kind: MathObjectType, c: char) -> Object {
        Object {
            char: Some(c),
            ..of(kind)
        }
    }

    fn with_align(object: Object, align: u8) -> Object {
        Object {
            align: Some(align),
            ..object
        }
    }

    fn ok(runs: Runs) -> String {
        latex(runs).unwrap()
    }

    /// The runs of a²+b², as onenote.rs's Math.one stores them: each marker at the start or
    /// end of a run's text, every run carrying its object.
    #[test]
    fn markers_glued_to_text_split_off() {
        let sup = of(T::Superscript);
        let runs = vec![
            run(START, Object { args: 2, ..sup }),
            run("𝑎\u{fdee}", sup),
            run("2\u{fdef}", Object { args: 1, ..sup }),
            run("+", of(T::PlainText)),
            run(START, Object { args: 2, ..sup }),
            run("𝑏\u{fdee}", sup),
            run("2\u{fdef}", sup),
        ];
        assert_eq!(ok(runs), "a^{2}+b^{2}");
    }

    /// One run holding a whole object, markers inside its text, which one2html's lexer would
    /// leave as text.
    #[test]
    fn markers_inside_a_run_split_off() {
        let sup = Object {
            args: 2,
            ..of(T::Superscript)
        };
        assert_eq!(ok(vec![run("\u{fdd0}𝑥\u{fdee}2\u{fdef}", sup)]), "x^{2}");
    }

    #[test]
    fn accent() {
        let hat = with_char(T::Accent, '\u{0302}');
        assert_eq!(ok(object(hat, vec![text("𝑥")])), "\\hat{x}");
        let arrow = with_char(T::Accent, '→');
        assert_eq!(ok(object(arrow, vec![text("𝑥")])), "\\overset{→}{x}");
    }

    /// A box's size is its alignment's bits 5 and 6, its no-break bit 7.
    #[test]
    fn box_() {
        assert_eq!(ok(object(of(T::Box), vec![text("𝑥")])), "x");
        let small = with_align(of(T::Box), 64);
        assert_eq!(
            ok(object(small, vec![text("𝑥")])),
            "{\\scriptscriptstyle x}"
        );
        let script = with_align(of(T::Box), 32 | 128);
        assert_eq!(
            ok(object(script, vec![text("𝑥")])),
            "\\nobreak {\\scriptstyle x}"
        );
    }

    #[test]
    fn boxed_formula() {
        assert_eq!(
            ok(object(of(T::BoxedFormula), vec![text("𝑥+1")])),
            "\\boxed{x+1}"
        );
    }

    /// Brackets grow with their content unless a size is stored; a side with no character is
    /// LaTeX's empty delimiter.
    #[test]
    fn brackets() {
        let parens = Object {
            char: Some('('),
            char1: Some(')'),
            ..of(T::Brackets)
        };
        assert_eq!(ok(object(parens, vec![text("𝑥")])), "\\left( x \\right)");
        assert_eq!(
            ok(object(with_align(parens, 32), vec![text("𝑥")])),
            "\\bigl( x \\bigr)"
        );
        let brace = with_char(T::Brackets, '{');
        assert_eq!(ok(object(brace, vec![text("𝑥")])), "\\left\\{ x \\right.");
    }

    #[test]
    fn brackets_with_separators() {
        let bra_ket = Object {
            char: Some('⟨'),
            char1: Some('⟩'),
            char2: Some('|'),
            ..of(T::BracketsWithSeps)
        };
        assert_eq!(
            // U+1D711 is math italic phi, φ; its variant, 𝜙, would be ϕ.
            ok(object(bra_ket, vec![text("𝜓"), text("\u{1d711}")])),
            "\\left\\langle  ψ \\,\\middle|\\, φ \\right\\rangle "
        );
        let no_separator = of(T::BracketsWithSeps);
        assert!(latex(object(no_separator, vec![text("𝑥")])).is_err());
    }

    /// An equation array's rows, aligned at their `&`, which passes as LaTeX's own.
    #[test]
    fn equation_array() {
        assert_eq!(
            ok(object(
                of(T::EquationArray),
                vec![text("𝑥&=1"), text("𝑦&=2")]
            )),
            "\\begin{aligned} x&=1 \\\\ y&=2 \\end{aligned}"
        );
    }

    #[test]
    fn fraction() {
        assert_eq!(
            ok(object(of(T::Fraction), vec![text("𝑎"), text("𝑏")])),
            "\\frac{a}{b}"
        );
        let small = with_char(T::Fraction, '\u{2298}');
        assert_eq!(
            ok(object(small, vec![text("𝑎"), text("𝑏")])),
            "\\tfrac{a}{b}"
        );
    }

    /// Objects of one type nest: each reads its own separators and end.
    #[test]
    fn nested_objects_of_one_type() {
        let inner = object(of(T::Fraction), vec![text("𝑎"), text("𝑏")]);
        assert_eq!(
            ok(object(of(T::Fraction), vec![inner, text("𝑐")])),
            "\\frac{\\frac{a}{b}}{c}"
        );
    }

    #[test]
    fn function_apply() {
        assert_eq!(
            ok(object(of(T::FunctionApply), vec![text("sin"), text("𝑥")])),
            "\\operatorname{sin}x"
        );
    }

    #[test]
    fn left_sub_sup() {
        assert_eq!(
            ok(object(
                of(T::LeftSubSup),
                vec![text("1"), text("2"), text("𝑋")]
            )),
            "{}_{1}^{2}X"
        );
        assert_eq!(
            ok(object(
                of(T::LeftSubSup),
                vec![text("1"), Vec::new(), text("𝑋")]
            )),
            "{}_{1}^{}X"
        );
    }

    #[test]
    fn lower_limit() {
        assert_eq!(
            ok(object(of(T::LowerLimit), vec![text("lim"), text("𝑛→∞")])),
            "\\underset{n→∞}{\\operatorname{lim}}"
        );
    }

    /// A matrix's items in rows of its columns; its character is its brackets. With
    /// ShowMatPlaceHldr, an empty cell, and the last row's padding, shows a placeholder.
    #[test]
    fn matrix() {
        let pmatrix = Object {
            column: Some(2),
            ..with_char(T::Matrix, '\u{24a8}')
        };
        let items = vec![text("𝑎"), text("𝑏"), text("𝑐"), text("𝑑")];
        assert_eq!(
            ok(object(pmatrix, items)),
            "\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}"
        );
        let bare = Object {
            column: Some(2),
            ..with_align(with_char(T::Matrix, '\u{25a0}'), 8)
        };
        assert_eq!(
            ok(object(bare, vec![text("𝑎"), Vec::new(), text("𝑐")])),
            "\\begin{matrix} a & \\square \\\\ c & \\square \\end{matrix}"
        );
        let unknown = Object {
            column: Some(2),
            ..with_char(T::Matrix, 'x')
        };
        assert!(latex(object(unknown, vec![text("𝑎")])).is_err());
        let no_columns = with_char(T::Matrix, '\u{24a8}');
        assert!(latex(object(no_columns, vec![text("𝑎")])).is_err());
    }

    /// An n-ary's alignment: bits 0 and 1 place its limits, bit 2 swaps them, and bits 3 and
    /// 4 show a placeholder for an empty lower or upper limit.
    #[test]
    fn nary() {
        let args = || vec![text("𝑘=0"), text("𝑛"), text("𝑘")];
        let sum = with_char(T::Nary, '∑');
        assert_eq!(ok(object(sum, args())), "\\sum_{k=0}^{n} k");
        assert_eq!(
            ok(object(with_align(sum, 1), args())),
            "\\sum\\limits_{k=0}^{n} k"
        );
        assert_eq!(ok(object(with_align(sum, 4), args())), "\\sum_{n}^{k=0} k");
        let int = with_align(with_char(T::Nary, '∫'), 2 | 8);
        assert_eq!(
            ok(object(int, vec![Vec::new(), Vec::new(), text("𝑥")])),
            "\\int\\nolimits_{\\square} x"
        );
        let no_operator = of(T::Nary);
        assert!(latex(object(no_operator, args())).is_err());
    }

    #[test]
    fn op_char() {
        assert_eq!(ok(object(with_char(T::OpChar, '+'), Vec::new())), "+");
    }

    #[test]
    fn overbar_and_underbar() {
        assert_eq!(ok(object(of(T::Overbar), vec![text("𝑥")])), "\\overline{x}");
        assert_eq!(
            ok(object(of(T::Underbar), vec![text("𝑥")])),
            "\\underline{x}"
        );
    }

    /// A phantom's character is its kind; its alignment's bits show it or make it
    /// transparent.
    #[test]
    fn phantom() {
        let phantom = |c: char, align: Option<u8>| {
            let object = Object {
                align,
                ..with_char(T::Phantom, c)
            };
            ok(object_of_x(object))
        };
        assert_eq!(phantom('\u{27e1}', None), "\\phantom{x}");
        assert_eq!(phantom('\u{2b04}', None), "\\hphantom{x}");
        assert_eq!(phantom('\u{21f3}', None), "\\vphantom{x}");
        assert_eq!(phantom('\u{2b06}', None), "\\smash[t]{x}");
        assert_eq!(phantom('\u{2b07}', None), "\\smash[b]{x}");
        assert_eq!(phantom('\u{2b0d}', None), "\\smash{x}");
        assert_eq!(phantom('\u{27e1}', Some(1 | 16)), "{\\color{transparent}x}");
        assert!(latex(object_of_x(with_char(T::Phantom, 'x'))).is_err());
    }

    fn object_of_x(object: Object) -> Runs {
        self::object(object, vec![text("𝑥")])
    }

    /// A radical's arguments are its degree, then its body.
    #[test]
    fn radical() {
        assert_eq!(
            ok(object(of(T::Radical), vec![Vec::new(), text("2")])),
            "\\sqrt{2}"
        );
        assert_eq!(
            ok(object(of(T::Radical), vec![text("3"), text("𝑥")])),
            "\\sqrt[3]{x}"
        );
    }

    #[test]
    fn slashed_fraction() {
        assert_eq!(
            ok(object(of(T::SlashedFraction), vec![text("𝑎"), text("𝑏")])),
            "{}^{a}\\!/\\!_{b}"
        );
        let linear = with_char(T::SlashedFraction, '\u{2215}');
        assert_eq!(ok(object(linear, vec![text("𝑎"), text("𝑏")])), "a/b");
    }

    #[test]
    fn stack() {
        assert_eq!(
            ok(object(of(T::Stack), vec![text("𝑎"), text("𝑏")])),
            "\\substack{a \\\\ b}"
        );
    }

    /// A stretch stack's character is a brace, an arrow, or set over or under its body as
    /// its alignment places it.
    #[test]
    fn stretch_stack() {
        let stack = |c: char, align: Option<u8>| {
            object_of_x(Object {
                align,
                ..with_char(T::StretchStack, c)
            })
        };
        assert_eq!(ok(stack('\u{23de}', None)), "\\overbrace{x}");
        assert_eq!(ok(stack('→', None)), "\\xrightarrow{x}");
        assert_eq!(ok(stack('~', None)), "\\underset{~}{x}");
        assert_eq!(ok(stack('~', Some(1))), "\\overset{~}{x}");
        assert!(latex(stack('~', Some(7))).is_err());
    }

    /// Scripts: an empty one shows a placeholder.
    #[test]
    fn subscript_superscript_and_sub_sup() {
        assert_eq!(
            ok(object(of(T::Subscript), vec![text("𝑥"), text("𝑖")])),
            "x_{i}"
        );
        assert_eq!(
            ok(object(of(T::Subscript), vec![text("𝑥"), Vec::new()])),
            "x_{\\square}"
        );
        assert_eq!(
            ok(object(of(T::Superscript), vec![text("𝑥"), text("2")])),
            "x^{2}"
        );
        assert_eq!(
            ok(object(of(T::SubSup), vec![text("𝑥"), text("𝑖"), text("2")])),
            "x_{i}^{2}"
        );
    }

    #[test]
    fn upper_limit() {
        assert_eq!(
            ok(object(of(T::UpperLimit), vec![text("="), text("def")])),
            "\\overset{\\operatorname{def}}{=}"
        );
    }

    /// Math text by kind: styled letters decoded inside their style's command, math italic
    /// and Greek bare, LaTeX's own characters escaped, spaces thin.
    #[test]
    fn text_by_kind() {
        assert_eq!(ok(text("𝐗")), "\\mathbf{X}");
        assert_eq!(ok(text("𝔸")), "\\mathbb{A}");
        assert_eq!(ok(text("𝜃ℝ")), "θℝ");
        assert_eq!(ok(text("{%}")), "\\{\\%\\}");
        assert_eq!(ok(text("𝑥 𝑦")), "x\\,y");
        assert_eq!(ok(text("ⅆ𝑥")), "\\,ⅆx");
        // Sans bold italic Greek, the range one2html's U+17C9 typo left empty.
        assert_eq!(ok(text("𝞪")), "\\boldsymbol{\\mathsf{α}}");
    }

    /// An object whose markers run out, an empty object, and markers out of place.
    #[test]
    fn marker_edge_cases() {
        let mut unterminated = object(of(T::Fraction), vec![text("𝑎"), text("𝑏")]);
        unterminated.pop();
        let err = latex(unterminated).unwrap_err();
        assert!(err.contains("ends inside"), "{err}");

        assert_eq!(ok(object(of(T::Box), vec![Vec::new()])), "{}");
        assert_eq!(ok(Vec::new()), "");

        let stray_end = vec![run(END, of(T::Box))];
        assert!(latex(stray_end).is_err());
        let stray_separator = vec![run(SEP, of(T::Fraction))];
        assert!(latex(stray_separator).is_err());
        // A start whose run has no math object, as a math run past onenote_parser's last.
        assert!(latex(vec![run(START, Object::default())]).is_err());
        // An argument count the object doesn't take.
        let mut wrong_count = object(of(T::Fraction), vec![text("𝑎"), text("𝑏")]);
        wrong_count[0].1.args = 1;
        assert!(latex(wrong_count).is_err());
    }

    #[test]
    fn text_alone_drops_the_markers() {
        assert_eq!(latex_of_text("\u{fdd0}𝑎\u{fdee}2"), "a2");
    }
}
