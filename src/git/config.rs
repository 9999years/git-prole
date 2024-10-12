use std::fmt::Debug;

use command_error::CommandExt;
use command_error::OutputContext;
use miette::IntoDiagnostic;
use tracing::instrument;
use utf8_command::Utf8Output;

use super::Git;

/// Git methods for dealing with config.
#[repr(transparent)]
pub struct GitConfig<'a>(&'a Git);

impl Debug for GitConfig<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.0, f)
    }
}

impl<'a> GitConfig<'a> {
    pub fn new(git: &'a Git) -> Self {
        Self(git)
    }

    /// Get a config setting by name.
    #[instrument(level = "trace")]
    pub fn get(&self, key: &str) -> miette::Result<Option<String>> {
        self.0
            .command()
            .args(["config", "get", "--null", key])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if context.status().success() {
                    // TODO: Should this be a winnow parser?
                    match context.output().stdout.as_str().split_once('\0') {
                        Some((value, rest)) => {
                            if !rest.is_empty() {
                                tracing::warn!(
                                    %key,
                                    data=rest,
                                    "Trailing data in `git config` output"
                                );
                            }
                            Ok(Some(value.to_owned()))
                        }
                        None => Err(context.error_msg("Output didn't contain any null bytes")),
                    }
                } else if let Some(1) = context.status().code() {
                    Ok(None)
                } else {
                    Err(context.error())
                }
            })
            .into_diagnostic()
    }

    /// Set a local config setting.
    #[instrument(level = "trace")]
    pub fn set(&self, key: &str, value: &str) -> miette::Result<()> {
        self.0
            .command()
            .args(["config", "set", key, value])
            .output_checked_utf8()
            .into_diagnostic()?;
        Ok(())
    }
}
