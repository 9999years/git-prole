use std::fmt::Debug;
use std::process::Command;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use command_error::CommandExt;
use miette::IntoDiagnostic;
use tracing::instrument;

mod branch;
mod commit_hash;
mod commitish;
mod config;
mod head_state;
mod path;
mod refs;
mod remote;
mod repository_url_destination;
mod status;
mod worktree;

pub use branch::GitBranch;
pub use commit_hash::CommitHash;
pub use commitish::ResolvedCommitish;
pub use config::GitConfig;
pub use head_state::HeadKind;
pub use path::GitPath;
pub use refs::BranchRef;
pub use refs::GitRefs;
pub use refs::LocalBranchRef;
pub use refs::Ref;
pub use refs::RemoteBranchRef;
pub use remote::GitRemote;
pub use repository_url_destination::repository_url_destination;
pub use status::GitStatus;
pub use status::Status;
pub use status::StatusCode;
pub use status::StatusEntry;
pub use worktree::GitWorktree;
pub use worktree::Worktree;
pub use worktree::WorktreeHead;
pub use worktree::Worktrees;

use crate::app_git::AppGit;
use crate::config::Config;
use crate::current_dir::current_dir_utf8;

/// `git` CLI wrapper.
#[derive(Clone)]
pub struct Git {
    current_dir: Utf8PathBuf,
    env_variables: Vec<(String, String)>,
    args: Vec<String>,
}

impl Debug for Git {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Git").field(&self.current_dir).finish()
    }
}

impl Git {
    pub fn from_path(current_dir: Utf8PathBuf) -> Self {
        Self {
            current_dir,
            env_variables: Vec::new(),
            args: Vec::new(),
        }
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
        command.envs(self.env_variables.iter().map(|(key, value)| (key, value)));
        command.args(&self.args);
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
        Self {
            current_dir: path,
            env_variables: self.env_variables.clone(),
            args: self.args.clone(),
        }
    }

    pub fn env(&mut self, key: String, value: String) {
        self.env_variables.push((key, value));
    }

    pub fn envs(&mut self, iter: impl IntoIterator<Item = (String, String)>) {
        self.env_variables.extend(iter);
    }

    pub fn arg(&mut self, arg: String) {
        self.args.push(arg);
    }

    pub fn args(&mut self, iter: impl IntoIterator<Item = String>) {
        self.args.extend(iter);
    }

    /// Methods for dealing with Git remotes.
    pub fn remote(&self) -> GitRemote<'_> {
        GitRemote::new(self)
    }

    /// Methods for dealing with Git remotes.
    pub fn path(&self) -> GitPath<'_> {
        GitPath::new(self)
    }

    /// Methods for dealing with Git remotes.
    pub fn worktree(&self) -> GitWorktree<'_> {
        GitWorktree::new(self)
    }

    /// Methods for dealing with Git refs.
    pub fn refs(&self) -> GitRefs<'_> {
        GitRefs::new(self)
    }

    /// Methods for dealing with Git statuses and the working tree.
    pub fn status(&self) -> GitStatus<'_> {
        GitStatus::new(self)
    }

    /// Methods for dealing with Git statuses and the working tree.
    pub fn config(&self) -> GitConfig<'_> {
        GitConfig::new(self)
    }

    /// Methods for dealing with Git statuses and the working tree.
    pub fn branch(&self) -> GitBranch<'_> {
        GitBranch::new(self)
    }

    pub(crate) fn rev_parse_command(&self) -> Command {
        let mut command = self.command();
        command.args(["rev-parse", "--path-format=absolute"]);
        command
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

    /// `git reset`.
    #[instrument(level = "trace")]
    pub fn reset(&self) -> miette::Result<()> {
        self.command()
            .arg("reset")
            .output_checked_utf8()
            .into_diagnostic()?;
        Ok(())
    }
}
