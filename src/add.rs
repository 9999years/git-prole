use std::fmt::Display;
use std::process::Command;

use camino::Utf8PathBuf;
use command_error::CommandExt;
use command_error::Utf8ProgramAndArgs;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;
use owo_colors::OwoColorize;
use owo_colors::Stream;
use tap::Tap;

use crate::app_git::AppGit;
use crate::cli::AddArgs;
use crate::format_bulleted_list::format_bulleted_list;
use crate::git::Git;
use crate::normal_path::NormalPath;

/// A plan for creating a new `git worktree`.
#[derive(Debug, Clone)]
pub struct WorktreePlan<'a> {
    git: AppGit<'a>,
    /// The directory to run commands from.
    repo_root: Utf8PathBuf,
    branch: BranchPlan,
    destination: NormalPath,
    start_point: StartPointPlan,
    /// Relative paths to copy to the new worktree, if any.
    copy_untracked: Vec<Utf8PathBuf>,
}

impl<'a> WorktreePlan<'a> {
    pub fn new(git: AppGit<'a>, args: &'a AddArgs) -> miette::Result<Self> {
        // TODO: Check if there's more than 1 worktree and (offer to?) convert if not?
        // TODO: Allow user to run commands, e.g. `direnv allow`?

        let repo_root = git.repo_root()?;
        let branch = BranchPlan::new(&git, args)?;
        let start_point = StartPointPlan::new(&git, args, &branch)?;
        let destination = Self::destination_plan(&git, args, &branch)?;
        let copy_untracked = if git.config.file.copy_untracked() {
            git.untracked_files()?
        } else {
            Vec::new()
        };
        Ok(Self {
            git,
            repo_root,
            branch,
            destination,
            start_point,
            copy_untracked,
        })
    }

    fn destination_plan(
        git: &AppGit<'_>,
        args: &AddArgs,
        branch_plan: &BranchPlan,
    ) -> miette::Result<NormalPath> {
        match &args.inner.name_or_path {
            Some(name_or_path) => {
                if name_or_path.contains('/') {
                    NormalPath::from_cwd(name_or_path)
                } else {
                    NormalPath::from_cwd(
                        git.worktree_container()?.tap_mut(|p| p.push(name_or_path)),
                    )
                }
            }
            None => NormalPath::from_cwd(git.branch_path(branch_plan.branch_name())?),
        }
    }

    fn command(&self, git: &Git) -> Command {
        let mut command = git.with_directory(self.repo_root.clone()).command();
        command.args(["worktree", "add"]);

        match &self.branch {
            BranchPlan::New(branch) => {
                command.args(["-b", branch]);
            }
            BranchPlan::NewForce(branch) => {
                command.args(["-B", branch]);
            }
            BranchPlan::Existing(_) => {
                // TODO: Do we need `--track` here?
            }
        }

        command.args([self.destination.as_str(), self.start_point.commitish()]);

        command
    }

    fn copy_untracked(&self) -> miette::Result<()> {
        if self.copy_untracked.is_empty() {
            return Ok(());
        }
        tracing::info!("Copying untracked files to {}", self.destination);
        for path in &self.copy_untracked {
            let from = self.repo_root.join(path);
            let to = self.destination.join(path);
            tracing::trace!(
                %path,
                %from, %to,
                "Copying untracked file"
            );
            let errors = crate::copy_dir::copy_dir(&from, &to)
                .into_diagnostic()
                .wrap_err_with(|| format!("Failed to copy untracked files from {from} to {to}"))?;
            if !errors.is_empty() {
                tracing::debug!(
                    "Errors encountered while copying untracked files:\n{}",
                    format_bulleted_list(errors)
                );
            }
        }
        Ok(())
    }

    pub fn execute(&self) -> miette::Result<()> {
        let mut command = self.command(&self.git);

        tracing::info!("{self}");

        if self.git.config.cli.dry_run {
            tracing::info!("$ {}", Utf8ProgramAndArgs::from(&command));
        } else {
            command.status_checked().into_diagnostic()?;
            self.copy_untracked()?;
        }
        Ok(())
    }
}

impl Display for WorktreePlan<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.branch {
            BranchPlan::Existing(_) => {
                write!(
                    f,
                    "Creating worktree in {} for {}",
                    self.destination, self.branch,
                )?;
            }
            _ => {
                write!(
                    f,
                    "Creating worktree in {} for {} starting at {}",
                    self.destination, self.branch, self.start_point,
                )?;
            }
        }

        if !self.copy_untracked.is_empty() {
            write!(
                f,
                "\nCopying {} untracked paths to new worktree",
                self.copy_untracked.len()
            )?;
        }

        Ok(())
    }
}

/// The branch to checkout or create for a new `git worktree`.
#[derive(Debug, Clone)]
enum BranchPlan {
    /// Create a new branch with `-b`.
    New(String),
    /// Create (and forcibly reset) a new branch with `-B`.
    NewForce(String),
    /// Use an existing local or remote branch.
    Existing(String),
}

impl BranchPlan {
    pub fn new(git: &AppGit<'_>, args: &AddArgs) -> miette::Result<Self> {
        match (&args.inner.branch, &args.inner.force_branch) {
            (Some(_), Some(_)) => Err(miette!(
                "`--branch` and `--force-branch` are mutually exclusive."
            )),
            (Some(branch), None) => Ok(Self::New(branch.to_owned())),
            (None, Some(force_branch)) => Ok(Self::NewForce(force_branch.to_owned())),
            (None, None) => {
                let name_or_path = args
                    .inner
                    .name_or_path
                    .as_deref()
                    .expect("If `--branch` is not given, `NAME_OR_PATH` must be given");
                let branch = Git::branch_dirname(name_or_path);
                if git.local_branch_exists(branch)? {
                    Ok(Self::Existing(branch.to_owned()))
                } else if let Some(remote) = git.find_remote_for_branch(branch)? {
                    // This is implicit behavior documented in `git-worktree(1)`.
                    Ok(Self::Existing(format!("{remote}/{branch}")))
                } else {
                    // Otherwise, create a new branch with the given name.
                    Ok(Self::New(branch.to_owned()))
                }
            }
        }
    }

    pub fn branch_name(&self) -> &str {
        match self {
            BranchPlan::New(branch_name)
            | BranchPlan::NewForce(branch_name)
            | BranchPlan::Existing(branch_name) => branch_name,
        }
    }
}

impl Display for BranchPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BranchPlan::New(branch) | BranchPlan::NewForce(branch) => {
                write!(
                    f,
                    "new branch {}",
                    branch.if_supports_color(Stream::Stdout, |text| text.cyan())
                )
            }
            BranchPlan::Existing(branch) => {
                write!(
                    f,
                    "branch {}",
                    branch.if_supports_color(Stream::Stdout, |text| text.cyan())
                )
            }
        }
    }
}

/// The commit or branch to start a new `git worktree` at.
#[derive(Debug, Clone)]
enum StartPointPlan {
    /// Check out an existing branch.
    Existing(String),
    /// Use the default branch.
    Default(String),
    /// The user specified a start point explicitly.
    Explicit(String),
}

impl StartPointPlan {
    pub fn new(git: &AppGit<'_>, args: &AddArgs, branch_plan: &BranchPlan) -> miette::Result<Self> {
        match &args.commitish {
            Some(commitish) => Ok(Self::Explicit(commitish.to_owned())),
            None => match branch_plan {
                BranchPlan::New(_) | BranchPlan::NewForce(_) => {
                    Ok(Self::Default(git.preferred_branch()?))
                }
                BranchPlan::Existing(branch) => Ok(Self::Existing(branch.clone())),
            },
        }
    }

    pub fn commitish(&self) -> &str {
        match self {
            StartPointPlan::Existing(commitish)
            | StartPointPlan::Default(commitish)
            | StartPointPlan::Explicit(commitish) => commitish,
        }
    }
}

impl Display for StartPointPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartPointPlan::Existing(branch) => {
                write!(
                    f,
                    "branch {}",
                    branch.if_supports_color(Stream::Stdout, |text| text.cyan())
                )
            }
            StartPointPlan::Default(branch) => {
                write!(
                    f,
                    "default branch {}",
                    branch.if_supports_color(Stream::Stdout, |text| text.cyan())
                )
            }
            StartPointPlan::Explicit(commitish) => {
                write!(
                    f,
                    "{}",
                    commitish.if_supports_color(Stream::Stdout, |text| text.cyan())
                )
            }
        }
    }
}
