use std::fmt::Debug;
use std::fmt::Display;
use std::iter;
use std::ops::Deref;
use std::str::FromStr;

use camino::Utf8PathBuf;
use command_error::CommandExt;
use command_error::OutputContext;
use miette::miette;
use tracing::instrument;
use utf8_command::Utf8Output;
use winnow::combinator::eof;
use winnow::combinator::opt;
use winnow::combinator::repeat_till;
use winnow::token::one_of;
use winnow::PResult;
use winnow::Parser;

use crate::parse::till_null;

use super::GitLike;

/// Git methods for dealing with statuses and the working tree.
#[repr(transparent)]
pub struct GitStatus<'a, G>(&'a G);

impl<G> Debug for GitStatus<'_, G>
where
    G: GitLike,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("GitStatus")
            .field(&self.0.get_current_dir().as_ref())
            .finish()
    }
}

impl<'a, G> GitStatus<'a, G>
where
    G: GitLike,
{
    pub fn new(git: &'a G) -> Self {
        Self(git)
    }

    #[instrument(level = "trace")]
    pub fn get(&self) -> miette::Result<Status> {
        Ok(self
            .0
            .command()
            .args(["status", "--porcelain=v1", "--ignored=traditional", "-z"])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if context.status().success() {
                    Status::from_str(&context.output().stdout).map_err(|err| context.error_msg(err))
                } else {
                    Err(context.error())
                }
            })?)
    }
}

/// The status code of a particular file. Each [`StatusEntry`] has two of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCode {
    /// ` `
    Unmodified,
    /// `M`
    Modified,
    /// `T`
    TypeChanged,
    /// `A`
    Added,
    /// `D`
    Deleted,
    /// `R`
    Renamed,
    /// `C`
    Copied,
    /// `U`
    Unmerged,
    /// `?`
    Untracked,
    /// `!`
    Ignored,
}

impl StatusCode {
    pub fn parser(input: &mut &str) -> PResult<Self> {
        let code = one_of([' ', 'M', 'T', 'A', 'D', 'R', 'C', 'U', '?', '!']).parse_next(input)?;
        Ok(match code {
            ' ' => Self::Unmodified,
            'M' => Self::Modified,
            'T' => Self::TypeChanged,
            'A' => Self::Added,
            'D' => Self::Deleted,
            'R' => Self::Renamed,
            'C' => Self::Copied,
            'U' => Self::Unmerged,
            '?' => Self::Untracked,
            '!' => Self::Ignored,
            _ => {
                unreachable!()
            }
        })
    }
}

impl Display for StatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Unmodified => ' ',
                Self::Modified => 'M',
                Self::TypeChanged => 'T',
                Self::Added => 'A',
                Self::Deleted => 'D',
                Self::Renamed => 'R',
                Self::Copied => 'C',
                Self::Unmerged => 'U',
                Self::Untracked => '?',
                Self::Ignored => '!',
            }
        )
    }
}

/// The status of a particular file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    /// The status of the file in the index.
    ///
    /// If no merge is occurring, or a merge was successful, this indicates the status of the
    /// index.
    ///
    /// If a merge conflict has occured and is not resolved, this is the left head of th
    /// merge.
    pub left: StatusCode,
    /// The status of the file in the working tree.
    ///
    /// If no merge is occurring, or a merge was successful, this indicates the status of the
    /// working tree.
    ///
    /// If a merge conflict has occured and is not resolved, this is the right head of th
    /// merge.
    pub right: StatusCode,
    /// The path for this status entry.
    pub path: Utf8PathBuf,
    /// The path this status entry was renamed from, if any.
    pub renamed_from: Option<Utf8PathBuf>,
}

impl StatusEntry {
    pub fn codes(&self) -> impl Iterator<Item = StatusCode> {
        iter::once(self.left).chain(iter::once(self.right))
    }

    pub fn is_renamed(&self) -> bool {
        self.codes().any(|code| matches!(code, StatusCode::Renamed))
    }

    /// True if the file is not ignored, untracked, or unmodified.
    pub fn is_modified(&self) -> bool {
        self.codes().any(|code| {
            !matches!(
                code,
                StatusCode::Ignored | StatusCode::Untracked | StatusCode::Unmodified
            )
        })
    }

    pub fn is_ignored(&self) -> bool {
        self.codes().any(|code| matches!(code, StatusCode::Ignored))
    }

    pub fn parser(input: &mut &str) -> PResult<Self> {
        let left = StatusCode::parser.parse_next(input)?;
        let right = StatusCode::parser.parse_next(input)?;
        let _ = ' '.parse_next(input)?;
        let path = till_null.parse_next(input)?;

        let mut entry = Self {
            left,
            right,
            path: Utf8PathBuf::from(path),
            renamed_from: None,
        };

        if entry.is_renamed() {
            let renamed_from = till_null.parse_next(input)?;
            entry.renamed_from = Some(Utf8PathBuf::from(renamed_from));
        }

        Ok(entry)
    }
}

impl FromStr for StatusEntry {
    type Err = miette::Report;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parser.parse(input).map_err(|err| miette!("{err}"))
    }
}

impl Display for StatusEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{} ", self.left, self.right)?;
        if let Some(renamed_from) = &self.renamed_from {
            write!(f, "{renamed_from} -> ")?;
        }
        write!(f, "{}", self.path)
    }
}

/// A `git status` listing.
///
/// ```plain
///  M Cargo.lock
///  M Cargo.toml
///  M src/app.rs
///  M src/cli.rs
///  D src/commit_hash.rs
///  D src/git.rs
///  M src/main.rs
///  D src/ref_name.rs
///  D src/worktree.rs
/// ?? src/config.rs
/// ?? src/git/
/// ?? src/utf8tempdir.rs
/// !! target/
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub entries: Vec<StatusEntry>,
}

impl Status {
    #[instrument(level = "trace")]
    pub fn is_clean(&self) -> bool {
        self.entries.iter().all(|entry| !entry.is_modified())
    }

    pub fn parser(input: &mut &str) -> PResult<Self> {
        if opt(eof).parse_next(input)?.is_some() {
            return Ok(Self {
                entries: Vec::new(),
            });
        }

        let (entries, _eof) = repeat_till(1.., StatusEntry::parser, eof).parse_next(input)?;
        Ok(Self { entries })
    }

    pub fn iter(&self) -> std::slice::Iter<'_, StatusEntry> {
        self.entries.iter()
    }
}

impl IntoIterator for Status {
    type Item = StatusEntry;

    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl Deref for Status {
    type Target = Vec<StatusEntry>;

    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl FromStr for Status {
    type Err = miette::Report;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parser.parse(input).map_err(|err| miette!("{err}"))
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_status_parse_empty() {
        assert_eq!(Status::from_str("").unwrap().entries, vec![]);
    }

    #[test]
    fn test_status_parse_complex() {
        assert_eq!(
            Status::from_str(
                &indoc!(
                    " M Cargo.lock
                     M Cargo.toml
                     M src/app.rs
                     M src/cli.rs
                     D src/commit_hash.rs
                     D src/git.rs
                     M src/main.rs
                     D src/ref_name.rs
                     D src/worktree.rs
                    ?? src/config.rs
                    ?? src/git/
                    ?? src/utf8tempdir.rs
                    !! target/
                    "
                )
                .replace('\n', "\0")
            )
            .unwrap()
            .entries,
            vec![
                StatusEntry {
                    left: StatusCode::Unmodified,
                    right: StatusCode::Modified,
                    path: "Cargo.lock".into(),
                    renamed_from: None,
                },
                StatusEntry {
                    left: StatusCode::Unmodified,
                    right: StatusCode::Modified,
                    path: "Cargo.toml".into(),
                    renamed_from: None,
                },
                StatusEntry {
                    left: StatusCode::Unmodified,
                    right: StatusCode::Modified,
                    path: "src/app.rs".into(),
                    renamed_from: None,
                },
                StatusEntry {
                    left: StatusCode::Unmodified,
                    right: StatusCode::Modified,
                    path: "src/cli.rs".into(),
                    renamed_from: None,
                },
                StatusEntry {
                    left: StatusCode::Unmodified,
                    right: StatusCode::Deleted,
                    path: "src/commit_hash.rs".into(),
                    renamed_from: None,
                },
                StatusEntry {
                    left: StatusCode::Unmodified,
                    right: StatusCode::Deleted,
                    path: "src/git.rs".into(),
                    renamed_from: None,
                },
                StatusEntry {
                    left: StatusCode::Unmodified,
                    right: StatusCode::Modified,
                    path: "src/main.rs".into(),
                    renamed_from: None,
                },
                StatusEntry {
                    left: StatusCode::Unmodified,
                    right: StatusCode::Deleted,
                    path: "src/ref_name.rs".into(),
                    renamed_from: None,
                },
                StatusEntry {
                    left: StatusCode::Unmodified,
                    right: StatusCode::Deleted,
                    path: "src/worktree.rs".into(),
                    renamed_from: None,
                },
                StatusEntry {
                    left: StatusCode::Untracked,
                    right: StatusCode::Untracked,
                    path: "src/config.rs".into(),
                    renamed_from: None,
                },
                StatusEntry {
                    left: StatusCode::Untracked,
                    right: StatusCode::Untracked,
                    path: "src/git/".into(),
                    renamed_from: None,
                },
                StatusEntry {
                    left: StatusCode::Untracked,
                    right: StatusCode::Untracked,
                    path: "src/utf8tempdir.rs".into(),
                    renamed_from: None,
                },
                StatusEntry {
                    left: StatusCode::Ignored,
                    right: StatusCode::Ignored,
                    path: "target/".into(),
                    renamed_from: None,
                },
            ]
        );
    }

    #[test]
    fn test_status_parse_renamed() {
        assert_eq!(
            Status::from_str("R  PUPPY.md\0README.md\0")
                .unwrap()
                .entries,
            vec![StatusEntry {
                left: StatusCode::Renamed,
                right: StatusCode::Unmodified,
                path: "PUPPY.md".into(),
                renamed_from: Some("README.md".into()),
            }]
        );
    }
}
