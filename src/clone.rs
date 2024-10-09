use std::process::Command;

use command_error::CommandExt;
use miette::miette;
use miette::IntoDiagnostic;
use which::which_global;

use crate::app::App;
use crate::cli::CloneArgs;
use crate::convert::ConvertPlan;
use crate::convert::ConvertPlanOpts;
use crate::current_dir::current_dir_utf8;
use crate::gh::looks_like_gh_url;
use crate::git::repository_url_destination::repository_url_destination;

pub fn clone(app: &App, args: CloneArgs) -> miette::Result<()> {
    let destination = match args.directory {
        Some(directory) => directory.to_owned(),
        None => current_dir_utf8()?.join(repository_url_destination(&args.repository)),
    };

    if app.config.cli.dry_run {
        return Err(miette!("--dry-run is not supported for this command yet"));
    }

    if looks_like_gh_url(&args.repository) && which_global("gh").is_ok() {
        Command::new("gh")
            .args(["repo", "clone", &args.repository, destination.as_str()])
            .args(args.clone_args)
            .status_checked()
            .into_diagnostic()?;
    } else {
        app.git
            .clone_repository(&args.repository, Some(&destination), &args.clone_args)?;
    }

    ConvertPlan::new(
        app,
        ConvertPlanOpts {
            repository: destination,
            default_branch: None,
        },
    )?
    .execute(app)?;

    Ok(())
}
