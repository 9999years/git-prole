use std::fmt::Display;

use miette::miette;
use owo_colors::OwoColorize;
use owo_colors::Stream;
use tap::Tap;

use crate::app_git::AppGit;
use crate::format_bulleted_list::format_bulleted_list;
use crate::fs;
use crate::git::BranchRef;
use crate::git::LocalBranchRef;
use crate::normal_path::NormalPath;
use crate::utf8tempdir::Utf8TempDir;
use crate::AddWorktreeOpts;

#[derive(Debug)]
pub struct ConvertPlanOpts {
    pub default_branch: Option<String>,
}

#[derive(Debug)]
pub struct ConvertPlan<'a> {
    git: AppGit<'a>,
    repo_name: String,
    steps: Vec<Step>,
}

impl Display for ConvertPlan<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format_bulleted_list(&self.steps))
    }
}

impl<'a> ConvertPlan<'a> {
    pub fn new(git: AppGit<'a>, opts: ConvertPlanOpts) -> miette::Result<Self> {
        // Figuring out which worktrees to create is non-trivial:
        // - [x] We might already have worktrees.
        // - [x] We might have any number of remotes.
        //   Pick a reasonable & configurable default to determine the default branch.
        // - [x] We might already have the default branch checked out.
        // - [x] We might _not_ have the default branch checked out.
        // - [x] We might have unstaged/uncommitted work.
        //       TODO: The `git reset` causes staged changes to be lost; bring back the
        //       `git status push`/`pop`?
        // - [x] We might not be on _any_ branch.
        // - [x] There is no local branch for the default branch.
        //       (`convert_multiple_remotes`)

        let tempdir = NormalPath::from_cwd(Utf8TempDir::new()?.into_path())?;
        let worktrees = git.worktree().list()?;

        // TODO:
        // - toposort worktrees
        // - resolve them all into unique directory names
        if worktrees.len() != 1 {
            return Err(miette!(
                "Cannot convert a repository with multiple worktrees into a `git-prole` checkout:\n{worktrees}",
            ));
        }

        let default_branch = match opts.default_branch {
            Some(default_branch) => LocalBranchRef::new(default_branch).into(),
            None => git.preferred_branch()?,
        };
        tracing::debug!(%default_branch, "Default branch determined");
        let default_branch_dirname = git.worktree().dirname_for(default_branch.branch_name());
        let head = git.refs().head_kind()?;
        tracing::debug!(%head, "HEAD determined");
        let worktree_dirname = head.branch_name().unwrap_or("work");
        // TODO: Is this sufficient if handling multiple worktrees?
        let default_branch_is_checked_out = head.is_on_branch(default_branch.branch_name());

        // The path of the repository/main worktree before we start meddling with it.
        let repo_root = NormalPath::from_cwd(git.path().repo_root()?)?;
        let repo_name = repo_root
            .file_name()
            .ok_or_else(|| miette!("Repository has no basename: {repo_root}"))?;
        // The path of the `.git` directory before we start meddling with it.
        let repo_git_dir = NormalPath::from_cwd(git.path().git_common_dir()?)?;
        // The path where we'll put the main worktree once we're done meddling with it.
        let repo_worktree = repo_root.clone().tap_mut(|p| p.push(worktree_dirname));

        // The path in the `tempdir` where we'll place the `.git` directory while we're setting up
        // worktrees.
        let temp_git_dir = tempdir.clone().tap_mut(|p| p.push(".git"));
        // The path in the `tempdir` where we'll place the current worktree while we
        // reassociate it with the (now-bare) repository.
        let temp_worktree = tempdir.clone().tap_mut(|p| p.push(worktree_dirname));

        // I don't know, what if you have `fix/main` (not a `fix` remote, but a
        // branch named `fix/main`!) checked out, and the default branch is `main`?
        if !default_branch_is_checked_out && worktree_dirname == default_branch_dirname {
            return Err(
                miette!("Worktree directory names for default branch ({default_branch_dirname}) and current branch ({worktree_dirname}) would conflict")
            );
        }

        let mut steps = vec![
            Step::Move {
                from: repo_git_dir.clone(),
                to: temp_git_dir.clone(),
            },
            Step::SetConfig {
                repo: temp_git_dir.clone(),
                key: "core.bare".to_owned(),
                value: "true".to_owned(),
            },
            Step::Move {
                from: repo_root.clone(),
                to: temp_worktree.clone(),
            },
            Step::CreateDir {
                path: repo_root.clone(),
            },
            Step::Move {
                from: temp_git_dir.clone(),
                to: repo_git_dir.clone(),
            },
            Step::CreateWorktreeNoCheckout {
                repo: repo_git_dir.clone(),
                path: repo_worktree.clone(),
                commitish: head.commitish().to_owned(),
            },
            Step::Reset {
                repo: repo_worktree.clone(),
            },
            Step::Move {
                from: repo_worktree.clone().tap_mut(|p| p.push(".git")),
                to: temp_worktree.clone().tap_mut(|p| p.push(".git")),
            },
            Step::RemoveDirectory {
                path: repo_worktree.clone(),
            },
            Step::Move {
                from: temp_worktree.clone(),
                to: repo_worktree.clone(),
            },
        ];

        if !default_branch_is_checked_out {
            let default_branch_root = repo_root
                .clone()
                .tap_mut(|p| p.push(default_branch_dirname));

            steps.push(Step::CreateWorktree {
                repo: repo_git_dir.clone(),
                path: default_branch_root.clone(),
                branch: default_branch,
            });
        }

        Ok(Self {
            steps,
            git,
            repo_name: repo_name.to_owned(),
        })
    }

    pub fn execute(&self) -> miette::Result<()> {
        tracing::info!("{self}");

        if self.git.config.cli.dry_run {
            return Ok(());
        }

        // TODO: Ask the user before we start messing around with their repo layout!

        for step in &self.steps {
            tracing::debug!(%step, "Performing step");
            match step {
                Step::MoveWorktree { from, to, is_main } => {
                    if *is_main {
                        // The main worktree cannot be moved with `git worktree move`.
                        fs::rename(from, to)?;
                        self.git
                            .with_directory(to.as_path().to_owned())
                            .worktree()
                            .repair()?;
                    } else {
                        self.git.worktree().rename(from, to)?;
                    }
                }
                Step::CreateDir { path } => {
                    fs::create_dir_all(path)?;
                }
                Step::Move { from, to } => {
                    fs::rename(from, to)?;
                }
                Step::SetConfig { repo, key, value } => {
                    self.git
                        .with_directory(repo.as_path().to_owned())
                        .config()
                        .set(key, value)?;
                }
                Step::CreateWorktree {
                    repo: repo_root,
                    path,
                    branch,
                } => {
                    // If we're creating a worktree for a default branch from a
                    // remote, we may not have a corresponding local branch
                    // yet.
                    let (create_branch, start_point) = match branch {
                        BranchRef::Remote(remote_branch) => {
                            if self
                                .git
                                .branch()
                                .exists_local(remote_branch.branch_name())?
                            {
                                (None, &BranchRef::Local(remote_branch.as_local()))
                            } else {
                                tracing::warn!(
                                    %remote_branch,
                                    "Fetching the default branch"
                                );
                                self.git.remote().fetch(
                                    remote_branch.remote(),
                                    Some(&format!(
                                        "{:#}:{remote_branch:#}",
                                        remote_branch.as_local()
                                    )),
                                )?;
                                (Some(remote_branch.as_local()), branch)
                            }
                        }
                        BranchRef::Local(_) => (None, branch),
                    };

                    self.git
                        .with_directory(repo_root.as_path().to_owned())
                        .worktree()
                        // .add(path.as_path(), commitish.qualified_branch_name())?;
                        .add(
                            path.as_path(),
                            &AddWorktreeOpts {
                                track: create_branch.is_some(),
                                create_branch: create_branch.as_ref(),
                                start_point: Some(start_point.qualified_branch_name()),
                                ..Default::default()
                            },
                        )?;
                }
                Step::CreateWorktreeNoCheckout {
                    repo,
                    path,
                    commitish,
                } => {
                    self.git
                        .with_directory(repo.as_path().to_owned())
                        .worktree()
                        .add(
                            path,
                            &AddWorktreeOpts {
                                checkout: false,
                                start_point: Some(commitish),
                                ..Default::default()
                            },
                        )?;
                }
                Step::Reset { repo } => {
                    self.git.with_directory(repo.as_path().to_owned()).reset()?;
                }
                Step::RemoveDirectory { path } => {
                    fs::remove_dir(path)?;
                }
            }
        }

        tracing::info!(
            "{} has been converted to a worktree checkout",
            self.repo_name
        );
        tracing::info!("You may need to `cd .` to refresh your shell");

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Step {
    Move {
        from: NormalPath,
        to: NormalPath,
    },
    SetConfig {
        repo: NormalPath,
        key: String,
        value: String,
    },
    CreateWorktreeNoCheckout {
        repo: NormalPath,
        path: NormalPath,
        commitish: String,
    },
    Reset {
        repo: NormalPath,
    },
    RemoveDirectory {
        path: NormalPath,
    },
    /// Will be needed for multiple worktree support.
    #[expect(dead_code)]
    MoveWorktree {
        from: NormalPath,
        to: NormalPath,
        is_main: bool,
    },
    CreateDir {
        path: NormalPath,
    },
    CreateWorktree {
        repo: NormalPath,
        path: NormalPath,
        branch: BranchRef,
    },
}

impl Display for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Step::MoveWorktree {
                from,
                to,
                is_main: _,
            } => {
                write!(f, "Move {from} to {to}")
            }
            Step::CreateDir { path } => {
                write!(f, "Create directory {path}")
            }
            Step::CreateWorktree {
                path,
                branch: commitish,
                repo: repo_root,
            } => {
                write!(
                    f,
                    "In {repo_root}, create a worktree for {} at {path}",
                    commitish.if_supports_color(Stream::Stdout, |branch| branch.cyan())
                )
            }
            Step::SetConfig { repo, key, value } => {
                write!(
                    f,
                    "In {repo}, set {}={}",
                    key.if_supports_color(Stream::Stdout, |text| text.cyan()),
                    value.if_supports_color(Stream::Stdout, |text| text.cyan()),
                )
            }
            Step::Move { from, to } => {
                write!(f, "Move {from} to {to}")
            }
            Step::CreateWorktreeNoCheckout {
                repo,
                commitish,
                path,
            } => {
                write!(
                    f,
                    "In {repo}, create but don't check out a worktree for {} at {path}",
                    commitish.if_supports_color(Stream::Stdout, |text| text.cyan()),
                )
            }
            Step::Reset { repo } => {
                write!(f, "In {repo}, reset the index state")
            }
            Step::RemoveDirectory { path } => {
                write!(f, "Remove {path}")
            }
        }
    }
}
