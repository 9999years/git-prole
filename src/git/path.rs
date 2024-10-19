use std::fmt::Debug;

use camino::Utf8PathBuf;
use command_error::CommandExt;
use miette::miette;
use tracing::instrument;

use crate::PathDisplay;

use super::Git;

/// Git methods for dealing with paths.
#[repr(transparent)]
pub struct GitPath<'a>(&'a Git);

impl Debug for GitPath<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.0, f)
    }
}

impl<'a> GitPath<'a> {
    pub fn new(git: &'a Git) -> Self {
        Self(git)
    }

    /// If in a working tree, get the repository root (`git rev-parse --show-toplevel`). If the
    /// repository is bare, get the `.git` directory (`git rev-parse --git-dir`). Otherwise, error.
    #[instrument(level = "trace")]
    pub fn repo_root_or_git_common_dir_if_bare(&self) -> miette::Result<Utf8PathBuf> {
        if self.0.worktree().is_inside()? {
            self.0.worktree().root()
        } else if self.0.config().is_bare()? {
            self.git_common_dir()
        } else {
            Err(miette!(
                "Path is not in a working tree or a bare repository: {}",
                self.0.get_directory().display_path_cwd()
            ))
        }
    }

    /// Get the `.git` directory path.
    #[expect(dead_code)] // #[instrument(level = "trace")]
    pub(crate) fn get_git_dir(&self) -> miette::Result<Utf8PathBuf> {
        Ok(self
            .0
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
            .rev_parse_command()
            .arg("--git-common-dir")
            .output_checked_utf8()
            .map(|output| Utf8PathBuf::from(output.stdout.trim()))?)
    }
}
