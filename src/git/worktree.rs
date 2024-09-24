use std::collections::HashMap;
use std::fmt::Display;
use std::ops::Deref;
use std::str::FromStr;

use camino::Utf8PathBuf;
use command_error::CommandExt;
use command_error::OutputContext;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;
use utf8_command::Utf8Output;

use super::commit_hash::CommitHash;
use super::ref_name::Ref;
use super::Git;

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
        let main = git.main_worktree()?;

        let worktrees = git
            .command()
            .args(["worktree", "list", "--porcelain"])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                Worktree::from_git_output_all(&context.output().stdout)
                    .map_err(|err| context.error_msg(err))
            })
            .into_diagnostic()?;

        let mut worktrees = Self {
            main,
            inner: worktrees,
        };

        match worktrees.inner.get_mut(&worktrees.main) {
            Some(main_worktree) => {
                main_worktree.is_main = true;
            }
            None => {
                tracing::warn!(
                    main = %worktrees.main,
                    %worktrees,
                    "No main worktree found in `git worktree list` output"
                );
            }
        }

        Ok(worktrees)
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
    fn from_git_output_all(mut output: &str) -> miette::Result<HashMap<Utf8PathBuf, Self>> {
        let mut worktrees = HashMap::new();

        while !output.is_empty() {
            let (worktree, rest) = Self::from_git_output(output)?;
            output = rest;
            worktrees.insert(worktree.path.clone(), worktree);
        }

        Ok(worktrees)
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
        assert_eq!(
            Worktree::from_git_output_all(indoc!(
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
            .unwrap()
            .into_values()
            .sorted_by_key(|worktree| worktree.path.to_owned())
            .collect::<Vec<_>>(),
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
                    is_main: false,
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
