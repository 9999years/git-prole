use std::process::Command;

use camino::Utf8PathBuf;
use clap::Parser;
use miette::Context;
use miette::IntoDiagnostic;
use serde::de::Error;
use serde::Deserialize;
use unindent::unindent;
use xdg::BaseDirectories;

use crate::cli::Cli;
use crate::fs;
use crate::install_tracing::install_tracing;

/// Configuration, both from the command-line and a user configuration file.
#[derive(Debug)]
pub struct Config {
    /// User directories.
    #[expect(dead_code)]
    pub(crate) dirs: BaseDirectories,
    /// User configuration file.
    pub file: ConfigFile,
    /// User configuration file path.
    pub path: Utf8PathBuf,
    /// Command-line options.
    pub cli: Cli,
}

impl Config {
    /// The contents of the default configuration file.
    pub const DEFAULT: &str = include_str!("../config.toml");

    pub fn new() -> miette::Result<Self> {
        let cli = Cli::parse();
        // TODO: add tracing settings to the config file
        install_tracing(&cli.log)?;
        let dirs = BaseDirectories::with_prefix("git-prole").into_diagnostic()?;
        const CONFIG_FILE_NAME: &str = "config.toml";
        // TODO: Use `git config` for configuration?
        let path = cli
            .config
            .as_ref()
            .map(|path| Ok(path.join(CONFIG_FILE_NAME)))
            .unwrap_or_else(|| dirs.get_config_file(CONFIG_FILE_NAME).try_into())
            .into_diagnostic()?;
        let file = {
            if !path.exists() {
                ConfigFile::default()
            } else {
                toml::from_str(
                    &fs::read_to_string(&path).wrap_err("Failed to read configuration file")?,
                )
                .into_diagnostic()
                .wrap_err("Failed to deserialize configuration file")?
            }
        };
        Ok(Self {
            dirs,
            path,
            file,
            cli,
        })
    }
}

/// Configuration file format.
///
/// Each configuration key should have two test cases:
/// - `config_{key}` for setting the value.
/// - `config_{key}_default` for the default value.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
pub struct ConfigFile {
    #[serde(default)]
    remotes: Vec<String>,

    #[serde(default)]
    default_branches: Vec<String>,

    #[serde(default)]
    copy_untracked: Option<bool>,

    #[serde(default)]
    enable_gh: Option<bool>,

    #[serde(default)]
    commands: Vec<ShellCommand>,
}

impl ConfigFile {
    pub fn remotes(&self) -> Vec<String> {
        // Yeah this basically sucks. But how big could these lists really be?
        if self.remotes.is_empty() {
            vec!["upstream".to_owned(), "origin".to_owned()]
        } else {
            self.remotes.clone()
        }
    }

    pub fn default_branches(&self) -> Vec<String> {
        // Yeah this basically sucks. But how big could these lists really be?
        if self.default_branches.is_empty() {
            vec!["main".to_owned(), "master".to_owned(), "trunk".to_owned()]
        } else {
            self.default_branches.clone()
        }
    }

    pub fn copy_untracked(&self) -> bool {
        self.copy_untracked.unwrap_or(true)
    }

    pub fn enable_gh(&self) -> bool {
        self.enable_gh.unwrap_or(false)
    }

    pub fn commands(&self) -> Vec<ShellCommand> {
        self.commands.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum ShellCommand {
    Simple(ShellArgs),
    Shell { sh: String },
}

impl ShellCommand {
    pub fn as_command(&self) -> Command {
        match self {
            ShellCommand::Simple(args) => {
                let mut command = Command::new(&args.program);
                command.args(&args.args);
                command
            }
            ShellCommand::Shell { sh } => {
                let mut command = Command::new("sh");
                let sh = unindent(sh);
                command.args(["-c", sh.trim_ascii()]);
                command
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellArgs {
    program: String,
    args: Vec<String>,
}

impl<'de> Deserialize<'de> for ShellArgs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let quoted: String = Deserialize::deserialize(deserializer)?;
        let mut args = shell_words::split(&quoted).map_err(D::Error::custom)?;

        if args.is_empty() {
            return Err(D::Error::invalid_value(
                serde::de::Unexpected::Str(&quoted),
                // TODO: This error message doesn't actually get propagated upward
                // correctly, so you get "data did not match any variant of untagged enum
                // ShellCommand" instead.
                &"a shell command (you are missing a program)",
            ));
        }

        let program = args.remove(0);

        Ok(Self { program, args })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_default_config_file_parse() {
        assert_eq!(
            toml::from_str::<ConfigFile>(Config::DEFAULT).unwrap(),
            ConfigFile {
                remotes: vec!["upstream".to_owned(), "origin".to_owned(),],
                default_branches: vec!["main".to_owned(), "master".to_owned(), "trunk".to_owned(),],
                copy_untracked: Some(true),
                enable_gh: Some(false),
                commands: vec![],
            }
        );
    }
}
