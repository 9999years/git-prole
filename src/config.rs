use std::process::Command;

use camino::Utf8PathBuf;
use clap::Parser;
use miette::Context;
use miette::IntoDiagnostic;
use regex::Regex;
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
        // TODO: Use `git config` for configuration?
        let path = cli
            .config
            .as_ref()
            .map(|path| Ok(path.to_owned()))
            .unwrap_or_else(|| config_file_path(&dirs))?;
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

    /// A fake stub config for testing.
    #[cfg(test)]
    pub fn test_stub() -> Self {
        // TODO: Make this pure-er.
        let dirs = BaseDirectories::new().unwrap();
        let path = config_file_path(&dirs).unwrap();
        Self {
            dirs,
            file: ConfigFile::default(),
            path,
            cli: Cli::test_stub(),
        }
    }
}

fn config_file_path(dirs: &BaseDirectories) -> miette::Result<Utf8PathBuf> {
    dirs.get_config_file(ConfigFile::FILE_NAME)
        .try_into()
        .into_diagnostic()
}

/// Configuration file format.
///
/// Each configuration key should have two test cases:
/// - `config_{key}` for setting the value.
/// - `config_{key}_default` for the default value.
///
/// For documentation, see the default configuration file (`../config.toml`).
///
/// The default configuration file is accessible as [`Config::DEFAULT`].
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ConfigFile {
    remote_names: Vec<String>,
    branch_names: Vec<String>,
    pub clone: CloneConfig,
    pub add: AddConfig,
}

impl ConfigFile {
    pub const FILE_NAME: &str = "config.toml";

    pub fn remote_names(&self) -> Vec<String> {
        // Yeah this basically sucks. But how big could these lists really be?
        if self.remote_names.is_empty() {
            vec!["upstream".to_owned(), "origin".to_owned()]
        } else {
            self.remote_names.clone()
        }
    }

    pub fn branch_names(&self) -> Vec<String> {
        // Yeah this basically sucks. But how big could these lists really be?
        if self.branch_names.is_empty() {
            vec!["main".to_owned(), "master".to_owned(), "trunk".to_owned()]
        } else {
            self.branch_names.clone()
        }
    }
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CloneConfig {
    enable_gh: Option<bool>,
}

impl CloneConfig {
    pub fn enable_gh(&self) -> bool {
        self.enable_gh.unwrap_or(false)
    }
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AddConfig {
    copy_untracked: Option<bool>,
    copy_ignored: Option<bool>,
    commands: Vec<ShellCommand>,
    branch_replacements: Vec<BranchReplacement>,
}

impl AddConfig {
    pub fn copy_ignored(&self) -> bool {
        if let Some(copy_untracked) = self.copy_untracked {
            tracing::warn!("`add.copy_untracked` has been replaced with `add.copy_ignored`");
            return copy_untracked;
        }
        self.copy_ignored.unwrap_or(true)
    }

    pub fn commands(&self) -> &[ShellCommand] {
        &self.commands
    }

    pub fn branch_replacements(&self) -> &[BranchReplacement] {
        &self.branch_replacements
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

#[derive(Clone, Debug, Deserialize)]
pub struct BranchReplacement {
    #[serde(deserialize_with = "deserialize_regex")]
    pub find: Regex,
    pub replace: String,
    pub count: Option<usize>,
}

impl PartialEq for BranchReplacement {
    fn eq(&self, other: &Self) -> bool {
        self.replace == other.replace && self.find.as_str() == other.find.as_str()
    }
}

impl Eq for BranchReplacement {}

fn deserialize_regex<'de, D>(deserializer: D) -> Result<Regex, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let input: String = Deserialize::deserialize(deserializer)?;
    Regex::new(&input).map_err(D::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_default_config_file_parse() {
        let default_config = toml::from_str::<ConfigFile>(Config::DEFAULT).unwrap();
        assert_eq!(
            default_config,
            ConfigFile {
                remote_names: vec!["upstream".to_owned(), "origin".to_owned(),],
                branch_names: vec!["main".to_owned(), "master".to_owned(), "trunk".to_owned(),],
                clone: CloneConfig {
                    enable_gh: Some(false)
                },
                add: AddConfig {
                    copy_untracked: None,
                    copy_ignored: Some(true),
                    commands: vec![],
                    branch_replacements: vec![],
                }
            }
        );

        let empty_config = toml::from_str::<ConfigFile>("").unwrap();
        assert_eq!(
            default_config,
            ConfigFile {
                remote_names: empty_config.remote_names(),
                branch_names: empty_config.branch_names(),
                clone: CloneConfig {
                    enable_gh: Some(empty_config.clone.enable_gh()),
                },
                add: AddConfig {
                    copy_untracked: None,
                    copy_ignored: Some(empty_config.add.copy_ignored()),
                    commands: empty_config
                        .add
                        .commands()
                        .iter()
                        .map(|command| command.to_owned())
                        .collect(),
                    branch_replacements: empty_config
                        .add
                        .branch_replacements()
                        .iter()
                        .map(|replacement| replacement.to_owned())
                        .collect(),
                },
            }
        );
    }
}
