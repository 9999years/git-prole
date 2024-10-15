use std::fmt::Debug;
use std::process::Command;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use command_error::CommandExt;
use command_error::OutputContext;
use miette::miette;
use miette::IntoDiagnostic;
use tap::Tap;
use tracing::instrument;
use utf8_command::Utf8Output;
use winnow::Parser;

use super::Git;
use super::LocalBranchRef;

mod parse;
pub use parse::Worktree;
pub use parse::WorktreeHead;
pub use parse::Worktrees;

/// Git methods for dealing with worktrees.
#[repr(transparent)]
pub struct GitWorktree<'a>(&'a Git);

impl Debug for GitWorktree<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.0, f)
    }
}

impl<'a> GitWorktree<'a> {
    pub fn new(git: &'a Git) -> Self {
        Self(git)
    }

    /// Get the 'main' worktree. There can only be one main worktree, and it contains the
    /// common `.git` directory.
    ///
    /// See: <https://stackoverflow.com/a/68754000>
    #[instrument(level = "trace")]
    pub fn main(&self) -> miette::Result<Worktree> {
        // Kinda wasteful; we parse all the worktrees and then throw them away.
        Ok(self.list()?.into_main())
    }

    /// Get the worktree container directory.
    ///
    /// This is the main worktree's parent, and is usually where all the other worktrees are
    /// cloned as well.
    #[instrument(level = "trace")]
    pub fn container(&self) -> miette::Result<Utf8PathBuf> {
        // TODO: Write `.git-prole` to indicate worktree container root?
        let main = self.main()?;
        let mut path = if main.head == WorktreeHead::Bare {
            // Git has a bug(?) where `git worktree list` will show the _parent_ of a
            // bare worktree in a directory named `.git`. Work around it by getting the
            // `.git` directory manually.
            //
            // See: https://lore.kernel.org/git/8f961645-2b70-4d45-a9f9-72e71c07bc11@app.fastmail.com/T/
            self.0.with_directory(main.path).path().git_common_dir()?
        } else {
            main.path
        };

        if !path.pop() {
            Err(miette!("Main worktree path has no parent: {path}"))
        } else {
            Ok(path)
        }
    }

    /// List Git worktrees.
    #[instrument(level = "trace")]
    pub fn list(&self) -> miette::Result<Worktrees> {
        self.0
            .command()
            .args(["worktree", "list", "--porcelain", "-z"])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if !context.status().success() {
                    Err(context.error())
                } else {
                    let output = &context.output().stdout;
                    match Worktrees::parser.parse(output) {
                        Ok(worktrees) => Ok(worktrees),
                        Err(err) => {
                            let err = miette!("{err}");
                            Err(context.error_msg(err))
                        }
                    }
                }
            })
            .into_diagnostic()
    }

    #[instrument(level = "trace")]
    pub fn add(&self, path: &Utf8Path, options: &AddWorktreeOpts<'_>) -> miette::Result<()> {
        self.add_command(path, options)
            .status_checked()
            .into_diagnostic()?;
        Ok(())
    }

    #[instrument(level = "trace")]
    pub fn add_command(&self, path: &Utf8Path, options: &AddWorktreeOpts<'_>) -> Command {
        let mut command = self.0.command();
        command.args(["worktree", "add"]);

        if let Some(branch) = options.create_branch {
            command.arg(if options.force_branch { "-B" } else { "-b" });
            command.arg(branch.branch_name());
        }

        if !options.checkout {
            command.arg("--no-checkout");
        }

        if options.guess_remote {
            command.arg("--guess-remote");
        }

        if options.track {
            command.arg("--track");
        }

        command.arg(path.as_str());

        if let Some(start_point) = options.start_point {
            command.arg(start_point);
        }

        command
    }

    #[instrument(level = "trace")]
    pub fn rename(&self, from: &Utf8Path, to: &Utf8Path) -> miette::Result<()> {
        self.0
            .command()
            .current_dir(from)
            .args(["worktree", "move", from.as_str(), to.as_str()])
            .status_checked()
            .into_diagnostic()?;
        Ok(())
    }

    #[instrument(level = "trace")]
    pub fn repair(&self) -> miette::Result<()> {
        self.0
            .command()
            .args(["worktree", "repair"])
            .status_checked()
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
    pub fn dirname_for<'b>(&self, branch: &'b str) -> &'b str {
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
    /// This appends the [`Self::dirname_for`] to the [`Self::container`].
    #[instrument(level = "trace")]
    pub fn path_for(&self, branch: &str) -> miette::Result<Utf8PathBuf> {
        Ok(self
            .container()?
            .tap_mut(|p| p.push(self.dirname_for(branch))))
    }
}

/// Options for `git worktree add`.
#[derive(Clone, Copy, Debug)]
pub struct AddWorktreeOpts<'a> {
    /// If true, use `-B` instead of `-b` for `create_branch`.
    /// Default false.
    pub force_branch: bool,
    /// Create a new branch.
    pub create_branch: Option<&'a LocalBranchRef>,
    /// If false, use `--no-checkout`.
    /// Default true.
    pub checkout: bool,
    /// If true, use `--guess-remote`.
    /// Default false.
    pub guess_remote: bool,
    /// If true, use `--track`.
    /// Default false.
    pub track: bool,
    /// The start point for the new worktree.
    pub start_point: Option<&'a str>,
}

impl<'a> Default for AddWorktreeOpts<'a> {
    fn default() -> Self {
        Self {
            force_branch: false,
            create_branch: None,
            checkout: true,
            guess_remote: false,
            track: false,
            start_point: None,
        }
    }
}