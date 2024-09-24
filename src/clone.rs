use std::process::Command;

use command_error::CommandExt;
use miette::miette;
use miette::IntoDiagnostic;
use which::which_global;

use crate::app_git::AppGit;
use crate::cli::CloneArgs;
use crate::convert::ConvertPlan;
use crate::convert::ConvertPlanOpts;
use crate::current_dir::current_dir_utf8;
use crate::gh::looks_like_gh_url;
use crate::git::repository_url_destination;

pub fn clone(git: AppGit<'_>, args: CloneArgs) -> miette::Result<()> {
    let destination = match args.directory {
        Some(directory) => directory.to_owned(),
        None => current_dir_utf8()?.join(repository_url_destination(&args.repository)),
    };

    if git.config.cli.dry_run {
        return Err(miette!("--dry-run is not supported for this command yet"));
    }

    if git.config.file.enable_gh()
        && looks_like_gh_url(&args.repository)
        && which_global("gh").is_ok()
    {
        // TODO: Test this!!!
        Command::new("gh")
            .args(["repo", "clone", &args.repository, destination.as_str()])
            .args(args.clone_args)
            .status_checked()
            .into_diagnostic()?;
    } else {
        // Test case: `clone_simple`.
        git.clone_repository(&args.repository, Some(&destination), &args.clone_args)?;
    }

    ConvertPlan::new(
        git.with_directory(destination),
        ConvertPlanOpts {
            default_branch: None,
        },
    )?
    .execute()?;

    Ok(())
}
