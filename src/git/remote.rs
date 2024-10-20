use std::fmt::Debug;
use std::str::FromStr;

use camino::Utf8Path;
use command_error::CommandExt;
use command_error::OutputContext;
use miette::miette;
use miette::Context;
use rustc_hash::FxHashSet;
use tap::TryConv;
use tracing::instrument;
use utf8_command::Utf8Output;
use winnow::combinator::rest;
use winnow::token::take_till;
use winnow::PResult;
use winnow::Parser;

use crate::AppGit;

use super::GitLike;
use super::LocalBranchRef;
use super::Ref;
use super::RemoteBranchRef;

/// Git methods for dealing with remotes.
#[repr(transparent)]
pub struct GitRemote<'a, G>(&'a G);

impl<G> Debug for GitRemote<'_, G>
where
    G: GitLike,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("GitRemote")
            .field(&self.0.get_current_dir().as_ref())
            .finish()
    }
}

impl<'a, G> GitRemote<'a, G>
where
    G: GitLike,
{
    pub fn new(git: &'a G) -> Self {
        Self(git)
    }

    /// Get a list of all `git remote`s.
    #[instrument(level = "trace")]
    pub fn list(&self) -> miette::Result<Vec<String>> {
        Ok(self
            .0
            .command()
            .arg("remote")
            .output_checked_utf8()
            .wrap_err("Failed to list Git remotes")?
            .stdout
            .lines()
            .map(|line| line.to_owned())
            .collect())
    }

    /// Get the (push) URL for the given remote.
    #[expect(dead_code)] // #[instrument(level = "trace")]
    pub(crate) fn get_push_url(&self, remote: &str) -> miette::Result<String> {
        Ok(self
            .0
            .command()
            .args(["remote", "get-url", "--push", remote])
            .output_checked_utf8()
            .wrap_err("Failed to get Git remote URL")?
            .stdout
            .trim()
            .to_owned())
    }

    #[instrument(level = "trace")]
    fn default_branch_symbolic_ref(&self, remote: &str) -> miette::Result<RemoteBranchRef> {
        Ok(self
            .0
            .command()
            .args(["symbolic-ref", &format!("refs/remotes/{remote}/HEAD")])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if !context.status().success() {
                    Err(context.error())
                } else {
                    let output = context.output().stdout.trim_end();
                    match Ref::from_str(output) {
                        Err(err) => Err(context.error_msg(err)),
                        Ok(ref_name) => match ref_name.try_conv::<RemoteBranchRef>() {
                            Ok(remote_branch) => Ok(remote_branch),
                            Err(err) => Err(context.error_msg(format!("{err}"))),
                        },
                    }
                }
            })?)
    }

    #[instrument(level = "trace")]
    fn default_branch_ls_remote(&self, remote: &str) -> miette::Result<RemoteBranchRef> {
        let branch = self
            .0
            .command()
            .args(["ls-remote", "--symref", remote, "HEAD"])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if !context.status().success() {
                    Err(context.error())
                } else {
                    let output = &context.output().stdout;
                    match parse_ls_remote_symref.parse(output) {
                        Err(err) => {
                            let err = miette!("{err}");
                            Err(context.error_msg(err))
                        }
                        Ok(ref_name) => match ref_name.try_conv::<LocalBranchRef>() {
                            Ok(local_branch) => Ok(local_branch.on_remote(remote)),
                            Err(err) => Err(context.error_msg(format!("{err}"))),
                        },
                    }
                }
            })?;

        // To avoid talking to the remote next time, write a symbolic-ref.
        self.0
            .command()
            .args([
                "symbolic-ref",
                &format!("refs/remotes/{remote}/HEAD"),
                &format!("refs/remotes/{remote}/{branch}"),
            ])
            .output_checked_utf8()
            .wrap_err_with(|| {
                format!("Failed to store symbolic ref for default branch for remote {remote}")
            })?;

        Ok(branch)
    }

    /// Get the default branch for the given remote.
    #[instrument(level = "trace")]
    pub fn default_branch(&self, remote: &str) -> miette::Result<RemoteBranchRef> {
        self.default_branch_symbolic_ref(remote).or_else(|err| {
            tracing::debug!("Failed to get default branch: {err}");
            self.default_branch_ls_remote(remote)
        })
    }

    /// Get the `checkout.defaultRemote` setting.
    #[instrument(level = "trace")]
    pub fn get_default(&self) -> miette::Result<Option<String>> {
        self.0.config().get("checkout.defaultRemote")
    }

    /// Find a unique remote branch by name.
    ///
    /// The discovered remote, if any, is returned.
    ///
    /// This is (hopefully!) how Git determines which remote-tracking branch you want when you do a
    /// `git switch` or `git worktree add`.
    #[instrument(level = "trace")]
    pub fn for_branch(&self, branch: &str) -> miette::Result<Option<RemoteBranchRef>> {
        let mut exists_on_remotes = self
            .0
            .refs()
            .for_each_ref(Some(&[&format!("refs/remotes/*/{branch}")]))?;

        if exists_on_remotes.is_empty() {
            Ok(None)
        } else if exists_on_remotes.len() == 1 {
            Ok(exists_on_remotes.pop().map(|ref_name| {
                RemoteBranchRef::try_from(ref_name)
                    .expect("`for-each-ref` restricted to `refs/remotes/*` refs")
            }))
        } else if let Some(default_remote) = self.get_default()? {
            // if-let chains when?
            match exists_on_remotes
                .into_iter()
                .map(|ref_name| {
                    RemoteBranchRef::try_from(ref_name)
                        .expect("`for-each-ref` restricted to `refs/remotes/*` refs")
                })
                .find(|branch| branch.remote() == default_remote)
            {
                Some(remote) => Ok(Some(remote)),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    /// Fetch a refspec from a remote.
    #[instrument(level = "trace")]
    pub fn fetch(&self, remote: &str, refspec: Option<&str>) -> miette::Result<()> {
        let mut command = self.0.command();
        command.args(["fetch", remote]);
        if let Some(refspec) = refspec {
            command.arg(refspec);
        }
        command.status_checked()?;
        Ok(())
    }
}

impl<'a, C> GitRemote<'a, AppGit<'a, C>>
where
    C: AsRef<Utf8Path>,
{
    /// Get a list of remotes in the user's preference order.
    #[instrument(level = "trace")]
    pub fn list_preferred(&self) -> miette::Result<Vec<String>> {
        let mut all_remotes = self.list()?.into_iter().collect::<FxHashSet<_>>();

        let mut sorted = Vec::with_capacity(all_remotes.len());

        if let Some(default_remote) = self.get_default()? {
            if let Some(remote) = all_remotes.take(&default_remote) {
                sorted.push(remote);
            }
        }

        let preferred_remotes = self.0.config.file.remotes();
        for remote in preferred_remotes {
            if let Some(remote) = all_remotes.take(&remote) {
                sorted.push(remote);
            }
        }

        Ok(sorted)
    }

    /// Get the user's preferred remote, if any.
    #[instrument(level = "trace")]
    pub fn preferred(&self) -> miette::Result<Option<String>> {
        Ok(self.list_preferred()?.first().cloned())
    }
}

/// Parse a symbolic ref from the start of `git ls-remote --symref` output.
fn parse_ls_remote_symref(input: &mut &str) -> PResult<Ref> {
    let _ = "ref: ".parse_next(input)?;
    let ref_name = take_till(1.., '\t')
        .and_then(Ref::parser)
        .parse_next(input)?;
    let _ = '\t'.parse_next(input)?;
    // Don't care about the rest!
    let _ = rest.parse_next(input)?;
    Ok(ref_name)
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_parse_ls_remote_symref() {
        assert_eq!(
            parse_ls_remote_symref
                .parse(indoc!(
                    "
                    ref: refs/heads/main\tHEAD
                    9afc843b4288394fe3a2680b13070cfd53164b92\tHEAD
                    "
                ))
                .unwrap(),
            Ref::from_str("refs/heads/main").unwrap(),
        );
    }
}
