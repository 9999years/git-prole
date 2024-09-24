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
use miette::IntoDiagnostic;
use owo_colors::OwoColorize;
use owo_colors::Stream;
use tap::Tap;
use tracing::instrument;
use utf8_command::Utf8Output;
use winnow::combinator::alt;
use winnow::combinator::cut_err;
use winnow::combinator::eof;
use winnow::combinator::opt;
use winnow::combinator::repeat_till;
use winnow::PResult;
use winnow::Parser;

use crate::parse::till_null;
use crate::NormalPath;

use super::commit_hash::CommitHash;
use super::Git;
use super::Ref;

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
        let mut worktrees = self.list()?;
        Ok(worktrees.inner.remove(&worktrees.main).unwrap())
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
    /// This appends the [`Self::dirname_for`] to the [`Self::container`].
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktrees {
    /// The path of the main worktree. This contains the common `.git` directory.
    main: Utf8PathBuf,
    /// A map from worktree paths to worktree information.
    inner: HashMap<Utf8PathBuf, Worktree>,
}

impl Worktrees {
    pub fn main(&self) -> &Utf8Path {
        &self.main
    }

    pub fn parser(input: &mut &str) -> PResult<Self> {
        let mut main = Worktree::parser.parse_next(input)?;
        main.is_main = true;
        let main_path = main.path.clone();

        let mut inner: HashMap<_, _> = repeat_till(
            0..,
            Worktree::parser.map(|worktree| (worktree.path.clone(), worktree)),
            eof,
        )
        .map(|(inner, _eof)| inner)
        .parse_next(input)?;

        inner.insert(main_path.clone(), main);

        Ok(Self {
            main: main_path,
            inner,
        })
    }
}

impl FromStr for Worktrees {
    type Err = miette::Report;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parser.parse(input).map_err(|err| miette!("{err}"))
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeHead {
    Bare,
    Detached(CommitHash),
    Branch(CommitHash, Ref),
}

impl WorktreeHead {
    pub fn commit(&self) -> Option<&CommitHash> {
        match self {
            WorktreeHead::Bare => None,
            WorktreeHead::Detached(commit) => Some(commit),
            WorktreeHead::Branch(commit, _branch) => Some(commit),
        }
    }

    pub fn parser(input: &mut &str) -> PResult<Self> {
        alt(("bare\0".map(|_| Self::Bare), Self::parse_non_bare)).parse_next(input)
    }

    fn parse_non_bare(input: &mut &str) -> PResult<Self> {
        let _ = "HEAD ".parse_next(input)?;
        let head = till_null.and_then(CommitHash::parser).parse_next(input)?;
        let branch = alt((Self::parse_branch, "detached\0".map(|_| None))).parse_next(input)?;

        Ok(match branch {
            Some(branch) => Self::Branch(head, branch),
            None => Self::Detached(head),
        })
    }

    fn parse_branch(input: &mut &str) -> PResult<Option<Ref>> {
        let _ = "branch ".parse_next(input)?;
        let ref_name = cut_err(till_null.and_then(Ref::parser)).parse_next(input)?;

        Ok(Some(ref_name))
    }
}

impl Display for WorktreeHead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorktreeHead::Bare => write!(
                f,
                "{}",
                "bare".if_supports_color(Stream::Stdout, |text| text.dimmed())
            ),
            WorktreeHead::Detached(commit) => {
                write!(
                    f,
                    "{}",
                    commit.if_supports_color(Stream::Stdout, |text| text.cyan())
                )
            }
            WorktreeHead::Branch(_, ref_name) => {
                write!(
                    f,
                    "{}",
                    ref_name.if_supports_color(Stream::Stdout, |text| text.cyan())
                )
            }
        }
    }
}

/// A Git worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: Utf8PathBuf,
    pub head: WorktreeHead,
    pub is_main: bool,
    pub locked: Option<String>,
    pub prunable: Option<String>,
}

impl Display for Worktree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = NormalPath::from_cwd(&self.path)
            .map(|path| path.to_string())
            .unwrap_or_else(|_| {
                self.path
                    .if_supports_color(Stream::Stdout, |text| text.cyan())
                    .to_string()
            });
        write!(f, "{path} {}", self.head)?;

        if self.is_main {
            write!(
                f,
                " [{}]",
                "main".if_supports_color(Stream::Stdout, |text| text.cyan())
            )?;
        }

        if let Some(reason) = &self.locked {
            if reason.is_empty() {
                write!(f, " (locked)")?;
            } else {
                write!(f, " (locked: {reason})")?;
            }
        }

        if let Some(reason) = &self.prunable {
            if reason.is_empty() {
                write!(f, " (prunable)")?;
            } else {
                write!(f, " (prunable: {reason})")?;
            }
        }

        Ok(())
    }
}

impl Worktree {
    pub fn parser(input: &mut &str) -> PResult<Self> {
        let _ = "worktree ".parse_next(input)?;
        let path = Utf8PathBuf::from(till_null.parse_next(input)?);
        let head = WorktreeHead::parser.parse_next(input)?;
        let locked = opt(Self::parse_locked).parse_next(input)?;
        let prunable = opt(Self::parse_prunable).parse_next(input)?;
        let _ = '\0'.parse_next(input)?;

        Ok(Self {
            path,
            head,
            locked,
            prunable,
            is_main: false,
        })
    }

    fn parse_locked(input: &mut &str) -> PResult<String> {
        let _ = "locked".parse_next(input)?;
        let reason = Self::parse_reason.parse_next(input)?;

        Ok(reason)
    }

    fn parse_prunable(input: &mut &str) -> PResult<String> {
        let _ = "prunable".parse_next(input)?;
        let reason = Self::parse_reason.parse_next(input)?;

        Ok(reason)
    }

    fn parse_reason(input: &mut &str) -> PResult<String> {
        let maybe_space = opt(' ').parse_next(input)?;

        match maybe_space {
            None => {
                let _ = '\0'.parse_next(input)?;
                Ok(String::new())
            }
            Some(_) => {
                let reason = till_null.parse_next(input)?;
                Ok(reason.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use itertools::Itertools;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_parse_worktrees_list() {
        let worktrees = Worktrees::from_str(
            &indoc!(
                "
                worktree /path/to/bare-source
                bare

                worktree /Users/wiggles/cabal/accept
                HEAD 0685cb3fec8b7144f865638cfd16768e15125fc2
                branch refs/heads/rebeccat/fix-accept-flag

                worktree /Users/wiggles/lix
                HEAD 0d484aa498b3c839991d11afb31bc5fcf368493d
                detached

                worktree /path/to/linked-worktree-locked-no-reason
                HEAD 5678abc5678abc5678abc5678abc5678abc5678c
                branch refs/heads/locked-no-reason
                locked

                worktree /path/to/linked-worktree-locked-with-reason
                HEAD 3456def3456def3456def3456def3456def3456b
                branch refs/heads/locked-with-reason
                locked reason why is locked

                worktree /path/to/linked-worktree-prunable
                HEAD 1233def1234def1234def1234def1234def1234b
                detached
                prunable gitdir file points to non-existent location

                "
            )
            .replace('\n', "\0"),
        )
        .unwrap();

        assert_eq!(worktrees.main(), "/path/to/bare-source");

        let worktrees = worktrees
            .inner
            .into_values()
            .sorted_by_key(|worktree| worktree.path.to_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            worktrees,
            vec![
                Worktree {
                    path: "/Users/wiggles/cabal/accept".into(),
                    head: WorktreeHead::Branch(
                        CommitHash::from("0685cb3fec8b7144f865638cfd16768e15125fc2"),
                        Ref::from_str("refs/heads/rebeccat/fix-accept-flag").unwrap(),
                    ),
                    is_main: false,
                    locked: None,
                    prunable: None,
                },
                Worktree {
                    path: "/Users/wiggles/lix".into(),
                    head: WorktreeHead::Detached(CommitHash::from(
                        "0d484aa498b3c839991d11afb31bc5fcf368493d"
                    )),
                    is_main: false,
                    locked: None,
                    prunable: None,
                },
                Worktree {
                    path: "/path/to/bare-source".into(),
                    head: WorktreeHead::Bare,
                    is_main: true,
                    locked: None,
                    prunable: None,
                },
                Worktree {
                    path: "/path/to/linked-worktree-locked-no-reason".into(),
                    head: WorktreeHead::Branch(
                        CommitHash::from("5678abc5678abc5678abc5678abc5678abc5678c"),
                        Ref::from_str("refs/heads/locked-no-reason").unwrap()
                    ),
                    is_main: false,
                    locked: Some("".into()),
                    prunable: None,
                },
                Worktree {
                    path: "/path/to/linked-worktree-locked-with-reason".into(),
                    head: WorktreeHead::Branch(
                        CommitHash::from("3456def3456def3456def3456def3456def3456b"),
                        Ref::from_str("refs/heads/locked-with-reason").unwrap()
                    ),
                    is_main: false,
                    locked: Some("reason why is locked".into()),
                    prunable: None,
                },
                Worktree {
                    path: "/path/to/linked-worktree-prunable".into(),
                    head: WorktreeHead::Detached(CommitHash::from(
                        "1233def1234def1234def1234def1234def1234b"
                    ),),
                    is_main: false,
                    locked: None,
                    prunable: Some("gitdir file points to non-existent location".into()),
                },
            ]
        );
    }
}
