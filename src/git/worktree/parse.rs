use std::fmt::Display;
use std::ops::Deref;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use miette::miette;
use owo_colors::OwoColorize;
use owo_colors::Stream;
use rustc_hash::FxHashMap as HashMap;
use winnow::combinator::alt;
use winnow::combinator::cut_err;
use winnow::combinator::eof;
use winnow::combinator::opt;
use winnow::combinator::repeat_till;
use winnow::error::AddContext;
use winnow::error::ContextError;
use winnow::error::ErrMode;
use winnow::error::StrContextValue;
use winnow::stream::Stream as _;
use winnow::PResult;
use winnow::Parser;

use crate::parse::till_null;
use crate::CommitHash;
use crate::Git;
use crate::LocalBranchRef;
use crate::PathDisplay;
use crate::Ref;
use crate::ResolvedCommitish;

/// A set of Git worktrees.
///
/// Exactly one of the worktrees is the main worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktrees {
    /// The path of the main worktree. This contains the common `.git` directory, or, in the
    /// case of a bare repository, _is_ a `.git` directory.
    pub(crate) main: Utf8PathBuf,
    /// A map from worktree paths to worktree information.
    pub(crate) inner: HashMap<Utf8PathBuf, Worktree>,
}

impl Worktrees {
    pub fn main_path(&self) -> &Utf8Path {
        &self.main
    }

    pub fn main(&self) -> &Worktree {
        self.inner.get(&self.main).unwrap()
    }

    pub fn into_main(mut self) -> Worktree {
        self.inner.remove(&self.main).unwrap()
    }

    pub fn into_inner(self) -> HashMap<Utf8PathBuf, Worktree> {
        self.inner
    }

    pub fn for_branch(&self, branch: &LocalBranchRef) -> Option<&Worktree> {
        self.iter()
            .map(|(_path, worktree)| worktree)
            .find(|worktree| worktree.head.branch() == Some(branch))
    }

    fn parser(input: &mut &str) -> PResult<Self> {
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

        let worktrees = Self {
            main: main_path,
            inner,
        };

        tracing::debug!(
            worktrees=%worktrees,
            "Parsed worktrees",
        );

        Ok(worktrees)
    }

    pub fn parse(git: &Git, input: &str) -> miette::Result<Self> {
        let mut ret = Self::parser.parse(input).map_err(|err| miette!("{err}"))?;

        if ret.main().head.is_bare() {
            // Git has a bug(?) where `git worktree list` will show the _parent_ of a
            // bare worktree in a directory named `.git`. Work around it by getting the
            // `.git` directory manually and patching up the returned value to replace the path of
            // the main worktree (which is, unfortunately, stored in three separate places).
            //
            // It's _possible_ to do this in the parser, but then the parser is no longer a pure
            // function and the error handling is very annoying.
            //
            // See: https://lore.kernel.org/git/8f961645-2b70-4d45-a9f9-72e71c07bc11@app.fastmail.com/T/

            let git_dir = git.path().git_common_dir()?;
            if git_dir != ret.main {
                let old_main_path = std::mem::replace(&mut ret.main, git_dir);
                let mut main = ret
                    .inner
                    .remove(&old_main_path)
                    .expect("There is always a main worktree");
                main.path = ret.main.clone();
                ret.inner.insert(ret.main.clone(), main);
            }
        }

        Ok(ret)
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
    Branch(CommitHash, LocalBranchRef),
}

impl WorktreeHead {
    pub fn commit(&self) -> Option<&CommitHash> {
        match self {
            WorktreeHead::Bare => None,
            WorktreeHead::Detached(commit) => Some(commit),
            WorktreeHead::Branch(commit, _branch) => Some(commit),
        }
    }

    pub fn commitish(&self) -> Option<ResolvedCommitish> {
        match self {
            WorktreeHead::Bare => None,
            WorktreeHead::Detached(commit) => Some(ResolvedCommitish::Commit(commit.clone())),
            WorktreeHead::Branch(_, branch) => Some(ResolvedCommitish::Ref(branch.deref().clone())),
        }
    }

    pub fn branch(&self) -> Option<&LocalBranchRef> {
        match &self {
            WorktreeHead::Bare => None,
            WorktreeHead::Detached(_) => None,
            WorktreeHead::Branch(_, branch) => Some(branch),
        }
    }

    pub fn is_bare(&self) -> bool {
        matches!(&self, WorktreeHead::Bare)
    }

    pub fn is_detached(&self) -> bool {
        matches!(&self, WorktreeHead::Detached(_))
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

    fn parse_branch(input: &mut &str) -> PResult<Option<LocalBranchRef>> {
        let _ = "branch ".parse_next(input)?;
        let before_branch = input.checkpoint();
        let ref_name = cut_err(till_null.and_then(Ref::parser))
            .parse_next(input)?
            .try_into()
            .map_err(|_err| {
                ErrMode::Cut(ContextError::new().add_context(
                    input,
                    &before_branch,
                    winnow::error::StrContext::Expected(StrContextValue::Description(
                        "a branch ref",
                    )),
                ))
            })?;

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
        write!(f, "{} {}", self.path.display_path_cwd(), self.head)?;

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
    fn parser(input: &mut &str) -> PResult<Self> {
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

    #[cfg(test)]
    pub fn new_bare(path: impl Into<Utf8PathBuf>) -> Self {
        Self {
            path: path.into(),
            head: WorktreeHead::Bare,
            is_main: true,
            locked: None,
            prunable: None,
        }
    }

    #[cfg(test)]
    pub fn new_detached(path: impl Into<Utf8PathBuf>, commit: impl Into<CommitHash>) -> Self {
        Self {
            path: path.into(),
            head: WorktreeHead::Detached(commit.into()),
            is_main: false,
            locked: None,
            prunable: None,
        }
    }

    #[cfg(test)]
    pub fn new_branch(
        path: impl Into<Utf8PathBuf>,
        commit: impl Into<CommitHash>,
        branch: impl Into<LocalBranchRef>,
    ) -> Self {
        Self {
            path: path.into(),
            head: WorktreeHead::Branch(commit.into(), branch.into()),
            is_main: false,
            locked: None,
            prunable: None,
        }
    }

    #[cfg(test)]
    pub fn with_is_main(mut self, is_main: bool) -> Self {
        self.is_main = is_main;
        self
    }

    #[cfg(test)]
    pub fn with_locked(mut self, locked: impl Into<String>) -> Self {
        self.locked = Some(locked.into());
        self
    }

    #[cfg(test)]
    pub fn with_prunable(mut self, prunable: impl Into<String>) -> Self {
        self.prunable = Some(prunable.into());
        self
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
        let worktrees = Worktrees::parser
            .parse(
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

        assert_eq!(worktrees.main_path(), "/path/to/bare-source");

        let worktrees = worktrees
            .inner
            .into_values()
            .sorted_by_key(|worktree| worktree.path.to_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            worktrees,
            vec![
                Worktree::new_branch(
                    "/Users/wiggles/cabal/accept",
                    "0685cb3fec8b7144f865638cfd16768e15125fc2",
                    "rebeccat/fix-accept-flag"
                ),
                Worktree::new_detached(
                    "/Users/wiggles/lix",
                    "0d484aa498b3c839991d11afb31bc5fcf368493d"
                ),
                Worktree::new_bare("/path/to/bare-source"),
                Worktree::new_branch(
                    "/path/to/linked-worktree-locked-no-reason",
                    "5678abc5678abc5678abc5678abc5678abc5678c",
                    "locked-no-reason"
                )
                .with_locked(""),
                Worktree::new_branch(
                    "/path/to/linked-worktree-locked-with-reason",
                    "3456def3456def3456def3456def3456def3456b",
                    "locked-with-reason"
                )
                .with_locked("reason why is locked"),
                Worktree::new_detached(
                    "/path/to/linked-worktree-prunable",
                    "1233def1234def1234def1234def1234def1234b",
                )
                .with_prunable("gitdir file points to non-existent location"),
            ]
        );
    }
}
