use std::fmt::Display;

use super::commit_hash::CommitHash;
use super::Ref;

/// A resolved `<commit-ish>`, which can either be a commit hash or a ref name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedCommitish {
    /// A commit hash.
    Commit(CommitHash),
    /// A ref name.
    Ref(Ref),
}

impl Display for ResolvedCommitish {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolvedCommitish::Commit(commit) => Display::fmt(commit, f),
            ResolvedCommitish::Ref(ref_name) => Display::fmt(ref_name, f),
        }
    }
}
