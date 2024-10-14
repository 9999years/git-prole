use std::borrow::Cow;
use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;

use miette::Diagnostic;
use miette::LabeledSpan;
use winnow::error::ContextError;

pub type PResult<O, E = ContextError<ParseContext>> = winnow::PResult<O, E>;

pub fn desc(while_parsing_a: impl Into<Cow<'static, str>>) -> ParseContext {
    ParseContext::Description(while_parsing_a.into())
}

pub fn expected(while_parsing_a: impl Into<Cow<'static, str>>) -> ParseContext {
    ParseContext::Expected(while_parsing_a.into())
}

#[derive(Debug, Clone)]
pub enum ParseContext {
    Description(Cow<'static, str>),
    Expected(Cow<'static, str>),
}

impl Display for ParseContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseContext::Description(description) => write!(f, "while parsing {description}"),
            ParseContext::Expected(expected) => write!(f, "expected {expected}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseError<'a> {
    inner: winnow::error::ParseError<&'a str, ContextError<ParseContext>>,
}

impl<'a> ParseError<'a> {
    pub fn new(error: winnow::error::ParseError<&'a str, ContextError<ParseContext>>) -> Self {
        Self { inner: error }
    }
}

impl Display for ParseError<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parsing failed")
    }
}

impl Error for ParseError<'_> {}

impl Diagnostic for ParseError<'_> {
    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        None
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(self.inner.input())
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(self.inner.inner().context().map(|context| {
            LabeledSpan::new_with_span(Some(context.to_string()), self.inner.offset())
        })))
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use miette::GraphicalReportHandler;
    use miette::GraphicalTheme;
    use winnow::PResult;
    use winnow::Parser;

    use super::*;

    fn parser(input: &mut &str) -> PResult<(), ContextError<ParseContext>> {
        let _ = "Hello, ".parse_next(input)?;
        let _ = "world!".context(expected("world!")).parse_next(input)?;
        Ok(())
    }

    fn render_diagnostic(diagnostic: &dyn Diagnostic) -> String {
        let mut rendered = String::new();

        GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
            .render_report(&mut rendered, diagnostic)
            .unwrap();

        rendered
    }

    #[test]
    fn test_parse_error() {
        let err = parser
            .parse("Hello, Puppy!")
            .map_err(ParseError::new)
            .unwrap_err();

        expect![[r#"
              × Parsing failed
               ╭────
             1 │ Hello, Puppy!
               ·        ▲
               ·        ╰── expected world!
               ╰────
        "#]]
        .assert_eq(&render_diagnostic(&err));
    }
}
