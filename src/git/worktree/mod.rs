use std::borrow::Cow;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::process::Command;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use command_error::CommandExt;
use command_error::OutputContext;
use miette::miette;
use miette::Context;
use rustc_hash::FxHashMap;
use tap::Tap;
use tracing::instrument;
use utf8_command::Utf8Output;

use crate::config::BranchReplacement;
use crate::final_component;
use crate::AppGit;

use super::BranchRef;
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
    pub fn dirname_for<'b>(&self, branch: &'b str) -> Cow<'b, str> {
        let branch_replacements = self.0.config.file.branch_replacements();
        if branch_replacements.is_empty() {
            Cow::Borrowed(final_component(branch))
        } else {
            let mut dirname = branch.to_owned();
            for BranchReplacement {
                find,
                replace,
                count,
            } in branch_replacements
            {
                dirname = match count {
                    Some(count) => find.replacen(&dirname, *count, replace),
                    None => find.replace_all(&dirname, replace),
                }
                .into_owned();
            }
            dirname.into()
        }
    }

    /// Get the full path for a new worktree with the given branch name.
    ///
    /// This appends the [`Self::dirname_for`] to the [`Self::container`].
    #[instrument(level = "trace")]
    pub fn path_for(&self, branch: &str) -> miette::Result<Utf8PathBuf> {
        Ok(self
            .container()?
            .tap_mut(|p| p.push(&*self.dirname_for(branch))))
    }

    /// Resolves a set of worktrees into a map from worktree paths to unique names.
    #[instrument(level = "trace")]
    pub fn resolve_unique_names(
        &self,
        opts: ResolveUniqueNameOpts<'_>,
    ) -> miette::Result<FxHashMap<Utf8PathBuf, RenamedWorktree>> {
        resolve_unique_names::resolve_unique_worktree_names(self.0, opts)
    }

    /// Get the worktree for the preferred branch, if any.
    #[instrument(level = "trace")]
    pub fn preferred_branch(
        &self,
        preferred_branch: Option<&BranchRef>,
        worktrees: Option<&Worktrees>,
    ) -> miette::Result<Option<Worktree>> {
        let worktrees = match worktrees {
            Some(worktrees) => worktrees,
            None => &self.list()?,
        };
        let preferred_branch = match preferred_branch {
            Some(preferred_branch) => preferred_branch,
            None => &match self.0.branch().preferred()? {
                Some(preferred_branch) => preferred_branch,
                None => {
                    return Ok(None);
                }
            },
        };

        // TODO: Check for branch with the default as an upstream as well?
        Ok(worktrees.for_branch(&preferred_branch.as_local()).cloned())
    }

    /// Get the path to _some_ worktree.
    ///
    /// This prefers, in order:
    /// 1. The current worktree.
    /// 2. The worktree for the default branch.
    /// 3. Any non-bare worktree.
    /// 4. A bare worktree.
    #[instrument(level = "trace")]
    pub fn find_some(&self) -> miette::Result<Utf8PathBuf> {
        if self.is_inside()? {
            tracing::debug!("Inside worktree");
            // Test: `add_by_path`
            return self.root();
        }
        let worktrees = self.list()?;

        if let Some(worktree) = self.preferred_branch(None, Some(&worktrees))? {
            tracing::debug!(%worktree, "Found worktree for preferred branch");
            // Test: `add_from_container`
            return Ok(worktree.path);
        }

        tracing::debug!("No worktree for preferred branch");

        if worktrees.main().head.is_bare() && worktrees.len() > 1 {
            // Find a non-bare worktree.
            //
            // Test: `add_from_container_no_default_branch`
            let worktree = worktrees
                .into_iter()
                .find(|(_path, worktree)| !worktree.head.is_bare())
                .expect("Only one worktree can be bare")
                .0;

            tracing::debug!(%worktree, "Found non-bare worktree");
            return Ok(worktree);
        }

        // Otherwise, get the main worktree.
        // Either the main worktree is bare and there's no other worktrees, or the main
        // worktree is not bare.
        //
        // Note: If the main worktree isn't bare, there's no way to run Git commands
        // without being in a worktree. IDK I guess you can probably do something silly
        // with separating the Git directory and the worktree but like, why.
        //
        // Tests:
        // - `add_from_bare_no_worktrees`
        tracing::debug!("Non-bare main worktree or no non-bare worktrees");
        Ok(worktrees.main_path().to_owned())
    }
}
