use std::fmt::Display;

use super::commit_hash::CommitHash;
use super::ref_name::Ref;
use super::status::Status;

/// What, exactly, is `HEAD` doing?
#[derive(Debug)]
pub struct Head {
    pub status: Status,
    pub kind: HeadKind,
}

impl Head {
    pub fn is_clean(&self) -> bool {
        self.status.is_clean()
    }

    pub fn commitish(&self) -> &str {
        match &self.kind {
            HeadKind::Detached(commit) => commit.as_str(),
            HeadKind::Ref(ref_name) => ref_name.name(),
        }
    }

    pub fn branch_name(&self) -> Option<&str> {
        match &self.kind {
            HeadKind::Detached(_) => None,
            // There's no way we can have a remote branch checked out.
            HeadKind::Ref(ref_name) => ref_name.local_branch_name(),
        }
    }

    pub fn is_on_branch(&self, branch: &str) -> bool {
        self.branch_name()
            .map_or(false, |checked_out| branch == checked_out)
    }
}

/// Is `HEAD` detached?
#[derive(Debug, PartialEq, Eq)]
pub enum HeadKind {
    Detached(CommitHash),
    Ref(Ref),
}

impl Display for HeadKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeadKind::Detached(commit) => Display::fmt(commit, f),
            HeadKind::Ref(ref_name) => Display::fmt(ref_name, f),
        }
    }
}
