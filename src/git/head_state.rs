use std::fmt::Display;

use tracing::instrument;

use super::CommitHash;
use super::LocalBranchRef;

/// Is `HEAD` detached?
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadKind {
    Detached(CommitHash),
    Branch(LocalBranchRef),
}

impl HeadKind {
    pub fn commitish(&self) -> &str {
        match &self {
            HeadKind::Detached(commit) => commit.as_str(),
            HeadKind::Branch(ref_name) => ref_name.name(),
        }
    }

    pub fn branch_name(&self) -> Option<&str> {
        match &self {
            HeadKind::Detached(_) => None,
            // There's no way we can have a remote branch checked out.
            HeadKind::Branch(branch) => Some(branch.branch_name()),
        }
    }

    #[instrument(level = "trace")]
    pub fn is_on_branch(&self, branch_name: &str) -> bool {
        match self {
            HeadKind::Detached(_) => false,
            HeadKind::Branch(checked_out) => checked_out.branch_name() == branch_name,
        }
    }
}

impl Display for HeadKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeadKind::Detached(commit) => Display::fmt(commit, f),
            HeadKind::Branch(ref_name) => Display::fmt(ref_name, f),
        }
    }
}
