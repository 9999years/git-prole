use std::fmt::Debug;
use std::str::FromStr;
use std::sync::OnceLock;

use command_error::CommandExt;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;
use regex::Regex;
use tracing::instrument;

use super::ref_name::Ref;
use super::Git;

/// Git methods for dealing with remotes.
#[repr(transparent)]
pub struct GitRemote<'a>(&'a Git);

impl Debug for GitRemote<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.0, f)
    }
}

impl<'a> GitRemote<'a> {
    pub fn new(git: &'a Git) -> Self {
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
            .into_diagnostic()
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
            .into_diagnostic()
            .wrap_err("Failed to get Git remote URL")?
            .stdout
            .trim()
            .to_owned())
    }

    #[instrument(level = "trace")]
    fn default_branch_symbolic_ref(&self, remote: &str) -> miette::Result<String> {
        let output = self
            .0
            .command()
            .args([
                "symbolic-ref",
                "--short",
                &format!("refs/remotes/{remote}/HEAD"),
            ])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout;

        static RE: OnceLock<Regex> = OnceLock::new();
        let captures = RE
            .get_or_init(|| {
                Regex::new(
                    r"(?xm)
                    ^
                    (?P<remote>[[:word:]]+)/(?P<branch>.+)
                    $
                    ",
                )
                .expect("Regex parses")
            })
            .captures(&output);

        match captures {
            Some(captures) => Ok(captures["branch"].to_owned()),
            None => Err(miette!(
                "Could not parse `git symbolic-ref` output:\n{output}"
            )),
        }
    }

    #[instrument(level = "trace")]
    fn default_branch_ls_remote(&self, remote: &str) -> miette::Result<String> {
        let output = self
            .0
            .command()
            .args(["ls-remote", "--symref", remote, "HEAD"])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout;

        static RE: OnceLock<Regex> = OnceLock::new();
        let captures = RE
            .get_or_init(|| {
                Regex::new(
                    r"(?xm)
                    ^
                    ref:\ refs/heads/(?P<branch>[^\t]+)\tHEAD
                    $
                    ",
                )
                .expect("Regex parses")
            })
            .captures(&output);

        let branch = match captures {
            Some(captures) => Ok(captures["branch"].to_owned()),
            None => Err(miette!("Could not parse `git ls-remote` output:\n{output}")),
        }?;

        // To avoid talking to the remote next time, write a symbolic-ref.
        self.0
            .command()
            .args([
                "symbolic-ref",
                &format!("refs/remotes/{remote}/HEAD"),
                &format!("refs/remotes/{remote}/{branch}"),
            ])
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err_with(|| {
                format!("Failed to store symbolic ref for default branch for remote {remote}")
            })?;

        Ok(branch)
    }

    /// Get the default branch for the given remote.
    #[instrument(level = "trace")]
    pub fn default_branch(&self, remote: &str) -> miette::Result<String> {
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
    pub fn for_branch(&self, branch: &str) -> miette::Result<Option<String>> {
        let refs = self
            .0
            .command()
            .args([
                "for-each-ref",
                "--format=%(refname)",
                &format!("refs/remotes/*/{branch}"),
            ])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout;

        let mut exists_on_remotes = Vec::new();

        for ref_name in refs.lines() {
            let parsed_ref = Ref::from_str(ref_name)?;
            match parsed_ref.remote_and_branch() {
                Some((remote, _branch)) => {
                    exists_on_remotes.push(remote.to_owned());
                }
                None => {
                    unreachable!()
                }
            }
        }

        if exists_on_remotes.is_empty() {
            Ok(None)
        } else if exists_on_remotes.len() == 1 {
            Ok(exists_on_remotes.pop())
        } else if let Some(default_remote) = self.get_default()? {
            // if-let chains when?
            if exists_on_remotes.contains(&default_remote) {
                Ok(Some(default_remote))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}
