use std::process::Command;

use camino::Utf8Path;

use super::Git;
use super::GitBranch;
use super::GitConfig;
use super::GitPath;
use super::GitRefs;
use super::GitRemote;
use super::GitStatus;
use super::GitWorktree;

pub trait GitLike: Sized {
    type CurrentDir: AsRef<Utf8Path>;

    fn as_git(&self) -> &Git<Self::CurrentDir>;

    #[inline]
    fn get_current_dir(&self) -> &Self::CurrentDir {
        self.as_git().get_current_dir()
    }

    /// Get a `git` command.
    #[inline]
    fn command(&self) -> Command {
        self.as_git().command()
    }

    /// Methods for dealing with Git remotes.
    #[inline]
    fn remote(&self) -> GitRemote<'_, Self> {
        GitRemote::new(self)
    }

    /// Methods for dealing with Git remotes.
    #[inline]
    fn path(&self) -> GitPath<'_, Self> {
        GitPath::new(self)
    }

    /// Methods for dealing with Git remotes.
    #[inline]
    fn worktree(&self) -> GitWorktree<'_, Self> {
        GitWorktree::new(self)
    }

    /// Methods for dealing with Git refs.
    #[inline]
    fn refs(&self) -> GitRefs<'_, Self> {
        GitRefs::new(self)
    }

    /// Methods for dealing with Git statuses and the working tree.
    #[inline]
    fn status(&self) -> GitStatus<'_, Self> {
        GitStatus::new(self)
    }

    /// Methods for dealing with Git statuses and the working tree.
    #[inline]
    fn config(&self) -> GitConfig<'_, Self> {
        GitConfig::new(self)
    }

    /// Methods for dealing with Git statuses and the working tree.
    #[inline]
    fn branch(&self) -> GitBranch<'_, Self> {
        GitBranch::new(self)
    }
}
