use std::fmt::Debug;

use camino::Utf8PathBuf;
use command_error::CommandExt;
use miette::Context;
use miette::IntoDiagnostic;
use tracing::instrument;

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

    /// `git rev-parse --show-toplevel`
    #[instrument(level = "trace")]
    pub fn repo_root(&self) -> miette::Result<Utf8PathBuf> {
        Ok(self
            .0
            .rev_parse_command()
            .arg("--show-toplevel")
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err("Failed to get working directory of repository")?
            .stdout
            .trim()
            .into())
    }

    /// Get the `.git` directory path.
    #[expect(dead_code)] // #[instrument(level = "trace")]
    pub(crate) fn get_git_dir(&self) -> miette::Result<Utf8PathBuf> {
        self.0
            .rev_parse_command()
            .arg("--git-dir")
            .output_checked_utf8()
            .into_diagnostic()
            .map(|output| Utf8PathBuf::from(output.stdout.trim()))
    }

    /// Get the common `.git` directory for all worktrees.
    #[instrument(level = "trace")]
    pub fn git_common_dir(&self) -> miette::Result<Utf8PathBuf> {
        self.0
            .rev_parse_command()
            .arg("--git-common-dir")
            .output_checked_utf8()
            .into_diagnostic()
            .map(|output| Utf8PathBuf::from(output.stdout.trim()))
    }
}
