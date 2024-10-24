use std::fmt::Debug;

use command_error::CommandExt;
use command_error::OutputContext;
use miette::miette;
use tracing::instrument;
use utf8_command::Utf8Output;

use super::GitLike;

/// Git methods for dealing with config.
#[repr(transparent)]
pub struct GitConfig<'a, G>(&'a G);

impl<G> Debug for GitConfig<'_, G>
where
    G: GitLike,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("GitConfig")
            .field(&self.0.get_current_dir().as_ref())
            .finish()
    }
}

impl<'a, G> GitConfig<'a, G>
where
    G: GitLike,
{
    pub fn new(git: &'a G) -> Self {
        Self(git)
    }

    /// Get a config setting by name and parse a value out of it.
    pub fn get_and<R>(
        &self,
        key: &str,
        parser: impl Fn(OutputContext<Utf8Output>, Option<String>) -> Result<R, command_error::Error>,
    ) -> miette::Result<R> {
        Ok(self
            .0
            .command()
            .args(["config", "--get", "--null", key])
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
                            let value = value.to_owned();
                            parser(context, Some(value))
                        }
                        None => Err(context.error_msg("Output didn't contain any null bytes")),
                    }
                } else if let Some(1) = context.status().code() {
                    parser(context, None)
                } else {
                    Err(context.error())
                }
            })?)
    }

    /// Get a config setting by name.
    #[instrument(level = "trace")]
    pub fn get(&self, key: &str) -> miette::Result<Option<String>> {
        self.get_and(key, |_, value| Ok(value))
    }

    /// Check if this repository is bare.
    #[instrument(level = "trace")]
    pub fn is_bare(&self) -> miette::Result<bool> {
        self.get_and("core.bare", |context, value| {
            match value {
                None => {
                    // This seems to not happen in practice, but whatever.
                    Ok(false)
                }
                Some(value) => match value.as_str() {
                    "true" => Ok(true),
                    "false" => Ok(false),
                    _ => Err(context.error_msg(miette!(
                        "Unexpected Git config value for `core.bare`: {value}"
                    ))),
                },
            }
        })
    }

    /// Set a local config setting.
    #[instrument(level = "trace")]
    pub fn set(&self, key: &str, value: &str) -> miette::Result<()> {
        self.0
            .command()
            .args(["config", "set", key, value])
            .output_checked_utf8()?;
        Ok(())
    }
}
