use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

use clonable_command::Command as ClonableCommand;
use itertools::Itertools;
use miette::IntoDiagnostic;
use tempfile::TempDir;

/// Builder for [`GitProle`].
pub struct GitProleBuilder {
    git_prole_args: Vec<OsString>,
    log_filters: Vec<String>,
}

impl GitProleBuilder {
    /// Create a new builder for a `git-prole` session.
    pub fn new() -> Self {
        Self {
            git_prole_args: Default::default(),
            log_filters: Default::default(),
        }
    }

    /// Add an argument to the `git-prole` invocation.
    pub fn with_arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.git_prole_args.push(arg.as_ref().to_owned());
        self
    }

    /// Add multiple arguments to the `git-prole` invocation.
    pub fn with_args(mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Self {
        self.git_prole_args
            .extend(args.into_iter().map(|s| s.as_ref().to_owned()));
        self
    }

    /// Add a `--log` clause to the `git-prole` invocation.
    pub fn with_log_filter(mut self, log_filter: impl AsRef<str>) -> Self {
        self.log_filters.push(log_filter.as_ref().to_owned());
        self
    }

    /// Add multiple `--log` clauses to the `git-prole` invocation.
    pub fn with_log_filters(
        mut self,
        log_filters: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Self {
        self.log_filters
            .extend(log_filters.into_iter().map(|s| s.as_ref().to_owned()));
        self
    }

    /// Start `git-prole`.
    pub fn start(self) -> miette::Result<GitProle> {
        GitProle::from_builder(self)
    }
}

/// `git-prole` session for integration testing.
pub struct GitProle {
    /// The command which started the `git-prole` session.
    command: ClonableCommand,
    /// The current working directory of the `git-prole` session.
    tempdir: TempDir,
}

impl GitProle {
    fn from_builder(builder: GitProleBuilder) -> miette::Result<Self> {
        let tempdir = tempfile::tempdir().into_diagnostic()?;

        tracing::info!("Starting git-prole");

        let log_filters = ["git_prole=debug"]
            .into_iter()
            .chain(builder.log_filters.iter().map(|s| s.as_ref()))
            .join(",");

        let command = ClonableCommand::new(test_bin::get_test_bin("git-prole").get_program())
            .args(["--log", &log_filters])
            .args(builder.git_prole_args)
            .current_dir(&tempdir);

        Ok(Self { command, tempdir })
    }

    pub fn new() -> miette::Result<Self> {
        GitProleBuilder::new().start()
    }

    pub fn cmd(&self) -> Command {
        self.command.to_std()
    }

    pub fn path(&self, tail: &str) -> PathBuf {
        self.tempdir.path().join(tail)
    }
}
