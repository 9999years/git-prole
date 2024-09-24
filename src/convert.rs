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

        let repo_root = NormalPath::from_cwd(git.repo_root()?)?;
        let repo_name = repo_root
            .file_name()
            .ok_or_else(|| miette!("Repository has no basename: {repo_root}"))?;
        let worktrees = git.worktree_list()?;
        let temp_repo_dir = tempdir.clone().tap_mut(|p| p.push(repo_name));

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

        // I don't know, what if you have `fix/main` (not a `fix` remote, but a
        // branch named `fix/main`!) checked out, and the default branch is `main`?
        if !default_branch_is_checked_out && worktree_dirname == default_branch_dirname {
            return Err(
                miette!("Worktree directory names for default branch ({default_branch_dirname}) and current branch ({worktree_dirname}) would conflict")
            );
        }

        let new_root = repo_root
            .clone()
            .tap_mut(|p| p.push(default_branch_dirname));

        let worktree_dir = repo_root.clone().tap_mut(|p| p.push(worktree_dirname));

        let mut steps = Vec::new();

        if !default_branch_is_checked_out {
            if !head.is_clean() {
                steps.push(Step::StashPush {
                    repo_root: repo_root.clone(),
                });
            }

            steps.push(Step::Switch {
                repo_root: repo_root.clone(),
                branch: default_branch.clone(),
            });
        }

        steps.push(Step::MoveWorktree {
            from: repo_root.clone(),
            to: temp_repo_dir.clone(),
            // This will change when we support multiple worktrees!
            is_main: true,
        });

        steps.push(Step::CreateDir {
            path: repo_root.clone(),
        });
        steps.push(Step::MoveWorktree {
            from: temp_repo_dir.clone(),
            to: new_root.clone(),
            // This will change when we support multiple worktrees!
            is_main: true,
        });

        if !default_branch_is_checked_out {
            steps.push(Step::CreateWorktree {
                repo_root: new_root.clone(),
                path: worktree_dir.clone(),
                commitish: head.commitish().to_owned(),
            });

            if !head.is_clean() {
                steps.push(Step::StashPop {
                    repo_root: worktree_dir.clone(),
                });
            }
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
                    repo_root,
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
        repo_root: NormalPath,
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
                repo_root,
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
        }
    }
}
