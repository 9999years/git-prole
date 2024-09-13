use std::process::Command;
use std::sync::OnceLock;

use camino::Utf8PathBuf;
use command_error::CommandExt;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;
use regex::Regex;

use crate::commit_hash::CommitHash;

/// `git` CLI wrapper.
#[derive(Debug, Default)]
pub struct Git {}

impl Git {
    #[expect(dead_code)]
    pub fn new() -> Self {
        Default::default()
    }

    /// Get a `git` command.
    pub fn command(&self) -> Command {
        Command::new("git")
    }

    /// Get a list of all `git remote`s.
    #[expect(dead_code)]
    pub fn remotes(&self) -> miette::Result<Vec<String>> {
        Ok(self
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
    #[expect(dead_code)]
    pub fn remote_url(&self, remote: &str) -> miette::Result<String> {
        Ok(self
            .command()
            .args(["remote", "get-url", "--push", remote])
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err("Failed to get Git remote URL")?
            .stdout
            .trim()
            .to_owned())
    }

    fn default_branch_symbolic_ref(&self, remote: &str) -> miette::Result<String> {
        let output = self
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
                    (?P<remote>[[:word:]]+)/(?P<branch>[[:word:]]+)
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

    fn default_branch_ls_remote(&self, remote: &str) -> miette::Result<String> {
        let output = self
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
                    ref: refs/heads/(?P<branch>[[:word:]]+)\tHEAD
                    $
                    ",
                )
                .expect("Regex parses")
            })
            .captures(&output);

        match captures {
            Some(captures) => Ok(captures["branch"].to_owned()),
            None => Err(miette!("Could not parse `git ls-remote` output:\n{output}")),
        }
    }

    #[expect(dead_code)]
    pub fn default_branch(&self, remote: &str) -> miette::Result<String> {
        self.default_branch_symbolic_ref(remote).or_else(|err| {
            tracing::debug!("Failed to get default branch: {err}");
            self.default_branch_ls_remote(remote)
        })
    }

    #[expect(dead_code)]
    pub fn commit_message(&self, commit: &str) -> miette::Result<String> {
        Ok(self
            .command()
            .args(["show", "--no-patch", "--format=%B", commit])
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err("Failed to get commit message")?
            .stdout)
    }

    /// Get the `HEAD` commit hash.
    #[expect(dead_code)]
    pub fn get_head(&self) -> miette::Result<CommitHash> {
        self.rev_parse("HEAD")
    }

    /// Get the `.git` directory path.
    #[expect(dead_code)]
    pub fn get_git_dir(&self) -> miette::Result<Utf8PathBuf> {
        self.command()
            .args(["rev-parse", "--git-dir"])
            .output_checked_utf8()
            .into_diagnostic()
            .map(|output| Utf8PathBuf::from(output.stdout.trim()))
    }

    pub fn rev_parse(&self, commitish: &str) -> miette::Result<CommitHash> {
        Ok(CommitHash::new(
            self.command()
                .args(["rev-parse", commitish])
                .output_checked_utf8()
                .into_diagnostic()?
                .stdout
                .trim()
                .to_owned(),
        ))
    }
}
