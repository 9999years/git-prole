use std::fmt::Debug;

use camino::Utf8PathBuf;
use command_error::CommandExt;
use miette::miette;
use tracing::instrument;

use crate::PathDisplay;

use super::GitLike;

/// Git methods for dealing with paths.
#[repr(transparent)]
pub struct GitPath<'a, G>(&'a G);

impl<G> Debug for GitPath<'_, G>
where
    G: GitLike,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("GitPath")
            .field(&self.0.get_current_dir().as_ref())
            .finish()
    }
}

impl<'a, G> GitPath<'a, G>
where
    G: GitLike,
{
    pub fn new(git: &'a G) -> Self {
        Self(git)
    }

    /// Get the path of the repository root, for display purposes only.
    ///
    /// If in a working tree, get the repository root (`git rev-parse --show-toplevel`).
    ///
    /// If the repository is bare, get the `.git` directory (`git rev-parse --git-dir`):
    /// - If it's named `.git`, get its parent.
    /// - Otherwise, return it directly.
    ///
    /// Otherwise, error.
    #[instrument(level = "trace")]
    pub fn repo_root_display(&self) -> miette::Result<Utf8PathBuf> {
        if self.0.worktree().is_inside()? {
            self.0.worktree().root()
        } else if self.0.config().is_bare()? {
            let git_dir = self.git_common_dir()?;
            let git_dir_basename = git_dir
                .file_name()
                .ok_or_else(|| miette!("Git directory has no basename: {git_dir}"))?;
            if git_dir_basename == ".git" {
                Ok(git_dir
                    .parent()
                    .ok_or_else(|| miette!("Git directory has no parent: {git_dir}"))?
                    .to_owned())
            } else {
                Ok(git_dir)
            }
        } else {
            Err(miette!(
                "Path is not in a working tree or a bare repository: {}",
                self.0.get_current_dir().as_ref().display_path_cwd()
            ))
        }
    }

    /// Get the `.git` directory path.
    #[expect(dead_code)] // #[instrument(level = "trace")]
    pub(crate) fn get_git_dir(&self) -> miette::Result<Utf8PathBuf> {
        Ok(self
            .0
            .as_git()
            .rev_parse_command()
            .arg("--git-dir")
            .output_checked_utf8()
            .map(|output| Utf8PathBuf::from(output.stdout.trim()))?)
    }

    /// Get the common `.git` directory for all worktrees.
    #[instrument(level = "trace")]
    pub fn git_common_dir(&self) -> miette::Result<Utf8PathBuf> {
        Ok(self
            .0
            .as_git()
            .rev_parse_command()
            .arg("--git-common-dir")
            .output_checked_utf8()
            .map(|output| Utf8PathBuf::from(output.stdout.trim()))?)
    }
}
