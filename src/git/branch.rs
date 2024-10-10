use std::collections::HashSet;
use std::fmt::Debug;

use command_error::CommandExt;
use command_error::OutputContext;
use miette::IntoDiagnostic;
use tracing::instrument;
use utf8_command::Utf8Output;

use super::Git;

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
    pub fn list_local(&self) -> miette::Result<HashSet<String>> {
        Ok(self
            .0
            .command()
            .args(["branch", "--format=%(refname:short)"])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout
            .lines()
            .map(|line| line.to_owned())
            .collect())
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
}
