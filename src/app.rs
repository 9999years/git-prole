use std::collections::HashSet;
use std::process::Command;

use calm_io::stdout;
use camino::Utf8PathBuf;
use clap::CommandFactory;
use command_error::CommandExt;
use fs_err as fs;
use miette::miette;
use miette::IntoDiagnostic;
use tap::Tap;
use which::which_global;

use crate::cli;
use crate::cli::AddArgs;
use crate::cli::CloneArgs;
use crate::cli::ConfigCommand;
use crate::cli::ConfigGenerateArgs;
use crate::cli::ConvertArgs;
use crate::config::Config;
use crate::convert::ConvertPlan;
use crate::convert::ConvertPlanOpts;
use crate::current_dir::current_dir_utf8;
use crate::gh::looks_like_gh_url;
use crate::git::repository_url_destination::repository_url_destination;
use crate::git::Git;

pub struct App {
    pub git: Git,
    pub config: Config,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            git: Git::new(),
        }
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
            cli::Command::Convert(args) => self.convert(args.to_owned())?,
            cli::Command::Clone(args) => self.clone(args.to_owned())?,
            cli::Command::Add(args) => self.add(args.to_owned())?,
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
            fs::create_dir_all(parent).into_diagnostic()?;
        }

        fs::write(path, Config::DEFAULT).into_diagnostic()?;

        Ok(())
    }

    fn add(&self, args: AddArgs) -> miette::Result<()> {
        crate::add::add(self, args)
    }

    fn clone(&self, args: CloneArgs) -> miette::Result<()> {
        let destination = match args.directory {
            Some(directory) => directory.to_owned(),
            None => current_dir_utf8()?.join(repository_url_destination(&args.repository)),
        };

        if self.config.cli.dry_run {
            return Err(miette!("--dry-run is not supported for this command yet"));
        }

        if looks_like_gh_url(&args.repository) && which_global("gh").is_ok() {
            Command::new("gh")
                .args([&args.repository, destination.as_str()])
                .args(args.clone_args)
                .status_checked()
                .into_diagnostic()?;
        } else {
            self.git
                .clone_repository(&args.repository, Some(&destination), &args.clone_args)?;
        }

        self.convert(ConvertArgs {
            default_branch: None,
        })?;
        Ok(())
    }

    fn convert(&self, args: ConvertArgs) -> miette::Result<()> {
        let plan = ConvertPlan::new(
            self,
            ConvertPlanOpts {
                repository: current_dir_utf8()?,
                default_branch: args.default_branch,
            },
        )?;

        tracing::info!("{plan}");

        // TODO: Ask the user before we start messing around with their repo layout!
        if !self.config.cli.dry_run {
            plan.execute()?;
        }

        Ok(())
    }

    pub fn pick_default_branch(&self) -> miette::Result<String> {
        if let Some(default_remote) = self.pick_default_remote()? {
            return self.git.default_branch(&default_remote);
        }

        let preferred_branches = self.config.file.default_branches();
        let all_branches = self.git.list_local_branches()?;
        for branch in preferred_branches {
            if all_branches.contains(&branch) {
                return Ok(branch);
            }
        }

        Err(miette!(
            "No default branch found; specify a `--default-branch` to check out"
        ))
    }

    pub fn pick_default_remote(&self) -> miette::Result<Option<String>> {
        Ok(self.sorted_remotes()?.first().cloned())
    }

    pub fn sorted_remotes(&self) -> miette::Result<Vec<String>> {
        let mut all_remotes = self.git.remotes()?.into_iter().collect::<HashSet<_>>();

        let mut sorted = Vec::with_capacity(all_remotes.len());

        if let Some(default_remote) = self.git.default_remote()? {
            if let Some(remote) = all_remotes.take(&default_remote) {
                sorted.push(remote);
            }
        }

        let preferred_remotes = self.config.file.remotes();
        for remote in preferred_remotes {
            if let Some(remote) = all_remotes.take(&remote) {
                sorted.push(remote);
            }
        }

        Ok(sorted)
    }

    /// The directory name, nested under the worktree parent directory, where the given
    /// branch's worktree will be placed.
    ///
    /// E.g. to convert a repo `~/puppy` with default branch `main`, this will return `main`,
    /// to indicate a worktree to be placed in `~/puppy/main`.
    ///
    /// TODO: Should support some configurable regex filtering or other logic?
    pub fn branch_dirname(branch: &str) -> &str {
        match branch.rsplit_once('/') {
            Some((_left, right)) => {
                tracing::warn!(
                    %branch,
                    worktree = %right,
                    "Branch contains a `/`, using trailing component for worktree directory name"
                );
                right
            }
            None => branch,
        }
    }

    /// Get the full path for a new worktree with the given branch name.
    ///
    /// This appends the [`Self::branch_dirname`] to the [`Git::worktree_container`].
    pub fn branch_path(&self, branch: &str) -> miette::Result<Utf8PathBuf> {
        Ok(self
            .git
            .worktree_container()?
            .tap_mut(|p| p.push(Self::branch_dirname(branch))))
    }
}
