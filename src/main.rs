mod cli;
mod commit_hash;
mod git;
mod install_tracing;

use clap::CommandFactory;
use clap::Parser;
use cli::Opts;
use install_tracing::install_tracing;

#[allow(unused_imports)]
use miette::Context;
#[allow(unused_imports)]
use miette::IntoDiagnostic;

fn main() -> miette::Result<()> {
    let opts = Opts::parse();
    install_tracing(&opts.log)?;

    match opts.command {
        cli::Command::Completions { shell } => {
            let mut clap_command = cli::Opts::command();
            clap_complete::generate(
                shell,
                &mut clap_command,
                "git-prole",
                &mut std::io::stdout(),
            );
        }
        #[cfg(feature = "clap_mangen")]
        cli::Command::Manpages { out_dir } => {
            let clap_command = cli::Opts::command();
            clap_mangen::generate_to(clap_command, out_dir)
                .into_diagnostic()
                .wrap_err("Failed to generate man pages")?;
        }
        cli::Command::Add {} => todo!(),
    }

    Ok(())
}
