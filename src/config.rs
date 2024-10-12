use camino::Utf8PathBuf;
use clap::Parser;
use fs_err as fs;
use miette::Context;
use miette::IntoDiagnostic;
use serde::Deserialize;
use xdg::BaseDirectories;

use crate::cli::Cli;
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
                    &fs::read_to_string(&path)
                        .into_diagnostic()
                        .wrap_err("Failed to read configuration file")?,
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
            }
        );
    }
}
