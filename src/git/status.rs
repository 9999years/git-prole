use std::fmt::Debug;
use std::iter;
use std::str::FromStr;

use camino::Utf8PathBuf;
use command_error::CommandExt;
use command_error::OutputContext;
use miette::miette;
use miette::IntoDiagnostic;
use tracing::instrument;
use utf8_command::Utf8Output;
use winnow::combinator::eof;
use winnow::combinator::opt;
use winnow::combinator::repeat_till;
use winnow::token::one_of;
use winnow::PResult;
use winnow::Parser;

use crate::parse::till_null;

use super::Git;

/// Git methods for dealing with statuses and the working tree.
#[repr(transparent)]
pub struct GitStatus<'a>(&'a Git);

impl Debug for GitStatus<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.0, f)
    }
}

impl<'a> GitStatus<'a> {
    pub fn new(git: &'a Git) -> Self {
        Self(git)
    }

    #[expect(dead_code)] // #[instrument(level = "trace")]
    pub(crate) fn get(&self) -> miette::Result<Status> {
        self.0
            .command()
            .args(["status", "--porcelain=v1", "--ignored=traditional", "-z"])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if context.status().success() {
                    Status::from_str(&context.output().stdout).map_err(|err| context.error_msg(err))
                } else {
                    Err(context.error())
                }
            })
            .into_diagnostic()
    }

    /// List untracked files and directories.
    #[instrument(level = "trace")]
    pub fn untracked_files(&self) -> miette::Result<Vec<Utf8PathBuf>> {
        Ok(self
            .0
            .command()
            .args([
                "ls-files",
                // Show untracked (e.g. ignored) files.
                "--others",
                // If a whole directory is classified as other, show just its name and not its
                // whole contents.
                "--directory",
                "-z",
            ])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(Utf8PathBuf::from)
            .collect())
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

/// The status of a particular file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    /// If no merge is occurring, or a merge was successful, this indicates the status of the
    /// index.
    ///
    /// If a merge conflict has occured and is not resolved, this is the left head of th
    /// merge.
    left: StatusCode,
    /// If no merge is occurring, or a merge was successful, this indicates the status of the
    /// working tree.
    ///
    /// If a merge conflict has occured and is not resolved, this is the right head of th
    /// merge.
    right: StatusCode,
    path: Utf8PathBuf,
    renamed_from: Option<Utf8PathBuf>,
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
#[derive(Debug, Clone)]
pub struct Status {
    entries: Vec<StatusEntry>,
}

impl Status {
    #[expect(dead_code)]
    pub(crate) fn is_clean(&self) -> bool {
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
