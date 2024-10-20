use std::fmt::Debug;
use std::process::Command;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use command_error::CommandExt;
use tracing::instrument;

mod branch;
mod commit_hash;
mod commitish;
mod config;
mod git_like;
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
pub use git_like::GitLike;
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
pub use worktree::AddWorktreeOpts;
pub use worktree::GitWorktree;
pub use worktree::RenamedWorktree;
pub use worktree::ResolveUniqueNameOpts;
pub use worktree::Worktree;
pub use worktree::WorktreeHead;
pub use worktree::Worktrees;

use crate::app_git::AppGit;
use crate::config::Config;
use crate::current_dir::current_dir_utf8;

/// `git` CLI wrapper.
#[derive(Clone)]
pub struct Git<C> {
    current_dir: C,
    env_variables: Vec<(String, String)>,
    args: Vec<String>,
}

impl<C> Debug for Git<C>
where
    C: AsRef<Utf8Path>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Git")
            .field(&self.current_dir.as_ref())
            .finish()
    }
}

impl<C> AsRef<Utf8Path> for Git<C>
where
    C: AsRef<Utf8Path>,
{
    fn as_ref(&self) -> &Utf8Path {
        self.current_dir.as_ref()
    }
}

impl<C> AsRef<Git<C>> for Git<C> {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<C> GitLike for Git<C>
where
    C: AsRef<Utf8Path>,
{
    type CurrentDir = C;

    fn as_git(&self) -> &Git<Self::CurrentDir> {
        self
    }

    fn get_current_dir(&self) -> &Self::CurrentDir {
        &self.current_dir
    }
}

impl Git<Utf8PathBuf> {
    pub fn from_current_dir() -> miette::Result<Self> {
        Ok(Self::from_path(current_dir_utf8()?))
    }
}

impl<C> Git<C>
where
    C: AsRef<Utf8Path>,
{
    pub fn from_path(current_dir: C) -> Self {
        Self {
            current_dir,
            env_variables: Vec::new(),
            args: Vec::new(),
        }
    }

    pub fn with_config(self, config: &Config) -> AppGit<'_, C> {
        AppGit { git: self, config }
    }

    /// Get a `git` command.
    pub fn command(&self) -> Command {
        let mut command = Command::new("git");
        command.current_dir(self.current_dir.as_ref());
        command.envs(self.env_variables.iter().map(|(key, value)| (key, value)));
        command.args(&self.args);
        command
    }

    /// Set the current working directory for `git` commands to be run in.
    pub fn set_current_dir(&mut self, path: C) {
        self.current_dir = path;
    }

    pub fn with_current_dir<C2>(&self, path: C2) -> Git<C2> {
        Git {
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
        command.status_checked()?;
        Ok(())
    }

    /// `git reset`.
    #[instrument(level = "trace")]
    pub fn reset(&self) -> miette::Result<()> {
        self.command().arg("reset").output_checked_utf8()?;
        Ok(())
    }
}
