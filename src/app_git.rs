use std::fmt::Debug;
use std::ops::Deref;
use std::ops::DerefMut;

use camino::Utf8PathBuf;
use rustc_hash::FxHashSet as HashSet;
use tracing::instrument;

use crate::config::Config;
use crate::git::BranchRef;
use crate::git::Git;
use crate::git::LocalBranchRef;
use crate::Worktree;
use crate::Worktrees;

/// A [`Git`] with borrowed [`Config`].
#[derive(Clone)]
pub struct AppGit<'a> {
    pub git: Git,
    pub config: &'a Config,
}

impl Debug for AppGit<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AppGit")
            .field(&self.git.get_directory())
            .finish()
    }
}

impl Deref for AppGit<'_> {
    type Target = Git;

    fn deref(&self) -> &Self::Target {
        &self.git
    }
}

impl DerefMut for AppGit<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.git
    }
}

impl AsRef<Git> for AppGit<'_> {
    fn as_ref(&self) -> &Git {
        &self.git
    }
}

impl AsRef<Config> for AppGit<'_> {
    fn as_ref(&self) -> &Config {
        self.config
    }
}

impl From<AppGit<'_>> for Git {
    fn from(value: AppGit<'_>) -> Self {
        value.git
    }
}

impl<'a> AppGit<'a> {
    pub fn with_directory(&self, path: Utf8PathBuf) -> Self {
        Self {
            git: self.git.with_directory(path),
            config: self.config,
        }
    }

    /// Get a list of remotes in the user's preference order.
    #[instrument(level = "trace")]
    pub fn preferred_remotes(&self) -> miette::Result<Vec<String>> {
        let mut all_remotes = self.remote().list()?.into_iter().collect::<HashSet<_>>();

        let mut sorted = Vec::with_capacity(all_remotes.len());

        if let Some(default_remote) = self.remote().get_default()? {
            if let Some(remote) = all_remotes.take(&default_remote) {
                sorted.push(remote);
            }
        }

        let preferred_remotes = self.config.file.remotes();
        for remote in preferred_remotes {
            if let Some(remote) = all_remotes.take(&remote) {
                sorted.push(remote);
            }
        }

        Ok(sorted)
    }

    /// Get the user's preferred remote, if any.
    #[instrument(level = "trace")]
    pub fn preferred_remote(&self) -> miette::Result<Option<String>> {
        Ok(self.preferred_remotes()?.first().cloned())
    }

    /// Get the user's preferred default branch.
    #[instrument(level = "trace")]
    pub fn preferred_branch(&self) -> miette::Result<Option<BranchRef>> {
        if let Some(default_remote) = self.preferred_remote()? {
            return self
                .remote()
                .default_branch(&default_remote)
                .map(BranchRef::from)
                .map(Some);
        }

        let preferred_branches = self.config.file.default_branches();
        let all_branches = self.branch().list_local()?;
        for preferred_branch in preferred_branches {
            let preferred_branch = LocalBranchRef::new(preferred_branch);
            if all_branches.contains(&preferred_branch) {
                return Ok(Some(preferred_branch.into()));
            } else if let Some(remote_branch) =
                self.remote().for_branch(preferred_branch.branch_name())?
            {
                return Ok(Some(remote_branch.into()));
            }
        }

        Ok(None)
    }

    /// Get the worktree for the preferred branch, if any.
    #[instrument(level = "trace")]
    pub fn preferred_branch_worktree(
        &self,
        preferred_branch: Option<&BranchRef>,
        worktrees: Option<&Worktrees>,
    ) -> miette::Result<Option<Worktree>> {
        let worktrees = match worktrees {
            Some(worktrees) => worktrees,
            None => &self.worktree().list()?,
        };
        let preferred_branch = match preferred_branch {
            Some(preferred_branch) => preferred_branch,
            None => &match self.preferred_branch()? {
                Some(preferred_branch) => preferred_branch,
                None => {
                    return Ok(None);
                }
            },
        };

        // TODO: Check for branch with the default as an upstream as well?
        Ok(worktrees.for_branch(&preferred_branch.as_local()).cloned())
    }

    /// Get the path to _some_ worktree.
    ///
    /// This prefers, in order:
    /// 1. The current worktree.
    /// 2. The worktree for the default branch.
    /// 3. Any non-bare worktree.
    /// 4. A bare worktree.
    #[instrument(level = "trace")]
    pub fn some_worktree(&self) -> miette::Result<Utf8PathBuf> {
        if self.worktree().is_inside()? {
            tracing::debug!("Inside worktree");
            // Test: `add_by_path`
            return self.worktree().root();
        }
        let worktrees = self.worktree().list()?;

        if let Some(worktree) = self.preferred_branch_worktree(None, Some(&worktrees))? {
            tracing::debug!(%worktree, "Found worktree for preferred branch");
            // Test: `add_from_container`
            return Ok(worktree.path);
        }

        tracing::debug!("No worktree for preferred branch");

        if worktrees.main().head.is_bare() && worktrees.len() > 1 {
            // Find a non-bare worktree.
            //
            // Test: `add_from_container_no_default_branch`
            let worktree = worktrees
                .into_iter()
                .find(|(_path, worktree)| !worktree.head.is_bare())
                .expect("Only one worktree can be bare")
                .0;

            tracing::debug!(%worktree, "Found non-bare worktree");
            return Ok(worktree);
        }

        // Otherwise, get the main worktree.
        // Either the main worktree is bare and there's no other worktrees, or the main
        // worktree is not bare.
        //
        // Note: If the main worktree isn't bare, there's no way to run Git commands
        // without being in a worktree. IDK I guess you can probably do something silly
        // with separating the Git directory and the worktree but like, why.
        //
        // Tests:
        // - `add_from_bare_no_worktrees`
        tracing::debug!("Non-bare main worktree or no non-bare worktrees");
        Ok(worktrees.main_path().to_owned())
    }
}
