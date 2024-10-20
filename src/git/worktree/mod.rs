use std::ffi::OsStr;
use std::fmt::Debug;
use std::process::Command;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use command_error::CommandExt;
use command_error::OutputContext;
use miette::miette;
use miette::Context;
use rustc_hash::FxHashMap as HashMap;
use tap::Tap;
use tracing::instrument;
use utf8_command::Utf8Output;

use crate::AppGit;

use super::GitLike;
use super::LocalBranchRef;

mod resolve_unique_names;

mod parse;

pub use parse::Worktree;
pub use parse::WorktreeHead;
pub use parse::Worktrees;
pub use resolve_unique_names::RenamedWorktree;
pub use resolve_unique_names::ResolveUniqueNameOpts;

/// Git methods for dealing with worktrees.
#[repr(transparent)]
pub struct GitWorktree<'a, G>(&'a G);

impl<G> Debug for GitWorktree<'_, G>
where
    G: GitLike,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("GitWorktree")
            .field(&self.0.get_current_dir().as_ref())
            .finish()
    }
}

impl<'a, G> GitWorktree<'a, G>
where
    G: GitLike,
{
    pub fn new(git: &'a G) -> Self {
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
        let mut path = self.main()?.path;
        if !path.pop() {
            Err(miette!("Main worktree path has no parent: {path}"))
        } else {
            Ok(path)
        }
    }

    /// List Git worktrees.
    #[instrument(level = "trace")]
    pub fn list(&self) -> miette::Result<Worktrees> {
        Ok(self
            .0
            .command()
            .args(["worktree", "list", "--porcelain", "-z"])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if !context.status().success() {
                    Err(context.error())
                } else {
                    let output = &context.output().stdout;
                    match Worktrees::parse(self.0.as_git(), output) {
                        Ok(worktrees) => Ok(worktrees),
                        Err(err) => {
                            let err = miette!("{err}");
                            Err(context.error_msg(err))
                        }
                    }
                }
            })?)
    }

    /// Check if we're inside a working tree.
    ///
    /// This will return false for a bare worktree like a `.git` directory!
    #[instrument(level = "trace")]
    pub fn is_inside(&self) -> miette::Result<bool> {
        Ok(self
            .0
            .as_git()
            .rev_parse_command()
            .arg("--is-inside-work-tree")
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if !context.status().success() {
                    Err(context.error())
                } else {
                    let stdout = context.output().stdout.trim();
                    match stdout {
                        "true" => Ok(true),
                        "false" => Ok(false),
                        _ => Err(context.error_msg("Expected 'true' or 'false'")),
                    }
                }
            })?)
    }

    /// Get the root of this worktree. Fails if not in a worktree.
    #[instrument(level = "trace")]
    pub fn root(&self) -> miette::Result<Utf8PathBuf> {
        Ok(self
            .0
            .as_git()
            .rev_parse_command()
            .arg("--show-toplevel")
            .output_checked_utf8()
            .wrap_err("Failed to get worktree root")?
            .stdout
            .trim()
            .into())
    }

    #[instrument(level = "trace")]
    pub fn add(&self, path: &Utf8Path, options: &AddWorktreeOpts<'_>) -> miette::Result<()> {
        self.add_command(path, options).status_checked()?;
        Ok(())
    }

    #[instrument(level = "trace")]
    pub fn add_command(&self, path: &Utf8Path, options: &AddWorktreeOpts<'_>) -> Command {
        let mut command = self.0.command();
        command.args(["worktree", "add"]);

        if options.detach {
            command.arg("--detach");
        }

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
            .status_checked()?;
        Ok(())
    }

    #[instrument(level = "trace")]
    pub fn repair(
        &self,
        paths: impl IntoIterator<Item = impl AsRef<OsStr>> + Debug,
    ) -> miette::Result<()> {
        self.0
            .command()
            .args(["worktree", "repair"])
            .args(paths)
            .output_checked_utf8()?;
        Ok(())
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
    /// If true, use `--detach`.
    /// Default false.
    pub detach: bool,
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
            detach: false,
        }
    }
}

impl<'a, C> GitWorktree<'a, AppGit<'a, C>>
where
    C: AsRef<Utf8Path>,
{
    /// The directory name, nested under the worktree parent directory, where the given
    /// branch's worktree will be placed.
    ///
    /// E.g. to convert a repo `~/puppy` with default branch `main`, this will return `main`,
    /// to indicate a worktree to be placed in `~/puppy/main`.
    ///
    /// TODO: Should support some configurable regex filtering or other logic?
    pub fn dirname_for<'b>(&self, branch: &'b str) -> &'b str {
        match branch.rsplit_once('/') {
            Some((_left, right)) => right,
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

    /// Resolves a set of worktrees into a map from worktree paths to unique names.
    #[instrument(level = "trace")]
    pub fn resolve_unique_names(
        &self,
        opts: ResolveUniqueNameOpts<'_>,
    ) -> miette::Result<HashMap<Utf8PathBuf, RenamedWorktree>> {
        resolve_unique_names::resolve_unique_worktree_names(self.0, opts)
    }
}
