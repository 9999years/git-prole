use std::borrow::Cow;
use std::fmt::Display;
use std::process::Command;

use camino::Utf8Path;
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
use crate::git::BranchRef;
use crate::git::Git;
use crate::git::LocalBranchRef;
use crate::AddWorktreeOpts;
use crate::PathDisplay;
use crate::Utf8Absolutize;

/// A plan for creating a new `git worktree`.
#[derive(Debug, Clone)]
pub struct WorktreePlan<'a> {
    git: AppGit<'a>,
    /// The directory to run commands from.
    worktree: Utf8PathBuf,
    destination: Utf8PathBuf,
    branch: BranchStartPointPlan,
    /// Relative paths to copy to the new worktree, if any.
    copy_untracked: Vec<Utf8PathBuf>,
}

impl<'a> WorktreePlan<'a> {
    pub fn new(git: AppGit<'a>, args: &'a AddArgs) -> miette::Result<Self> {
        // TODO: Check if there's more than 1 worktree and (offer to?) convert if not?
        // TODO: Allow user to run commands, e.g. `direnv allow`?

        let worktree = git.path().repo_root()?;
        let branch = BranchStartPointPlan::new(&git, args)?;
        let destination = Self::destination_plan(&git, args, branch.local_branch())?;
        let copy_untracked = if git.config.file.copy_untracked() {
            git.status().untracked_files()?
        } else {
            Vec::new()
        };
        Ok(Self {
            git,
            worktree,
            branch,
            destination,
            copy_untracked,
        })
    }

    fn destination_plan(
        git: &AppGit<'_>,
        args: &AddArgs,
        new_branch: &LocalBranchRef,
    ) -> miette::Result<Utf8PathBuf> {
        match &args.inner.name_or_path {
            Some(name_or_path) => {
                if name_or_path.contains('/') {
                    // Test case: `add_by_path`.
                    Utf8Path::new(name_or_path)
                        .absolutize()
                        .map(Cow::into_owned)
                } else {
                    // Test case: `add_by_name_new_local`.
                    git.worktree()
                        .container()?
                        .tap_mut(|p| p.push(name_or_path))
                        .absolutize()
                        .map(Cow::into_owned)
                }
            }
            // Test case: `add_branch_new_local`.
            None => git
                .worktree()
                .path_for(new_branch.branch_name())?
                .absolutize()
                .map(Cow::into_owned),
        }
        .into_diagnostic()
    }

    fn command(&self, git: &Git) -> Command {
        let (force_branch, track, create_branch) = match &self.branch {
            BranchStartPointPlan::Existing(_) => (false, false, None),
            BranchStartPointPlan::New {
                force,
                branch,
                start,
            } => {
                let track = matches!(start, StartPoint::Branch(_));

                (*force, track, Some(branch))
            }
        };

        git.with_directory(self.worktree.clone())
            .worktree()
            .add_command(
                &self.destination,
                &AddWorktreeOpts {
                    force_branch,
                    create_branch,
                    track,
                    start_point: Some(match &self.branch {
                        BranchStartPointPlan::Existing(branch) => branch.branch_name(),
                        BranchStartPointPlan::New { start, .. } => match start {
                            StartPoint::Branch(start) => start.qualified_branch_name(),
                            StartPoint::Commitish(commitish) => commitish,
                        },
                    }),
                    ..Default::default()
                },
            )
    }

    fn copy_untracked(&self) -> miette::Result<()> {
        if self.copy_untracked.is_empty() {
            return Ok(());
        }
        tracing::info!(
            "Copying untracked files to {}",
            self.destination.display_path_cwd()
        );
        for path in &self.copy_untracked {
            let from = self.worktree.join(path);
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
        tracing::debug!("{self:#?}");

        if self.git.config.cli.dry_run {
            tracing::info!("$ {}", Utf8ProgramAndArgs::from(&command));
        } else {
            command.status_checked()?;
            self.copy_untracked()?;
        }
        Ok(())
    }
}

impl Display for WorktreePlan<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Creating worktree in {} for {}",
            self.destination.display_path_cwd(),
            self.branch,
        )?;

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

/// Where to start a worktree at.
#[derive(Debug, Clone)]
enum StartPoint {
    /// An existing local or remote branch. The new branch should track this branch.
    Branch(BranchRef),
    /// A commit.
    Commitish(String),
}

impl StartPoint {
    pub fn new(git: &AppGit<'_>, commitish: Option<&str>) -> miette::Result<Self> {
        match commitish {
            Some(commitish) => match git.branch().local_or_remote(commitish)? {
                Some(branch) => Ok(Self::Branch(branch)),
                None => Ok(Self::Commitish(commitish.to_owned())),
            },
            None => Ok(Self::preferred(git)?),
        }
    }

    pub fn preferred(git: &AppGit) -> miette::Result<Self> {
        Ok(Self::Branch(git.preferred_branch()?))
    }
}

/// When creating a new `git worktree`, we can check out an existing branch or commit, or create a
/// new branch. Sometimes the cases are intertwined; for example, we can create a new local branch
/// tracking a remote branch.
///
/// When creating a new branch, we can either use the default branch as the starting point, track
/// an existing branch, or start at a specific commit.
#[derive(Debug, Clone)]
enum BranchStartPointPlan {
    /// Create a new branch.
    New {
        /// Whether to forcibly reset the branch if it already exists.
        force: bool,
        /// The branch to create or reset.
        branch: LocalBranchRef,
        /// The start-point for the new branch.
        start: StartPoint,
    },
    /// Check out an existing branch.
    Existing(LocalBranchRef),
}

impl BranchStartPointPlan {
    /// Create a branch and start-point plan from the given arguments.
    ///
    /// There's a lot of permutations to this functionality, so here's a big table!
    ///
    /// In general, for a fragment `NAME`, we perform the following logic:
    /// - If `NAME` is the name of a local branch, that branch is checked out.
    /// - If `NAME` is the name of a remote branch, a new local branch with the same name is
    ///   created to track the remote branch.
    /// - Otherwise, a new branch is created named `NAME` at the default starting point.
    ///   If an explicit start point is given, that's used instead.
    ///
    /// ```plain
    /// --branch | NAME_OR_PATH  | START_POINT   | behavior              | start         | test case
    /// -------- | ------------  | -----------   | ------------------    | ------------- | ---------
    /// BRANCH   | [ignored]     |               | new BRANCH            | DEFAULT       | add_branch_new
    /// BRANCH   | [ignored]     | LOCAL_BRANCH  | new BRANCH            | LOCAL_BRANCH  | add_branch_start_point_exiting_local
    /// BRANCH   | [ignored]     | REMOTE_BRANCH | new BRANCH            | REMOTE_BRANCH | add_branch_start_point_existing_remote
    /// BRANCH   | [ignored]     | COMMITISH     | new BRANCH            | COMMITISH     | add_branch_start_point_new_local
    /// -------- | ------------  | -----------   | ------------------    | ------------- | ---------
    ///          | NAME          | LOCAL_BRANCH  | existing LOCAL_BRANCH |               | add_start_point_existing_local
    ///          | NAME          | REMOTE_BRANCH | new REMOTE_BRANCH     | REMOTE_BRANCH | add_start_point_existing_remote
    ///          | NAME          | COMMITISH     | new NAME              | COMMITISH     | add_start_point_new_local
    /// -------- | ------------  | -----------   | ------------------    | ------------- | ---------
    ///          | LOCAL_BRANCH  |               | existing LOCAL_BRANCH |               | add_by_name_existing_local
    ///          | REMOTE_BRANCH |               | new REMOTE_BRANCH     | REMOTE_BRANCH | add_by_name_existing_remote
    ///          | BRANCH        |               | new BRANCH            | DEFAULT       | add_by_name_new_local
    /// ```
    ///
    /// This was very annoying to iron out, but hopefully it does what you want more of the time
    /// than `git-worktree(1)`.
    pub fn new(git: &AppGit<'_>, args: &AddArgs) -> miette::Result<Self> {
        match (&args.inner.branch, &args.inner.force_branch) {
            (Some(_), Some(_)) => Err(miette!(
                "`--branch` and `--force-branch` are mutually exclusive."
            )),
            // `add --branch BRANCH [NAME_OR_PATH [COMMITISH]]`
            (Some(branch), None) => Ok(Self::New {
                force: false,
                branch: LocalBranchRef::from(branch),
                start: StartPoint::new(git, args.commitish.as_deref())?,
            }),
            // `add --force-branch BRANCH [NAME_OR_PATH [COMMITISH]]`
            (None, Some(force_branch)) => Ok(Self::New {
                force: true,
                branch: LocalBranchRef::from(force_branch),
                start: StartPoint::new(git, args.commitish.as_deref())?,
            }),
            (None, None) => {
                let name_or_path = args
                    .inner
                    .name_or_path
                    .as_deref()
                    .expect("If `--branch` is not given, `NAME_OR_PATH` must be given");
                let dirname = git.worktree().dirname_for(name_or_path);

                match &args.commitish {
                    Some(commitish) => match Self::from_commitish(git, commitish)? {
                        // `add NAME_OR_PATH LOCAL_BRANCH`
                        // `add NAME_OR_PATH REMOTE_BRANCH`
                        Some(plan) => Ok(plan),
                        // `add NAME_OR_PATH COMMITISH`
                        None => Self::new_branch_at(git, false, dirname, Some(commitish)),
                    },

                    // `add NAME_OR_PATH`
                    None => match Self::from_commitish(git, dirname)? {
                        // `add ../puppy/LOCAL_BRANCH`
                        // `add ../puppy/REMOTE_BRANCH`
                        Some(plan) => Ok(plan),
                        // `add ../puppy/SOMETHING_ELSE`
                        None => Self::new_branch_at(git, false, dirname, None),
                    },
                }
            }
        }
    }

    fn new_branch_at(
        git: &AppGit<'_>,
        force: bool,
        branch: &str,
        commitish: Option<&str>,
    ) -> miette::Result<Self> {
        Ok(Self::New {
            force,
            branch: LocalBranchRef::new(branch.to_owned()),
            start: StartPoint::new(git, commitish)?,
        })
    }

    fn from_commitish(git: &AppGit<'_>, commitish: &str) -> miette::Result<Option<Self>> {
        Ok(git
            .branch()
            .local_or_remote(commitish)?
            .map(Self::from_branch))
    }

    fn from_branch(branch: BranchRef) -> Self {
        match branch {
            BranchRef::Local(local_branch) => Self::Existing(local_branch),
            BranchRef::Remote(remote_branch) => Self::New {
                force: false,
                branch: remote_branch.as_local(),
                start: StartPoint::Branch(remote_branch.into()),
            },
        }
    }

    /// The local branch that will be checked out or created.
    pub fn local_branch(&self) -> &LocalBranchRef {
        match self {
            BranchStartPointPlan::New { branch, .. } | BranchStartPointPlan::Existing(branch) => {
                branch
            }
        }
    }
}

impl Display for BranchStartPointPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BranchStartPointPlan::Existing(branch) => {
                write!(
                    f,
                    "{}",
                    branch
                        .branch_name()
                        .if_supports_color(Stream::Stdout, |text| text.cyan())
                )
            }
            BranchStartPointPlan::New {
                force: _,
                branch,
                start,
            } => {
                write!(
                    f,
                    "{}",
                    branch
                        .branch_name()
                        .if_supports_color(Stream::Stdout, |text| text.cyan())
                )?;
                match start {
                    StartPoint::Branch(tracking) => {
                        write!(
                            f,
                            " tracking {}",
                            tracking
                                .qualified_branch_name()
                                .if_supports_color(Stream::Stdout, |text| text.cyan())
                        )
                    }
                    StartPoint::Commitish(commitish) => {
                        write!(
                            f,
                            " starting at {}",
                            commitish.if_supports_color(Stream::Stdout, |text| text.cyan())
                        )
                    }
                }
            }
        }
    }
}
