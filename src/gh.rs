use std::ops::RangeInclusive;

use camino::Utf8Path;
use winnow::combinator::eof;
use winnow::token::take_while;
use winnow::PResult;
use winnow::Parser;

pub fn looks_like_gh_url(url: &str) -> bool {
    parse_gh_url.parse(url).is_ok() && !Utf8Path::new(url).exists()
}

pub fn parse_gh_url(input: &mut &str) -> PResult<()> {
    /// Technically they're a little more restrictive than this, but it's fine.
    ///
    /// See: <https://github.com/dead-claudia/github-limits>
    const GITHUB_NAME_CHAR: (
        RangeInclusive<char>,
        RangeInclusive<char>,
        RangeInclusive<char>,
        char,
        char,
        char,
    ) = ('a'..='z', 'A'..='Z', '0'..='9', '-', '_', '.');

    let _organization = take_while(1..40, GITHUB_NAME_CHAR).parse_next(input)?;
    let _ = '/'.parse_next(input)?;
    let _repository = take_while(1..=100, GITHUB_NAME_CHAR).parse_next(input)?;
    let _ = eof.parse_next(input)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_gh_url() {
        assert!(looks_like_gh_url("9999years/git-prole"));
        assert!(looks_like_gh_url("lf-/flakey-profile"));
        assert!(looks_like_gh_url("soft/puppy_doggy"));
        assert!(looks_like_gh_url("soft/puppy.doggy"));

        assert!(looks_like_gh_url(&format!(
            "{}/{}",
            "a".repeat(39),
            "a".repeat(100)
        )));
        assert!(!looks_like_gh_url(&format!(
            "{}/{}",
            "a".repeat(40),
            "a".repeat(100)
        )));
        assert!(!looks_like_gh_url(&format!(
            "{}/{}",
            "a".repeat(39),
            "a".repeat(101)
        )));
    }
}
