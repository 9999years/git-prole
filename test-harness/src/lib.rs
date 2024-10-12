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
use miette::Context;
use miette::IntoDiagnostic;

/// `git-prole` session for integration testing.
pub struct GitProle {
    command: ClonableCommand,
    tempdir: Utf8TempDir,
    git_prole: OsString,
    git_prole_args: Vec<String>,
}

impl GitProle {
    pub fn new() -> miette::Result<Self> {
        let tempdir = Utf8TempDir::new()?;

        let gitconfig = tempdir.join(".gitconfig");
        fs::write(
            &gitconfig,
            "[user]\n\
            name = Puppy Doggy\n\
            email = dog@becca.ooo\n\
            \n\
            [init]\n\
            defaultBranch = main\n\
            ",
        )
        .into_diagnostic()?;

        let git_prole = test_bin::get_test_bin("git-prole").get_program().to_owned();

        let log_filters = ["debug", "git_prole=debug", "git_prole::git=trace"]
            .into_iter()
            .join(",");

        let git_prole_args = vec!["--log".to_owned(), log_filters];

        let command = ClonableCommand::new("")
            .envs([
                // > Whether to skip reading settings from the system-wide $(prefix)/etc/gitconfig file.
                ("GIT_CONFIG_NOSYSTEM", "1"),
                ("GIT_CONFIG_GLOBAL", gitconfig.as_str()),
                ("GIT_AUTHOR_DATE", "2019-07-06T18:25:00-0700"),
                ("GIT_COMMITTER_DATE", "2019-07-06T18:25:00-0700"),
                ("HOME", tempdir.as_str()),
            ])
            .current_dir(&tempdir);

        Ok(Self {
            git_prole,
            git_prole_args,
            command,
            tempdir,
        })
    }

    fn any_command(&self, program: &str) -> Command {
        let mut command = self.command.clone();
        command.name = program.into();
        command.to_std()
    }

    pub fn cmd(&self) -> Command {
        let mut command = self.command.clone();
        command.name = self.git_prole.clone();
        command = command.args(&self.git_prole_args);
        command.to_std()
    }

    #[track_caller]
    pub fn cd_cmd(&self, current_dir: &str) -> Command {
        let path = self.path(current_dir);
        if !path.exists() {
            panic!("A test requested a command to run in a nonexistent path: {current_dir}");
        }
        let mut command = self.cmd();
        command.current_dir(self.path(current_dir));
        command
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

    pub fn sh(&self, script: &str) -> miette::Result<()> {
        let tempfile = tempfile::NamedTempFile::new().into_diagnostic()?;
        fs::write(
            &tempfile,
            format!(
                "set -ex\n\
                {script}"
            ),
        )
        .into_diagnostic()?;
        self.any_command("bash")
            .arg("--norc")
            .arg(tempfile.as_ref())
            .status_checked()
            .into_diagnostic()?;
        Ok(())
    }

    #[track_caller]
    pub fn git(&self, directory: &str) -> Git {
        let path = self.path(directory);
        if !path.exists() {
            panic!("A test requested a Git interface for a nonexistent path: {directory}");
        }
        let mut git = Git::from_path(self.path(directory));
        git.envs(self.command.environment.iter().filter_map(|(key, value)| {
            value.as_ref().map(|value| {
                (
                    key.to_owned().into_string().unwrap(),
                    value.to_owned().into_string().unwrap(),
                )
            })
        }));
        git
    }

    pub fn current_branch_in(&self, directory: &str) -> miette::Result<String> {
        Ok(self
            .git(directory)
            .branch()
            .current()?
            .ok_or_else(|| miette!("HEAD is detached in {directory}"))?
            .branch_name()
            .to_owned())
    }

    pub fn upstream_for_branch_in(&self, directory: &str, branch: &str) -> miette::Result<String> {
        Ok(self
            .git(directory)
            .branch()
            .upstream(branch)?
            .ok_or_else(|| miette!("Branch {branch} has no upstream in {directory}"))?
            .qualified_branch_name()
            .to_owned())
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

    pub fn setup_worktree_repo(&self, path: &str) -> miette::Result<()> {
        self.setup_repo(path)?;
        self.cmd()
            .current_dir(self.path(path))
            .arg("convert")
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to convert {path} to a worktree checkout"))?;

        Ok(())
    }
}
