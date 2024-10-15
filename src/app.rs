use calm_io::stdout;
use clap::CommandFactory;
use miette::miette;
use miette::IntoDiagnostic;

use crate::add::WorktreePlan;
use crate::app_git::AppGit;
use crate::cli;
use crate::cli::ConfigCommand;
use crate::cli::ConfigGenerateArgs;
use crate::config::Config;
use crate::convert::ConvertPlan;
use crate::convert::ConvertPlanOpts;
use crate::fs;
use crate::git::Git;

pub struct App {
    config: Config,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn git(&self) -> miette::Result<AppGit<'_>> {
        Ok(Git::from_current_dir()?.with_config(&self.config))
    }

    pub fn run(self) -> miette::Result<()> {
        match &self.config.cli.command {
            cli::Command::Completions { shell } => {
                let mut clap_command = cli::Cli::command();
                clap_complete::generate(
                    *shell,
                    &mut clap_command,
                    "git-prole",
                    &mut std::io::stdout(),
                );
            }
            #[cfg(feature = "clap_mangen")]
            cli::Command::Manpages { out_dir } => {
                use miette::Context;
                let clap_command = cli::Cli::command();
                clap_mangen::generate_to(clap_command, out_dir)
                    .into_diagnostic()
                    .wrap_err("Failed to generate man pages")?;
            }
            cli::Command::Convert(args) => ConvertPlan::new(
                self.git()?,
                ConvertPlanOpts {
                    default_branch: args.default_branch.clone(),
                    destination: args.destination.clone(),
                },
            )?
            .execute()?,
            cli::Command::Clone(args) => crate::clone::clone(self.git()?, args.to_owned())?,
            cli::Command::Add(args) => WorktreePlan::new(self.git()?, args)?.execute()?,
            cli::Command::Config(ConfigCommand::Generate(args)) => {
                self.config_generate(args.to_owned())?
            }
        }

        Ok(())
    }

    fn config_generate(&self, args: ConfigGenerateArgs) -> miette::Result<()> {
        let path = match &args.output {
            Some(path) => {
                if path == "-" {
                    stdout!("{}", Config::DEFAULT).into_diagnostic()?;
                    return Ok(());
                } else {
                    path
                }
            }
            None => &self.config.path,
        };

        if path.exists() {
            return Err(miette!("Default configuration file already exists: {path}"));
        }

        tracing::info!(
            %path,
            "Writing default configuration file"
        );

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, Config::DEFAULT)?;

        Ok(())
    }
}
