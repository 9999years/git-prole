use std::fmt::Debug;

use camino::Utf8Path;
use command_error::CommandExt;
use command_error::OutputContext;
use rustc_hash::FxHashSet;
use tracing::instrument;
use utf8_command::Utf8Output;

use crate::AppGit;

use super::BranchRef;
use super::GitLike;
use super::LocalBranchRef;

/// Git methods for dealing with worktrees.
#[repr(transparent)]
pub struct GitBranch<'a, G>(&'a G);

impl<G> Debug for GitBranch<'_, G>
where
    G: GitLike,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("GitBranch")
            .field(&self.0.get_current_dir().as_ref())
            .finish()
    }
}

impl<'a, G> GitBranch<'a, G>
where
    G: GitLike,
{
    pub fn new(git: &'a G) -> Self {
        Self(git)
    }

    /// Lists local branches.
    #[instrument(level = "trace")]
    pub fn list_local(&self) -> miette::Result<FxHashSet<LocalBranchRef>> {
        self.0
            .refs()
            .for_each_ref(Some(&["refs/heads/**"]))?
            .into_iter()
            .map(LocalBranchRef::try_from)
            .collect::<Result<FxHashSet<_>, _>>()
    }

    /// Lists local and remote branches.
    #[instrument(level = "trace")]
    pub fn list(&self) -> miette::Result<FxHashSet<BranchRef>> {
        self.0
            .refs()
            .for_each_ref(Some(&["refs/heads/**", "refs/remotes/**"]))?
            .into_iter()
            .map(BranchRef::try_from)
            .collect::<Result<FxHashSet<_>, _>>()
    }

    /// Does a local branch exist?
    #[instrument(level = "trace")]
    pub fn exists_local(&self, branch: &str) -> miette::Result<bool> {
        Ok(self
            .0
            .command()
            .args(["show-ref", "--quiet", "--branches", branch])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                Ok::<_, command_error::Error>(context.status().success())
            })?)
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

impl<'a, C> GitBranch<'a, AppGit<'a, C>>
where
    C: AsRef<Utf8Path>,
{
    /// Get the user's preferred default branch.
    #[instrument(level = "trace")]
    pub fn preferred(&self) -> miette::Result<Option<BranchRef>> {
        if let Some(default_remote) = self.0.remote().preferred()? {
            return self
                .0
                .remote()
                .default_branch(&default_remote)
                .map(BranchRef::from)
                .map(Some);
        }

        let preferred_branches = self.0.config.file.default_branches();
        let all_branches = self.0.branch().list_local()?;
        for preferred_branch in preferred_branches {
            let preferred_branch = LocalBranchRef::new(preferred_branch);
            if all_branches.contains(&preferred_branch) {
                return Ok(Some(preferred_branch.into()));
            } else if let Some(remote_branch) =
                self.0.remote().for_branch(preferred_branch.branch_name())?
            {
                return Ok(Some(remote_branch.into()));
            }
        }

        Ok(None)
    }
}
