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
    pub fn get(&self) -> miette::Result<Status> {
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
#[derive(Debug, Clone, Copy)]
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
    pub fn parse(status: char) -> Option<Self> {
        Some(match status {
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
                return None;
            }
        })
    }
}

/// The status of a particular file.
#[derive(Debug, Clone)]
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
    #[expect(dead_code)]
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
    pub fn is_clean(&self) -> bool {
        self.entries.iter().all(|entry| !entry.is_modified())
    }
}

impl FromStr for Status {
    type Err = miette::Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Ok(Self {
                entries: Vec::new(),
            });
        }

        let mut entries = Vec::new();
        let mut tokens = s.trim_end_matches('\0').split('\0');

        while let Some(token) = tokens.next() {
            let (status, path) = token
                .split_at_checked(2)
                .ok_or_else(|| miette!("`git status` output is weird: {token:?}"))?;

            let mut status_chars = status.chars();
            let left = status_chars
                .next()
                .ok_or_else(|| miette!("`git status` output missing status: {token:?}"))?;
            let left = StatusCode::parse(left)
                .ok_or_else(|| miette!("Unknown `git status` code {left} in: {token:?}"))?;
            let right = status_chars
                .next()
                .ok_or_else(|| miette!("`git status` output missing status: {token:?}"))?;
            let right = StatusCode::parse(right)
                .ok_or_else(|| miette!("Unknown `git status` code {right} in: {token:?}"))?;

            let mut entry = StatusEntry {
                left,
                right,
                path: Utf8PathBuf::from(path),
                renamed_from: None,
            };

            if entry.is_renamed() {
                let renamed_from = tokens.next().ok_or_else(|| {
                    miette!("Renamed `git status` entry has no 'renamed from' path: {token:?}")
                })?;

                entry.renamed_from = Some(Utf8PathBuf::from(renamed_from));
            }

            entries.push(entry);
        }

        Ok(Self { entries })
    }
}
