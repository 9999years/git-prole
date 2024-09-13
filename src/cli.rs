use clap::Parser;
use clap::Subcommand;

/// A `git-worktree(1)` manager.
#[derive(Debug, Clone, Parser)]
#[command(version, author, about)]
#[command(max_term_width = 100, disable_help_subcommand = true)]
pub struct Opts {
    /// Log filter directives, of the form `target[span{field=value}]=level`, where all components
    /// except the level are optional.
    ///
    /// Try `debug` or `trace`.
    #[arg(long, default_value = "info", env = "GIT_PROLE_LOG")]
    pub log: String,

    #[command(subcommand)]
    pub command: Command,
}

#[allow(rustdoc::bare_urls)]
#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    Add {},
    /// Generate shell completions.
    Completions {
        /// Shell to generate completions for.
        shell: clap_complete::shells::Shell,
    },
    /// Generate man pages.
    #[cfg(feature = "clap_mangen")]
    Manpages {
        /// Directory to write man pages to.
        out_dir: camino::Utf8PathBuf,
    },
}
