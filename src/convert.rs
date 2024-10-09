use std::fmt::Display;

use camino::Utf8PathBuf;
use fs_err as fs;
use miette::miette;
use miette::IntoDiagnostic;
use owo_colors::OwoColorize;
use owo_colors::Stream;
use tap::Tap;

use crate::app::App;
use crate::format_bulleted_list::format_bulleted_list;
use crate::git::Git;
use crate::normal_path::NormalPath;
use crate::utf8tempdir::Utf8TempDir;

#[derive(Debug)]
pub struct ConvertPlanOpts {
    pub repository: Utf8PathBuf,
    pub default_branch: Option<String>,
}

#[derive(Debug)]
pub struct ConvertPlan {
    repo_name: String,
    steps: Vec<Step>,
    git: Git,
}

impl Display for ConvertPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format_bulleted_list(&self.steps))
    }
}

impl ConvertPlan {
    pub fn new(app: &App, opts: ConvertPlanOpts) -> miette::Result<Self> {
        // Figuring out which worktrees to create is non-trivial:
        // - We might already have worktrees.
        // - We might have any number of remotes.
        //   Pick a reasonable & configurable default to determine the default branch.
        // - We might already have the default branch checked out.
        // - We might _not_ have the default branch checked out.
        // - We might have unstaged/uncommitted work.
        // - We might not be on _any_ branch.

        let tempdir = NormalPath::from_cwd(Utf8TempDir::new()?.into_path())?;
        let git = app.git.with_directory(opts.repository);
        let worktrees = git.worktree_list()?;

        // TODO:
        // - toposort worktrees
        // - resolve them all into unique directory names
        if worktrees.len() != 1 {
            return Err(miette!(
                "Cannot convert a repository with multiple worktrees into a `git-prole` checkout:\n{worktrees}",
            ));
        }

        let default_branch = match opts.default_branch {
            Some(default_branch) => default_branch,
            None => app.pick_default_branch()?,
        };
        let default_branch_dirname = App::branch_dirname(&default_branch);
        let head = git.head_state()?;
        let worktree_dirname = head.branch_name().unwrap_or("work");
        // TODO: Is this sufficient if handling multiple worktrees?
        let default_branch_is_checked_out = head.is_on_branch(&default_branch);

        // The path of the repository/main worktree before we start meddling with it.
        let repo_root = NormalPath::from_cwd(git.repo_root()?)?;
        let repo_name = repo_root
            .file_name()
            .ok_or_else(|| miette!("Repository has no basename: {repo_root}"))?;
        // The path of the `.git` directory before we start meddling with it.
        let repo_git_dir = NormalPath::from_cwd(git.git_common_dir()?)?;
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

        let mut steps = Vec::new();

        steps.push(Step::MoveGitDir {
            from: repo_git_dir.clone(),
            to: temp_git_dir.clone(),
        });

        steps.push(Step::SetConfig {
            repo: temp_git_dir.clone(),
            key: "core.bare".to_owned(),
            value: "true".to_owned(),
        });

        steps.push(Step::Move {
            from: repo_root.clone(),
            to: temp_worktree.clone(),
        });

        steps.push(Step::CreateDir {
            path: repo_root.clone(),
        });

        steps.push(Step::Move {
            from: temp_git_dir.clone(),
            to: repo_git_dir.clone(),
        });

        steps.push(Step::CreateWorktreeNoCheckout {
            repo: repo_git_dir.clone(),
            path: repo_worktree.clone(),
            commitish: head.commitish().to_owned(),
        });

        steps.push(Step::Reset {
            repo: repo_worktree.clone(),
        });

        steps.push(Step::Move {
            from: repo_worktree.clone().tap_mut(|p| p.push(".git")),
            to: temp_worktree.clone().tap_mut(|p| p.push(".git")),
        });

        steps.push(Step::RemoveDirectory {
            path: repo_worktree.clone(),
        });

        steps.push(Step::Move {
            from: temp_worktree.clone(),
            to: repo_worktree.clone(),
        });

        if !default_branch_is_checked_out {
            let default_branch_root = repo_root
                .clone()
                .tap_mut(|p| p.push(default_branch_dirname));

            steps.push(Step::CreateWorktree {
                repo: repo_git_dir.clone(),
                path: default_branch_root.clone(),
                commitish: default_branch,
            });
        }

        Ok(Self {
            steps,
            git,
            repo_name: repo_name.to_owned(),
        })
    }

    pub fn execute(&self) -> miette::Result<()> {
        for step in &self.steps {
            tracing::debug!(%step, "Performing step");
            match step {
                Step::MoveWorktree { from, to, is_main } => {
                    if *is_main {
                        // The main worktree cannot be moved with `git worktree move`.
                        fs::rename(from, to).into_diagnostic()?;
                        self.git
                            .with_directory(to.as_path().to_owned())
                            .worktree_repair()?;
                    } else {
                        self.git.worktree_move(from, to)?;
                    }
                }
                Step::StashPush { repo_root } => self
                    .git
                    .with_directory(repo_root.as_path().to_owned())
                    .stash_push()?,
                Step::Switch { repo_root, branch } => self
                    .git
                    .with_directory(repo_root.as_path().to_owned())
                    .switch(branch)?,
                Step::CreateDir { path } => {
                    fs::create_dir_all(path).into_diagnostic()?;
                }
                Step::CreateWorktree {
                    repo: repo_root,
                    path,
                    commitish,
                } => {
                    self.git
                        .with_directory(repo_root.as_path().to_owned())
                        .worktree_add(path.as_path(), commitish)?;
                }
                Step::StashPop { repo_root } => {
                    self.git
                        .with_directory(repo_root.as_path().to_owned())
                        .stash_pop()?;
                }
                Step::MoveGitDir { from, to } => {
                    fs::rename(from, to).into_diagnostic()?;
                }
                Step::Move { from, to } => {
                    fs::rename(from, to).into_diagnostic()?;
                }
                Step::SetConfig { repo, key, value } => {
                    self.git
                        .with_directory(repo.as_path().to_owned())
                        .set_config(key, value)?;
                }
                Step::CreateWorktreeNoCheckout {
                    repo,
                    path,
                    commitish,
                } => {
                    self.git
                        .with_directory(repo.as_path().to_owned())
                        .worktree_add_no_checkout(path, commitish)?;
                }
                Step::Reset { repo } => {
                    self.git.with_directory(repo.as_path().to_owned()).reset()?;
                }
                Step::RemoveDirectory { path } => {
                    fs::remove_dir(path).into_diagnostic()?;
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
    MoveGitDir {
        from: NormalPath,
        to: NormalPath,
    },
    SetConfig {
        repo: NormalPath,
        key: String,
        value: String,
    },
    Move {
        from: NormalPath,
        to: NormalPath,
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
    MoveWorktree {
        from: NormalPath,
        to: NormalPath,
        is_main: bool,
    },
    StashPush {
        repo_root: NormalPath,
    },
    Switch {
        repo_root: NormalPath,
        branch: String,
    },
    CreateDir {
        path: NormalPath,
    },
    CreateWorktree {
        repo: NormalPath,
        path: NormalPath,
        commitish: String,
    },
    StashPop {
        repo_root: NormalPath,
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
            Step::StashPush { repo_root } => {
                write!(f, "In {repo_root}, stash changes")
            }
            Step::Switch { repo_root, branch } => {
                write!(
                    f,
                    "In {repo_root}, switch to branch {}",
                    branch.if_supports_color(Stream::Stdout, |branch| branch.cyan())
                )
            }
            Step::CreateDir { path } => {
                write!(f, "Create directory {path}")
            }
            Step::CreateWorktree {
                path,
                commitish,
                repo: repo_root,
            } => {
                write!(
                    f,
                    "In {repo_root}, create a worktree for {} at {path}",
                    commitish.if_supports_color(Stream::Stdout, |branch| branch.cyan())
                )
            }
            Step::StashPop { repo_root } => {
                write!(f, "In {repo_root}, restore changes")
            }
            Step::MoveGitDir { from, to } => {
                write!(f, "Move {from} to {to}")
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
