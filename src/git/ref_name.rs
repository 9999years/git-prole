use std::fmt::Display;
use std::str::FromStr;

use miette::miette;
use winnow::combinator::rest;
use winnow::token::take_till;
use winnow::PResult;
use winnow::Parser;

/// A Git ref (a file under `refs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ref {
    /// The ref kind; usually `heads`, `remotes`, or `tags`.
    ///
    /// Other kinds:
    /// - `stash`
    /// - `bisect`
    kind: String,
    /// The ref name; everything after the kind.
    name: String,
}

impl Ref {
    /// The `kind` indicating a branch reference.
    const HEADS: &str = "heads";
    /// The `kind` indicating a remote-tracking branch reference.
    const REMOTES: &str = "remotes";
    /// The `kind` indicating a tag reference.
    const TAGS: &str = "tags";

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Determine if this is a remote branch, i.e. its kind is [`Self::REMOTES`].
    pub fn is_remote_branch(&self) -> bool {
        self.kind == Self::REMOTES
    }

    /// Determine if this is a local branch, i.e. its kind is [`Self::HEADS`].
    pub fn is_local_branch(&self) -> bool {
        self.kind == Self::HEADS
    }

    /// Determine if this is a tag, i.e. its kind is [`Self::TAGS`].
    #[expect(dead_code)]
    pub(crate) fn is_tag(&self) -> bool {
        self.kind == Self::TAGS
    }

    /// If this is a local branch ref, return the branch name.
    pub fn local_branch_name(&self) -> Option<&str> {
        if self.is_local_branch() {
            Some(&self.name)
        } else {
            None
        }
    }

    /// If this is a remote branch ref, return a pair of the remote and branch names.
    pub fn remote_and_branch(&self) -> Option<(&str, &str)> {
        if self.is_remote_branch() {
            self.name.split_once('/')
        } else {
            None
        }
    }

    /// Parse a ref name like `refs/puppy/doggy`.
    ///
    /// Needs at least one slash after `refs/`; this does not treat `refs/puppy` as a valid ref
    /// name.
    pub fn parser(input: &mut &str) -> PResult<Self> {
        let _refs_prefix = "refs/".parse_next(input)?;

        let kind = take_till(1.., '/').parse_next(input)?;
        let _ = '/'.parse_next(input)?;
        let name = rest.parse_next(input)?;

        Ok(Self {
            kind: kind.to_owned(),
            name: name.to_owned(),
        })
    }
}

impl FromStr for Ref {
    type Err = miette::Report;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parser.parse(input).map_err(|err| miette!("{err}"))
    }
}

impl Display for Ref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            write!(f, "refs/{}/{}", self.kind, self.name)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ref_parse_no_slash() {
        assert!(Ref::from_str("refs/puppy").is_err());
    }

    #[test]
    fn test_ref_parse_simple() {
        assert_eq!(
            Ref::from_str("refs/puppy/doggy").unwrap(),
            Ref {
                kind: "puppy".into(),
                name: "doggy".into()
            }
        );
    }

    #[test]
    fn test_ref_parse_multiple_slashes() {
        assert_eq!(
            Ref::from_str("refs/puppy/doggy/softie/cutie").unwrap(),
            Ref {
                kind: "puppy".into(),
                name: "doggy/softie/cutie".into()
            }
        );
    }
}
