use std::ffi::OsStr;
use std::ffi::OsString;
use std::process::Command;

use camino::Utf8PathBuf;
use clonable_command::Command as ClonableCommand;
use command_error::CommandExt;
use expect_test::Expect;
use fs_err as fs;
use git_prole::format_bulleted_list;
use git_prole::Git;
use git_prole::Utf8TempDir;
use itertools::Itertools;
use miette::miette;
use miette::IntoDiagnostic;
use utf8_command::Utf8Output;

/// Builder for [`GitProle`].
#[derive(Default)]
pub struct GitProleBuilder {
    git_prole_args: Vec<OsString>,
    log_filters: Vec<String>,
}

impl GitProleBuilder {
    /// Create a new builder for a `git-prole` session.
    pub fn new() -> Self {
        Default::default()
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
    pub fn build(self) -> miette::Result<GitProle> {
        GitProle::from_builder(self)
    }
}

/// `git-prole` session for integration testing.
pub struct GitProle {
    /// The command which started the `git-prole` session.
    command: ClonableCommand,
    /// The current working directory of the `git-prole` session.
    tempdir: Utf8TempDir,
}

impl GitProle {
    fn from_builder(builder: GitProleBuilder) -> miette::Result<Self> {
        let tempdir = Utf8TempDir::new()?;

        let log_filters = ["git_prole=debug"]
            .into_iter()
            .chain(builder.log_filters.iter().map(|s| s.as_ref()))
            .join(",");

        let command = ClonableCommand::new(test_bin::get_test_bin("git-prole").get_program())
            .envs([
                // > Whether to skip reading settings from the system-wide $(prefix)/etc/gitconfig
                // > file.
                ("GIT_CONFIG_NOSYSTEM", "1"),
                // > Can be set to /dev/null to skip reading configuration files of the respective
                // > level.
                ("GIT_CONFIG_SYSTEM", "/dev/null"),
                ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ])
            .args(["--log", &log_filters])
            .args(builder.git_prole_args)
            .current_dir(&tempdir);

        Ok(Self { command, tempdir })
    }

    pub fn new() -> miette::Result<Self> {
        GitProleBuilder::new().build()
    }

    pub fn with_args(args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> miette::Result<Self> {
        GitProleBuilder::new().with_args(args).build()
    }

    pub fn output(
        &self,
        args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    ) -> miette::Result<Utf8Output> {
        self.cmd()
            .args(args)
            .output_checked_utf8()
            .into_diagnostic()
    }

    pub fn cmd(&self) -> Command {
        self.command.to_std()
    }

    pub fn path(&self, tail: &str) -> Utf8PathBuf {
        self.tempdir.join(tail)
    }

    pub fn exists(&self, path: &str) -> bool {
        self.path(path).exists()
    }

    pub fn contents(&self, path: &str) -> miette::Result<String> {
        fs::read_to_string(self.path(path)).into_diagnostic()
    }

    #[track_caller]
    pub fn assert_exists(&self, paths: &[&str]) {
        let mut missing = Vec::new();
        for path in paths {
            if !self.exists(path) {
                missing.push(path);
            }
        }

        if !missing.is_empty() {
            panic!(
                "{:?}",
                miette!("Paths are missing:\n{}", format_bulleted_list(missing))
            )
        }
    }

    #[track_caller]
    pub fn assert_contents(&self, contents: &[(&str, Expect)]) {
        for (path, expect) in contents {
            let actual = self.contents(path).unwrap();
            expect.assert_eq(&actual);
        }
    }

    pub fn sh(&self, script: &str) -> miette::Result<Utf8Output> {
        let tempfile = tempfile::NamedTempFile::new().into_diagnostic()?;
        fs::write(
            &tempfile,
            format!(
                "set -ex\n\
                {script}"
            ),
        )
        .into_diagnostic()?;
        Command::new("bash")
            .arg(tempfile.as_ref())
            .output_checked_utf8()
            .into_diagnostic()
    }

    pub fn git(&self, directory: &str) -> Git {
        Git::from_path(Utf8PathBuf::from(directory))
    }

    /// Set up a new repository in `path` with a single commit.
    pub fn setup_repo(&self, path: &str) -> miette::Result<Utf8PathBuf> {
        let path = self.path(path);
        let path_quoted = shell_words::quote(path.as_str());
        self.sh(&format!(
            r#"
            mkdir -p {path_quoted}
            cd {path_quoted} || exit
            git init
            echo "puppy doggy" > README.md 
            git add .
            git commit -m "Initial commit"
            "#
        ))?;
        Ok(path)
    }
}
