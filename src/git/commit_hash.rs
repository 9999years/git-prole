use std::fmt::Display;
use std::str::FromStr;

use derive_more::{AsRef, Constructor, Deref, DerefMut, From, Into};
use miette::miette;
use winnow::combinator::repeat;
use winnow::token::one_of;
use winnow::PResult;
use winnow::Parser;

/// A Git commit hash.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Into, AsRef, Deref, DerefMut, Constructor,
)]
pub struct CommitHash(String);

impl CommitHash {
    /// A fake commit hash for testing purposes.
    #[cfg(test)]
    pub fn fake() -> Self {
        Self("a".repeat(40))
    }

    /// Get an abbreviated 8-character Git hash.
    pub fn abbrev(&self) -> &str {
        &self.0[..8]
    }

    pub fn parser(input: &mut &str) -> PResult<Self> {
        Ok(Self::from(
            repeat(40, one_of(('0'..='9', 'a'..='f')))
                .map(|()| ())
                .take()
                .parse_next(input)?,
        ))
    }
}

impl Display for CommitHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            Display::fmt(&self.0, f)
        } else {
            Display::fmt(self.abbrev(), f)
        }
    }
}

impl<S> From<S> for CommitHash
where
    S: AsRef<str>,
{
    fn from(value: S) -> Self {
        Self(value.as_ref().into())
    }
}

impl FromStr for CommitHash {
    type Err = miette::Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parser.parse(s).map_err(|err| miette!("{err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_commit_hash() {
        assert_eq!(
            CommitHash::from_str("1233def1234def1234def1234def1234def1234b").unwrap(),
            CommitHash::new("1233def1234def1234def1234def1234def1234b".into()),
        );

        // Too short
        assert!(CommitHash::from_str("1233def1234def1234def1234def1234def1234").is_err());

        // Too long
        assert!(CommitHash::from_str("1233def1234def1234def1234def1234def1234ab").is_err());

        // Uppercase not allowed
        assert!(CommitHash::from_str("1233DEF1234DEF1234DEF1234DEF1234DEF1234B").is_err());

        // Illegal character
        assert!(CommitHash::from_str("1233def1234def1234gef1234def1234def1234b").is_err());
    }
}
