use std::fmt::Debug;

use command_error::CommandExt;
use command_error::OutputContext;
use miette::IntoDiagnostic;
use rustc_hash::FxHashSet as HashSet;
use tracing::instrument;
use utf8_command::Utf8Output;

use super::BranchRef;
use super::Git;
use super::LocalBranchRef;

/// Git methods for dealing with worktrees.
#[repr(transparent)]
pub struct GitBranch<'a>(&'a Git);

impl Debug for GitBranch<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.0, f)
    }
}

impl<'a> GitBranch<'a> {
    pub fn new(git: &'a Git) -> Self {
        Self(git)
    }

    /// Lists local branches.
    #[instrument(level = "trace")]
    pub fn list_local(&self) -> miette::Result<HashSet<LocalBranchRef>> {
        self.0
            .refs()
            .for_each_ref(Some(&["refs/heads/**"]))?
            .into_iter()
            .map(LocalBranchRef::try_from)
            .collect::<Result<HashSet<_>, _>>()
    }

    /// Lists local and remote branches.
    #[instrument(level = "trace")]
    pub fn list(&self) -> miette::Result<HashSet<BranchRef>> {
        self.0
            .refs()
            .for_each_ref(Some(&["refs/heads/**", "refs/remotes/**"]))?
            .into_iter()
            .map(BranchRef::try_from)
            .collect::<Result<HashSet<_>, _>>()
    }

    /// Does a local branch exist?
    #[instrument(level = "trace")]
    pub fn exists_local(&self, branch: &str) -> miette::Result<bool> {
        self.0
            .command()
            .args(["show-ref", "--quiet", "--branches", branch])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                Ok::<_, command_error::Error>(context.status().success())
            })
            .into_diagnostic()
    }

    /// Does the given branch name exist as a local branch, a unique remote branch, or neither?
    pub fn local_or_remote(&self, branch: &str) -> miette::Result<Option<BranchRef>> {
        if self.exists_local(branch)? {
            Ok(Some(LocalBranchRef::new(branch.to_owned()).into()))
        } else if let Some(remote) = self.0.remote().for_branch(branch)? {
            // This is the implicit behavior documented in `git-worktree(1)`.
            Ok(Some(remote.into()))
        } else {
            Ok(None)
        }
    }

    pub fn current(&self) -> miette::Result<Option<LocalBranchRef>> {
        match self.0.refs().rev_parse_symbolic_full_name("HEAD")? {
            Some(ref_name) => Ok(Some(LocalBranchRef::try_from(ref_name)?)),
            None => Ok(None),
        }
    }

    /// Get the branch that a given branch is tracking.
    pub fn upstream(&self, branch: &str) -> miette::Result<Option<BranchRef>> {
        match self
            .0
            .refs()
            .rev_parse_symbolic_full_name(&format!("{branch}@{{upstream}}"))?
        {
            Some(ref_name) => Ok(Some(BranchRef::try_from(ref_name)?)),
            // NOTE: `branch` may not exist at all!
            None => Ok(None),
        }
    }
}
