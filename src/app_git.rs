use std::fmt::Debug;
use std::ops::Deref;
use std::ops::DerefMut;

use camino::Utf8PathBuf;
use miette::miette;
use rustc_hash::FxHashSet as HashSet;
use tracing::instrument;

use crate::config::Config;
use crate::git::BranchRef;
use crate::git::Git;
use crate::git::LocalBranchRef;

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
    pub fn preferred_branch(&self) -> miette::Result<BranchRef> {
        if let Some(default_remote) = self.preferred_remote()? {
            return self
                .remote()
                .default_branch(&default_remote)
                .map(BranchRef::from);
        }

        let preferred_branches = self.config.file.default_branches();
        let all_branches = self.branch().list_local()?;
        for preferred_branch in preferred_branches {
            let preferred_branch = LocalBranchRef::new(preferred_branch);
            if all_branches.contains(&preferred_branch) {
                return Ok(preferred_branch.into());
            } else if let Some(remote_branch) =
                self.remote().for_branch(preferred_branch.branch_name())?
            {
                return Ok(remote_branch.into());
            }
        }

        Err(miette!(
            "No default branch found; specify a `--default-branch` to check out"
        ))
    }
}
