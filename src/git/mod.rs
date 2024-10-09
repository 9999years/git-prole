use std::collections::HashSet;
use std::fmt::Debug;
use std::process::Command;
use std::str::FromStr;
use std::sync::OnceLock;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use command_error::CommandExt;
use command_error::OutputContext;
use commitish::ResolvedCommitish;
use head_state::HeadKind;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;
use ref_name::Ref;
use regex::Regex;
use status::Status;
use tap::Tap;
use tracing::instrument;
use utf8_command::Utf8Output;

pub mod commit_hash;
pub mod commitish;
pub mod head_state;
pub mod ref_name;
pub mod repository_url_destination;
pub mod status;
pub mod worktree;

use commit_hash::CommitHash;
use worktree::Worktrees;

use crate::app_git::AppGit;
use crate::config::Config;
use crate::current_dir::current_dir_utf8;

/// `git` CLI wrapper.
#[derive(Clone)]
pub struct Git {
    current_dir: Utf8PathBuf,
}

impl Debug for Git {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Git").field(&self.current_dir).finish()
    }
}

impl Git {
    pub fn from_path(current_dir: Utf8PathBuf) -> Self {
        Self { current_dir }
    }

    pub fn from_current_dir() -> miette::Result<Self> {
        Ok(Self::from_path(current_dir_utf8()?))
    }

    pub fn with_config(self, config: &Config) -> AppGit<'_> {
        AppGit { git: self, config }
    }

    /// Get a `git` command.
    pub fn command(&self) -> Command {
        let mut command = Command::new("git");
        command.current_dir(&self.current_dir);
        command
    }

    pub fn get_directory(&self) -> &Utf8Path {
        &self.current_dir
    }

    /// Set the current working directory for `git` commands to be run in.
    pub fn set_directory(&mut self, path: Utf8PathBuf) {
        self.current_dir = path;
    }

    pub fn with_directory(&self, path: Utf8PathBuf) -> Self {
        let mut ret = self.clone();
        ret.set_directory(path);
        ret
    }

    fn rev_parse_command(&self) -> Command {
        let mut command = self.command();
        command.args(["rev-parse", "--path-format=absolute"]);
        command
    }

    /// `git rev-parse --show-toplevel`
    #[instrument(level = "trace")]
    pub fn repo_root(&self) -> miette::Result<Utf8PathBuf> {
        Ok(self
            .rev_parse_command()
            .arg("--show-toplevel")
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err("Failed to get working directory of repository")?
            .stdout
            .trim()
            .into())
    }

    /// Get a list of all `git remote`s.
    #[instrument(level = "trace")]
    pub fn remotes(&self) -> miette::Result<Vec<String>> {
        Ok(self
            .command()
            .arg("remote")
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err("Failed to list Git remotes")?
            .stdout
            .lines()
            .map(|line| line.to_owned())
            .collect())
    }

    /// Get the (push) URL for the given remote.
    #[expect(dead_code)] // #[instrument(level = "trace")]
    pub fn remote_url(&self, remote: &str) -> miette::Result<String> {
        Ok(self
            .command()
            .args(["remote", "get-url", "--push", remote])
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err("Failed to get Git remote URL")?
            .stdout
            .trim()
            .to_owned())
    }

    #[instrument(level = "trace")]
    fn default_branch_symbolic_ref(&self, remote: &str) -> miette::Result<String> {
        let output = self
            .command()
            .args([
                "symbolic-ref",
                "--short",
                &format!("refs/remotes/{remote}/HEAD"),
            ])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout;

        static RE: OnceLock<Regex> = OnceLock::new();
        let captures = RE
            .get_or_init(|| {
                Regex::new(
                    r"(?xm)
                    ^
                    (?P<remote>[[:word:]]+)/(?P<branch>.+)
                    $
                    ",
                )
                .expect("Regex parses")
            })
            .captures(&output);

        match captures {
            Some(captures) => Ok(captures["branch"].to_owned()),
            None => Err(miette!(
                "Could not parse `git symbolic-ref` output:\n{output}"
            )),
        }
    }

    #[instrument(level = "trace")]
    fn default_branch_ls_remote(&self, remote: &str) -> miette::Result<String> {
        let output = self
            .command()
            .args(["ls-remote", "--symref", remote, "HEAD"])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout;

        static RE: OnceLock<Regex> = OnceLock::new();
        let captures = RE
            .get_or_init(|| {
                Regex::new(
                    r"(?xm)
                    ^
                    ref:\ refs/heads/(?P<branch>[^\t]+)\tHEAD
                    $
                    ",
                )
                .expect("Regex parses")
            })
            .captures(&output);

        let branch = match captures {
            Some(captures) => Ok(captures["branch"].to_owned()),
            None => Err(miette!("Could not parse `git ls-remote` output:\n{output}")),
        }?;

        // To avoid talking to the remote next time, write a symbolic-ref.
        self.command()
            .args([
                "symbolic-ref",
                &format!("refs/remotes/{remote}/HEAD"),
                &format!("refs/remotes/{remote}/{branch}"),
            ])
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err_with(|| {
                format!("Failed to store symbolic ref for default branch for remote {remote}")
            })?;

        Ok(branch)
    }

    #[instrument(level = "trace")]
    pub fn default_branch(&self, remote: &str) -> miette::Result<String> {
        self.default_branch_symbolic_ref(remote).or_else(|err| {
            tracing::debug!("Failed to get default branch: {err}");
            self.default_branch_ls_remote(remote)
        })
    }

    #[expect(dead_code)] // #[instrument(level = "trace")]
    pub fn commit_message(&self, commit: &str) -> miette::Result<String> {
        Ok(self
            .command()
            .args(["show", "--no-patch", "--format=%B", commit])
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err("Failed to get commit message")?
            .stdout)
    }

    /// Get the `HEAD` commit hash.
    #[instrument(level = "trace")]
    pub fn get_head(&self) -> miette::Result<CommitHash> {
        Ok(self.rev_parse("HEAD")?.expect("HEAD always exists"))
    }

    /// Get the `.git` directory path.
    #[expect(dead_code)] // #[instrument(level = "trace")]
    pub fn get_git_dir(&self) -> miette::Result<Utf8PathBuf> {
        self.rev_parse_command()
            .arg("--git-dir")
            .output_checked_utf8()
            .into_diagnostic()
            .map(|output| Utf8PathBuf::from(output.stdout.trim()))
    }

    /// Get the common `.git` directory for all worktrees.
    #[instrument(level = "trace")]
    pub fn git_common_dir(&self) -> miette::Result<Utf8PathBuf> {
        self.rev_parse_command()
            .arg("--git-common-dir")
            .output_checked_utf8()
            .into_diagnostic()
            .map(|output| Utf8PathBuf::from(output.stdout.trim()))
    }

    /// Parse a `commitish` into a commit hash.
    #[instrument(level = "trace")]
    pub fn rev_parse(&self, commitish: &str) -> miette::Result<Option<CommitHash>> {
        self.rev_parse_command()
            .args(["--verify", "--quiet", "--end-of-options", commitish])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if context.status().success() {
                    Ok::<_, command_error::Error>(Some(CommitHash::new(
                        context.output().stdout.trim().to_owned(),
                    )))
                } else {
                    Ok(None)
                }
            })
            .into_diagnostic()
    }

    /// `git rev-parse --symbolic-full-name`
    #[instrument(level = "trace")]
    pub fn rev_parse_symbolic_full_name(&self, commitish: &str) -> miette::Result<Option<Ref>> {
        self.rev_parse_command()
            .args([
                "--symbolic-full-name",
                "--verify",
                "--quiet",
                "--end-of-options",
                commitish,
            ])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if context.status().success() {
                    let trimmed = context.output().stdout.trim();
                    if trimmed.is_empty() {
                        Ok(None)
                    } else {
                        Ref::from_str(trimmed)
                            .map(Some)
                            .map_err(|err| context.error_msg(err))
                    }
                } else {
                    Ok(None)
                }
            })
            .into_diagnostic()
    }

    /// Determine if a given `<commit-ish>` refers to a commit or a symbolic ref name.
    #[expect(dead_code)] // #[instrument(level = "trace")]
    pub fn resolve_commitish(&self, commitish: &str) -> miette::Result<ResolvedCommitish> {
        match self.rev_parse_symbolic_full_name(commitish)? {
            Some(ref_name) => Ok(ResolvedCommitish::Ref(ref_name)),
            None => Ok(ResolvedCommitish::Commit(
                self.rev_parse(commitish)?.ok_or_else(|| {
                    miette!("Commitish could not be resolved to a ref or commit hash: {commitish}")
                })?,
            )),
        }
    }

    /// Get the 'main' worktree. There can only be one main worktree, and it contains the
    /// common `.git` directory.
    ///
    /// See: <https://stackoverflow.com/a/68754000>
    #[instrument(level = "trace")]
    pub fn main_worktree(&self) -> miette::Result<Utf8PathBuf> {
        let mut worktree = self.git_common_dir()?;
        // This seems incredibly buggy, given that bare checkouts are a thing and Git has
        // mechanisms for keeping the `.git` directory and the working tree in different
        // places, but it's what the Git source code does!
        //
        // See: https://github.com/git/git/blob/90fe3800b92a49173530828c0a17951abd30f0e1/worktree.c#L76
        // See: https://stackoverflow.com/a/21085415
        if worktree.ends_with(".git") {
            worktree.pop();
        }
        Ok(worktree)
    }

    /// Get the worktree container directory.
    ///
    /// This is the main worktree's parent, and is usually where all the other worktrees are cloned
    /// as well.
    #[instrument(level = "trace")]
    pub fn worktree_container(&self) -> miette::Result<Utf8PathBuf> {
        // TODO: Write `.git-prole` to indicate worktree container root?
        let mut container = self.main_worktree()?;
        if !container.pop() {
            Err(miette!("Main worktree path has no parent: {container}"))
        } else {
            Ok(container)
        }
    }

    /// List Git worktrees.
    #[instrument(level = "trace")]
    pub fn worktree_list(&self) -> miette::Result<Worktrees> {
        Worktrees::from_git(self)
    }

    #[instrument(level = "trace")]
    pub fn is_head_detached(&self) -> miette::Result<bool> {
        let output = self
            .command()
            .args(["symbolic-ref", "--quiet", "HEAD"])
            .output_checked_with_utf8::<String>(|_output| Ok(()))
            .into_diagnostic()?;

        Ok(!output.status.success())
    }

    #[expect(dead_code)] // #[instrument(level = "trace")]
    pub fn status(&self) -> miette::Result<Status> {
        self.command()
            .args(["status", "--porcelain=v1", "--ignored=traditional", "-z"])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if context.status().success() {
                    Status::from_str(&context.output().stdout).map_err(|err| context.error_msg(err))
                } else {
                    Err(context.error())
                }
            })
            .into_diagnostic()
    }

    /// Figure out what's going on with `HEAD`.
    #[instrument(level = "trace")]
    pub fn head_kind(&self) -> miette::Result<HeadKind> {
        Ok(if self.is_head_detached()? {
            HeadKind::Detached(self.get_head()?)
        } else {
            HeadKind::Ref(
                self.rev_parse_symbolic_full_name("HEAD")?
                    .expect("HEAD should always be a valid ref"),
            )
        })
    }

    /// List untracked files and directories.
    #[instrument(level = "trace")]
    pub fn untracked_files(&self) -> miette::Result<Vec<Utf8PathBuf>> {
        Ok(self
            .command()
            .args([
                "ls-files",
                // Show untracked (e.g. ignored) files.
                "--others",
                // If a whole directory is classified as other, show just its name and not its
                // whole contents.
                "--directory",
                "-z",
            ])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(Utf8PathBuf::from)
            .collect())
    }

    /// Lists local branches.
    #[instrument(level = "trace")]
    pub fn list_local_branches(&self) -> miette::Result<HashSet<String>> {
        Ok(self
            .command()
            .args(["branch", "--format=%(refname:short)"])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout
            .lines()
            .map(|line| line.to_owned())
            .collect())
    }

    #[instrument(level = "trace")]
    pub fn local_branch_exists(&self, branch: &str) -> miette::Result<bool> {
        self.command()
            .args(["show-ref", "--quiet", "--branches", branch])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                Ok::<_, command_error::Error>(context.status().success())
            })
            .into_diagnostic()
    }

    /// Get the `checkout.defaultRemote` setting.
    #[instrument(level = "trace")]
    pub fn default_remote(&self) -> miette::Result<Option<String>> {
        self.get_config("checkout.defaultRemote")
    }

    /// Find a unique remote branch by name.
    ///
    /// The discovered remote, if any, is returned.
    ///
    /// This is (hopefully!) how Git determines which remote-tracking branch you want when you do a
    /// `git switch` or `git worktree add`.
    #[instrument(level = "trace")]
    pub fn find_remote_for_branch(&self, branch: &str) -> miette::Result<Option<String>> {
        let refs = self
            .command()
            .args([
                "for-each-ref",
                "--format=%(refname)",
                &format!("refs/remotes/*/{branch}"),
            ])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout;

        let mut exists_on_remotes = Vec::new();

        for ref_name in refs.lines() {
            let parsed_ref = Ref::from_str(ref_name)?;
            match parsed_ref.remote_and_branch() {
                Some((remote, _branch)) => {
                    exists_on_remotes.push(remote.to_owned());
                }
                None => {
                    unreachable!()
                }
            }
        }

        if exists_on_remotes.is_empty() {
            Ok(None)
        } else if exists_on_remotes.len() == 1 {
            Ok(exists_on_remotes.pop())
        } else if let Some(default_remote) = self.default_remote()? {
            // if-let chains when?
            if exists_on_remotes.contains(&default_remote) {
                Ok(Some(default_remote))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    #[instrument(level = "trace")]
    pub fn worktree_add(&self, path: &Utf8Path, commitish: &str) -> miette::Result<()> {
        self.command()
            .args(["worktree", "add", path.as_str(), commitish])
            .status_checked()
            .into_diagnostic()?;
        Ok(())
    }

    #[instrument(level = "trace")]
    pub fn worktree_add_no_checkout(&self, path: &Utf8Path, commitish: &str) -> miette::Result<()> {
        self.command()
            .args(["worktree", "add", "--no-checkout", path.as_str(), commitish])
            .status_checked()
            .into_diagnostic()?;
        Ok(())
    }

    #[instrument(level = "trace")]
    pub fn worktree_move(&self, from: &Utf8Path, to: &Utf8Path) -> miette::Result<()> {
        self.command()
            .current_dir(from)
            .args(["worktree", "move", from.as_str(), to.as_str()])
            .status_checked()
            .into_diagnostic()?;
        Ok(())
    }

    #[instrument(level = "trace")]
    pub fn worktree_repair(&self) -> miette::Result<()> {
        self.command()
            .args(["worktree", "repair"])
            .status_checked()
            .into_diagnostic()?;
        Ok(())
    }

    #[instrument(level = "trace")]
    pub fn clone_repository(
        &self,
        repository: &str,
        destination: Option<&Utf8Path>,
        args: &[String],
    ) -> miette::Result<()> {
        let mut command = self.command();
        command.arg("clone").args(args).arg(repository);
        if let Some(destination) = destination {
            command.arg(destination);
        }
        command.status_checked().into_diagnostic()?;
        Ok(())
    }

    /// Get a config setting by name.
    #[instrument(level = "trace")]
    pub fn get_config(&self, key: &str) -> miette::Result<Option<String>> {
        self.command()
            .args(["config", "get", "--null", key])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if context.status().success() {
                    match context.output().stdout.as_str().split_once('\0') {
                        Some((value, rest)) => {
                            if !rest.is_empty() {
                                tracing::warn!(
                                    %key,
                                    data=rest,
                                    "Trailing data in `git config` output"
                                );
                            }
                            Ok(Some(value.to_owned()))
                        }
                        None => Err(context.error_msg("Output didn't contain any null bytes")),
                    }
                } else if let Some(1) = context.status().code() {
                    Ok(None)
                } else {
                    Err(context.error())
                }
            })
            .into_diagnostic()
    }

    /// Set a local config setting.
    #[instrument(level = "trace")]
    pub fn set_config(&self, key: &str, value: &str) -> miette::Result<()> {
        self.command()
            .args(["config", "set", key, value])
            .output_checked_utf8()
            .into_diagnostic()?;
        Ok(())
    }

    /// `git reset`.
    #[instrument(level = "trace")]
    pub fn reset(&self) -> miette::Result<()> {
        self.command()
            .arg("reset")
            .output_checked_utf8()
            .into_diagnostic()?;
        Ok(())
    }

    /// The directory name, nested under the worktree parent directory, where the given
    /// branch's worktree will be placed.
    ///
    /// E.g. to convert a repo `~/puppy` with default branch `main`, this will return `main`,
    /// to indicate a worktree to be placed in `~/puppy/main`.
    ///
    /// TODO: Should support some configurable regex filtering or other logic?
    pub fn branch_dirname(branch: &str) -> &str {
        match branch.rsplit_once('/') {
            Some((_left, right)) => {
                tracing::warn!(
                    %branch,
                    worktree = %right,
                    "Branch contains a `/`, using trailing component for worktree directory name"
                );
                right
            }
            None => branch,
        }
    }

    /// Get the full path for a new worktree with the given branch name.
    ///
    /// This appends the [`Self::branch_dirname`] to the [`Git::worktree_container`].
    #[instrument(level = "trace")]
    pub fn branch_path(&self, branch: &str) -> miette::Result<Utf8PathBuf> {
        Ok(self
            .worktree_container()?
            .tap_mut(|p| p.push(Self::branch_dirname(branch))))
    }
}
