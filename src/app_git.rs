use std::collections::HashSet;
use std::fmt::Debug;
use std::ops::Deref;

use miette::miette;
use tracing::instrument;

use crate::config::Config;
use crate::git::Git;

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

impl AsRef<Git> for AppGit<'_> {
    fn as_ref(&self) -> &Git {
        &self.git
    }
}

impl AsRef<Config> for AppGit<'_> {
    fn as_ref(&self) -> &Config {
        &self.config
    }
}

impl From<AppGit<'_>> for Git {
    fn from(value: AppGit<'_>) -> Self {
        value.git
    }
}

impl<'a> AppGit<'a> {
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
    pub fn preferred_branch(&self) -> miette::Result<String> {
        if let Some(default_remote) = self.preferred_remote()? {
            return self.remote().default_branch(&default_remote);
        }

        let preferred_branches = self.config.file.default_branches();
        let all_branches = self.branch().list_local()?;
        for branch in preferred_branches {
            if all_branches.contains(&branch) {
                return Ok(branch);
            }
        }

        Err(miette!(
            "No default branch found; specify a `--default-branch` to check out"
        ))
    }
}
