use std::collections::HashMap;
use std::fmt::Debug;
use std::fmt::Display;
use std::ops::Deref;
use std::str::FromStr;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use command_error::CommandExt;
use command_error::OutputContext;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;
use tap::Tap;
use tracing::instrument;
use utf8_command::Utf8Output;

use super::commit_hash::CommitHash;
use super::ref_name::Ref;
use super::Git;

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
    pub fn main(&self) -> miette::Result<Utf8PathBuf> {
        Ok(self.list()?.main)
    }

    /// Get the worktree container directory.
    ///
    /// This is the main worktree's parent, and is usually where all the other worktrees are cloned
    /// as well.
    #[instrument(level = "trace")]
    pub fn container(&self) -> miette::Result<Utf8PathBuf> {
        // TODO: Write `.git-prole` to indicate worktree container root?
        let mut container = self.main()?;
        if !container.pop() {
            Err(miette!("Main worktree path has no parent: {container}"))
        } else {
            Ok(container)
        }
    }

    /// List Git worktrees.
    #[instrument(level = "trace")]
    pub fn list(&self) -> miette::Result<Worktrees> {
        Worktrees::from_git(self.0)
    }

    #[instrument(level = "trace")]
    pub fn add(&self, path: &Utf8Path, commitish: &str) -> miette::Result<()> {
        self.0
            .command()
            .args(["worktree", "add", path.as_str(), commitish])
            .status_checked()
            .into_diagnostic()?;
        Ok(())
    }

    #[instrument(level = "trace")]
    pub fn add_no_checkout(&self, path: &Utf8Path, commitish: &str) -> miette::Result<()> {
        self.0
            .command()
            .args(["worktree", "add", "--no-checkout", path.as_str(), commitish])
            .status_checked()
            .into_diagnostic()?;
        Ok(())
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
    /// This appends the [`Self::branch_dirname`] to the [`Git::worktree_container`].
    #[instrument(level = "trace")]
    pub fn path_for(&self, branch: &str) -> miette::Result<Utf8PathBuf> {
        Ok(self
            .container()?
            .tap_mut(|p| p.push(self.dirname_for(branch))))
    }
}

/// A set of Git worktrees.
///
/// Exactly one of the worktrees is the main worktree.
#[derive(Debug, PartialEq, Eq)]
pub struct Worktrees {
    /// The path of the main worktree. This contains the common `.git` directory.
    main: Utf8PathBuf,
    /// A map from worktree paths to worktree information.
    inner: HashMap<Utf8PathBuf, Worktree>,
}

impl Worktrees {
    pub fn from_git(git: &Git) -> miette::Result<Self> {
        let (main, worktrees) = git
            .command()
            .args(["worktree", "list", "--porcelain"])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                Worktree::from_git_output_all(&context.output().stdout)
                    .map_err(|err| context.error_msg(err))
            })
            .into_diagnostic()?;

        Ok(Self {
            main,
            inner: worktrees,
        })
    }
}

impl Deref for Worktrees {
    type Target = HashMap<Utf8PathBuf, Worktree>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Display for Worktrees {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut trees = self.values().peekable();
        while let Some(tree) = trees.next() {
            if trees.peek().is_none() {
                write!(f, "{tree}")?;
            } else {
                writeln!(f, "{tree}")?;
            }
        }
        Ok(())
    }
}

/// A Git worktree.
#[derive(Debug, PartialEq, Eq)]
pub struct Worktree {
    pub path: Utf8PathBuf,
    pub head: CommitHash,
    pub branch: Option<Ref>,
    pub is_main: bool,
}

impl Display for Worktree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.path, self.head.abbrev())?;
        match &self.branch {
            None => {
                write!(f, " [detached]")?;
            }
            Some(ref_name) => {
                write!(f, " [{ref_name}]")?;
            }
        }
        if self.is_main {
            write!(f, " [main]")?;
        }
        Ok(())
    }
}

impl Worktree {
    /// Parses `git worktree list --porcelain` output, like this:
    ///
    /// ```plain
    /// worktree /Users/wiggles/cabal/master
    /// HEAD c53a03ae672c7d2d33ad9aa2469c1e38f3a052ce
    /// branch refs/heads/master
    ///
    /// worktree /Users/wiggles/cabal/accept
    /// HEAD 0685cb3fec8b7144f865638cfd16768e15125fc2
    /// branch refs/heads/rebeccat/fix-accept-flag
    ///
    /// ```
    ///
    /// Note the trailing newlines!
    fn from_git_output_all(
        mut output: &str,
    ) -> miette::Result<(Utf8PathBuf, HashMap<Utf8PathBuf, Self>)> {
        let mut worktrees = HashMap::new();
        let mut main = None;

        while !output.is_empty() {
            let (mut worktree, rest) = Self::from_git_output(output)?;

            // From `git-worktree(1)`:
            //
            //     The main worktree is listed first, followed by each of the linked worktrees.
            if main.is_none() {
                worktree.is_main = true;
                main = Some(worktree.path.clone());
            }

            output = rest;
            worktrees.insert(worktree.path.clone(), worktree);
        }

        let main = main.expect("There is always a main worktree");

        Ok((main, worktrees))
    }

    pub fn from_git_output(output: &str) -> miette::Result<(Self, &str)> {
        Self::from_git_output_inner(output)
            .wrap_err_with(|| format!("Failed to parse worktrees:\n{output}"))
    }

    fn from_git_output_inner(output: &str) -> miette::Result<(Self, &str)> {
        // TODO: Pull in a parsing library?
        let output = take_prefix(output, "worktree ")?;
        let (path, output) = take_rest_of_line(output)?;

        let output = take_prefix(output, "HEAD ")?;
        let (head, output) = take_rest_of_line(output)?;
        let head = CommitHash::from(head.to_owned());

        let (output, branch) = if output.starts_with("detached") {
            let output = take_prefix(output, "detached")?;
            let (_, output) = take_rest_of_line(output)?;
            (output, None)
        } else {
            let output = take_prefix(output, "branch ")?;
            let (branch, output) = take_rest_of_line(output)?;
            (output, Some(Ref::from_str(branch)?))
        };
        let output = take_prefix(output, "\n")?;

        Ok((
            Self {
                path: Utf8PathBuf::from(path),
                head,
                branch,
                is_main: false,
            },
            output,
        ))
    }
}

fn take_rest_of_line(input: &str) -> miette::Result<(&str, &str)> {
    input
        .split_once('\n')
        .ok_or_else(|| miette!("Expected text and then a newline"))
}

fn take_prefix<'i>(input: &'i str, prefix: &str) -> miette::Result<&'i str> {
    input
        .strip_prefix(prefix)
        .ok_or_else(|| miette!("Expected {prefix:?}"))
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use itertools::Itertools;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_from_git_output() {
        assert_eq!(
            Worktree::from_git_output(indoc!(
                "
                worktree /Users/wiggles/cabal/master
                HEAD c53a03ae672c7d2d33ad9aa2469c1e38f3a052ce
                branch refs/heads/master

                ooga booga

                "
            ))
            .unwrap(),
            (
                Worktree {
                    path: "/Users/wiggles/cabal/master".into(),
                    head: CommitHash::from("c53a03ae672c7d2d33ad9aa2469c1e38f3a052ce".to_owned()),
                    branch: Some(Ref::from_str("refs/heads/master").unwrap()),
                    is_main: false,
                },
                "ooga booga\n\n"
            )
        );
    }

    #[test]
    fn test_from_git_output_all() {
        let (main, worktrees) = Worktree::from_git_output_all(indoc!(
            "
                worktree /Users/wiggles/cabal/master
                HEAD c53a03ae672c7d2d33ad9aa2469c1e38f3a052ce
                branch refs/heads/master

                worktree /Users/wiggles/cabal/accept
                HEAD 0685cb3fec8b7144f865638cfd16768e15125fc2
                branch refs/heads/rebeccat/fix-accept-flag

                worktree /Users/wiggles/lix
                HEAD 0d484aa498b3c839991d11afb31bc5fcf368493d
                detached

                "
        ))
        .unwrap();

        assert_eq!(main, "/Users/wiggles/cabal/master");

        let worktrees = worktrees
            .into_values()
            .sorted_by_key(|worktree| worktree.path.to_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            worktrees,
            vec![
                Worktree {
                    path: "/Users/wiggles/cabal/accept".into(),
                    head: CommitHash::from("0685cb3fec8b7144f865638cfd16768e15125fc2".to_owned()),
                    branch: Some(Ref::from_str("refs/heads/rebeccat/fix-accept-flag").unwrap()),
                    is_main: false,
                },
                Worktree {
                    path: "/Users/wiggles/cabal/master".into(),
                    head: CommitHash::from("c53a03ae672c7d2d33ad9aa2469c1e38f3a052ce".to_owned()),
                    branch: Some(Ref::from_str("refs/heads/master").unwrap()),
                    is_main: true,
                },
                Worktree {
                    path: "/Users/wiggles/lix".into(),
                    head: CommitHash::from("0d484aa498b3c839991d11afb31bc5fcf368493d".to_owned()),
                    branch: None,
                    is_main: false,
                },
            ]
        );
    }
}
