use config::Config;
use install_tracing::install_tracing;

mod add;
mod app;
mod cli;
mod clone;
mod config;
mod convert;
mod copy_dir;
mod current_dir;
mod format_bulleted_list;
mod gh;
mod git;
mod install_tracing;
mod normal_path;
mod topological_sort;
mod utf8tempdir;

fn main() -> miette::Result<()> {
    let config = Config::new()?;
    install_tracing(&config.cli.log)?;
    app::App::new(config).run()
}
