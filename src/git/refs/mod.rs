use std::fmt::Debug;
use std::str::FromStr;

use command_error::CommandExt;
use command_error::OutputContext;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;
use tap::Tap;
use tracing::instrument;
use utf8_command::Utf8Output;

use super::commit_hash::CommitHash;
use super::commitish::ResolvedCommitish;
use super::head_state::HeadKind;
use super::Git;

mod branch;
mod local_branch;
mod name;
mod remote_branch;

pub use branch::BranchRef;
pub use local_branch::LocalBranchRef;
pub use name::Ref;
pub use remote_branch::RemoteBranchRef;

/// Git methods for dealing with refs.
#[repr(transparent)]
pub struct GitRefs<'a>(&'a Git);

impl Debug for GitRefs<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.0, f)
    }
}

impl<'a> GitRefs<'a> {
    pub fn new(git: &'a Git) -> Self {
        Self(git)
    }

    #[expect(dead_code)] // #[instrument(level = "trace")]
    pub(crate) fn commit_message(&self, commit: &str) -> miette::Result<String> {
        Ok(self
            .0
            .command()
            .args(["show", "--no-patch", "--format=%B", commit])
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err("Failed to get commit message")?
            .stdout)
    }

    /// Get the `HEAD` commit hash.
    #[instrument(level = "trace")]
    pub fn get_head(&self) -> miette::Result<CommitHash> {
        Ok(self.parse("HEAD")?.expect("HEAD always exists"))
    }

    /// Parse a `commitish` into a commit hash.
    #[instrument(level = "trace")]
    pub fn parse(&self, commitish: &str) -> miette::Result<Option<CommitHash>> {
        self.0
            .rev_parse_command()
            .args(["--verify", "--quiet", "--end-of-options", commitish])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if context.status().success() {
                    Ok::<_, command_error::Error>(Some(CommitHash::new(
                        context.output().stdout.trim().to_owned(),
                    )))
                } else {
                    Ok(None)
                }
            })
            .into_diagnostic()
    }

    /// `git rev-parse --symbolic-full-name`
    #[instrument(level = "trace")]
    pub fn rev_parse_symbolic_full_name(&self, commitish: &str) -> miette::Result<Option<Ref>> {
        self.0
            .rev_parse_command()
            .args([
                "--symbolic-full-name",
                "--verify",
                "--quiet",
                "--end-of-options",
                commitish,
            ])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if context.status().success() {
                    let trimmed = context.output().stdout.trim();
                    if trimmed.is_empty() {
                        Ok(None)
                    } else {
                        match Ref::from_str(trimmed) {
                            Ok(parsed) => Ok(Some(parsed)),
                            Err(err) => {
                                if commitish.ends_with("HEAD") && trimmed == commitish {
                                    tracing::debug!("{commitish} is detached");
                                    Ok(None)
                                } else {
                                    Err(context.error_msg(err))
                                }
                            }
                        }
                    }
                } else {
                    Ok(None)
                }
            })
            .into_diagnostic()
    }

    /// Determine if a given `<commit-ish>` refers to a commit or a symbolic ref name.
    #[instrument(level = "trace")]
    pub fn resolve_commitish(&self, commitish: &str) -> miette::Result<ResolvedCommitish> {
        match self.rev_parse_symbolic_full_name(commitish)? {
            Some(ref_name) => Ok(ResolvedCommitish::Ref(ref_name)),
            None => Ok(ResolvedCommitish::Commit(
                self.parse(commitish)?.ok_or_else(|| {
                    miette!("Commitish could not be resolved to a ref or commit hash: {commitish}")
                })?,
            )),
        }
    }

    #[instrument(level = "trace")]
    pub fn is_head_detached(&self) -> miette::Result<bool> {
        let output = self
            .0
            .command()
            .args(["symbolic-ref", "--quiet", "HEAD"])
            .output_checked_with_utf8::<String>(|_output| Ok(()))
            .into_diagnostic()?;

        Ok(!output.status.success())
    }

    /// Figure out what's going on with `HEAD`.
    #[instrument(level = "trace")]
    pub fn head_kind(&self) -> miette::Result<HeadKind> {
        Ok(if self.is_head_detached()? {
            HeadKind::Detached(self.get_head()?)
        } else {
            HeadKind::Branch(
                LocalBranchRef::try_from(
                    self.rev_parse_symbolic_full_name("HEAD")?
                        .expect("Non-detached HEAD should always be a valid ref"),
                )
                .expect("Non-detached HEAD should always be a local branch"),
            )
        })
    }

    #[instrument(level = "trace")]
    pub fn for_each_ref(&self, globs: Option<&[&str]>) -> miette::Result<Vec<Ref>> {
        self.0
            .command()
            .args(["for-each-ref", "--format=%(refname)"])
            .tap_mut(|c| {
                globs.map(|globs| c.args(globs));
            })
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout
            .lines()
            .map(Ref::from_str)
            .collect()
    }
}
