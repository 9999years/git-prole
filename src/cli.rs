use camino::Utf8PathBuf;
use clap::Args;
use clap::Parser;
use clap::Subcommand;

/// A `git-worktree(1)` manager.
#[derive(Debug, Clone, Parser)]
#[command(version, author, about)]
#[command(max_term_width = 100, disable_help_subcommand = true)]
pub struct Cli {
    /// Log filter directives, of the form `target[span{field=value}]=level`, where all components
    /// except the level are optional.
    ///
    /// Try `debug` or `trace`.
    #[arg(long, default_value = "info", env = "GIT_PROLE_LOG", global = true)]
    pub log: String,

    /// If set, do not perform any actions, and instead only construct and print a plan.
    #[arg(long, visible_alias = "dry", default_value = "false", global = true)]
    pub dry_run: bool,

    /// The location to read the configuration file from. Defaults to
    /// `~/.config/git-prole/config.toml`.
    #[arg(long, global = true)]
    pub config: Option<Utf8PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[allow(rustdoc::bare_urls)]
#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Convert a checkout into a worktree checkout.
    Convert(ConvertArgs),

    /// Clone a repository into a worktree checkout.
    ///
    /// If you have `gh` installed and the URL looks `gh`-like and isn't an existing local path,
    /// I'll pass the repository URL to that.
    Clone(CloneArgs),

    /// Add a new worktree.
    ///
    /// Unlike `git worktree add`, this will set new worktrees to start at and track the default
    /// branch by default, rather than the checked out commit or branch of the worktree the command
    /// is run from.
    ///
    /// By default, untracked files are copied to the new worktree.
    Add(AddArgs),

    /// Initialize the configuration file.
    #[command(subcommand)]
    Config(ConfigCommand),

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

#[derive(Args, Clone, Debug)]
pub struct ConvertArgs {
    /// A default branch to use as the main checkout.
    ///
    /// The `.git` directory will live in this worktree.
    #[arg(long)]
    pub default_branch: Option<String>,
}

#[derive(Args, Clone, Debug)]
pub struct CloneArgs {
    /// The repository URL to clone.
    #[arg()]
    pub repository: String,

    /// The directory to setup the worktrees in.
    ///
    /// Defaults to the last component of the repository URL, with a trailing `.git` removed.
    #[arg()]
    pub directory: Option<Utf8PathBuf>,

    /// Extra arguments to forward to `git clone`.
    #[arg(last = true)]
    pub clone_args: Vec<String>,
}

#[derive(Args, Clone, Debug)]
pub struct AddArgs {
    #[command(flatten)]
    pub inner: AddArgsInner,

    /// The commit to check out in the new worktree.
    #[arg()]
    pub commitish: Option<String>,

    /// Extra arguments to forward to `git worktree add`.
    #[arg(last = true)]
    pub worktree_add_args: Vec<String>,
}

#[derive(Args, Clone, Debug)]
#[group(required = true, multiple = true)]
pub struct AddArgsInner {
    /// Create a new branch with the given name instead of checking out an existing branch.
    ///
    /// This will refuse to reset a branch if it already exists; use `--force-branch`/`-B` to
    /// reset existing branches.
    #[arg(long, short = 'b', visible_alias = "create", visible_short_alias = 'c')]
    pub branch: Option<String>,

    /// Create a new branch with the given name, overwriting any existing branch with the same
    /// name.
    #[arg(
        long,
        short = 'B',
        visible_alias = "force-create",
        visible_short_alias = 'C'
    )]
    pub force_branch: Option<String>,

    /// The new worktree's name or path.
    ///
    /// If this doesn't contain a `/`, it's assumed to be a directory name, and the worktree
    /// will be placed adjacent to the other worktrees.
    #[arg()]
    pub name_or_path: Option<String>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigCommand {
    /// Initialize a default configuration file.
    Generate(ConfigGenerateArgs),
}

#[derive(Args, Clone, Debug)]
pub struct ConfigGenerateArgs {
    /// The location to write the configuration file. Can be `-` for stdout. Defaults to
    /// `~/.config/git-prole/config.toml`.
    pub output: Option<Utf8PathBuf>,
}
