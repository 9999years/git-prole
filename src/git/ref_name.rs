use std::fmt::Display;
use std::str::FromStr;

use miette::miette;

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
    pub fn is_tag(&self) -> bool {
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
}

impl FromStr for Ref {
    type Err = miette::Report;

    fn from_str(original: &str) -> Result<Self, Self::Err> {
        let rest = original
            .strip_prefix("refs/")
            .ok_or_else(|| miette!("Refs must start with `refs/`: {original}"))?;

        let (kind, name) = rest.split_once('/').ok_or_else(|| {
            miette!("Ref names should have at least one `/` after `refs/`: {original}")
        })?;

        Ok(Self {
            kind: kind.to_owned(),
            name: name.to_owned(),
        })
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
