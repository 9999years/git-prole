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

impl Cli {
    /// A fake stub CLI for testing.
    #[cfg(test)]
    pub fn test_stub() -> Self {
        Self {
            log: "info".to_owned(),
            dry_run: false,
            config: None,
            command: Command::Convert(ConvertArgs {
                default_branch: None,
                destination: None,
            }),
        }
    }
}

#[allow(rustdoc::bare_urls)]
#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Convert a repository into a worktree checkout.
    ///
    /// This will convert the repository in the current directory into a worktree repository. This includes:
    ///
    /// - Making the repository a bare repository.
    ///
    /// - Converting the current checkout (branch, commit, whatever) into a worktree.
    ///   Uncommited changes will be kept, but will not remain unstaged.
    ///
    /// - Creating a new worktree for the default branch.
    Convert(ConvertArgs),

    /// Clone a repository into a worktree checkout.
    ///
    /// If you have `gh` installed and the URL looks `gh`-like and isn't an existing local path,
    /// I'll pass the repository URL to that.
    ///
    /// This is just a regular `git clone` followed by `git prole convert`.
    Clone(CloneArgs),

    /// Add a new worktree.
    ///
    /// This command tries to guess what you want, and as a result the behavior can be a little bit
    /// subtle! If given, the `--branch` argument will always create a new branch, and the
    /// `COMMITISH` argument will always be checked out in the new worktree; use those to
    /// disambiguate when necessary.
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
    /// A default branch to create a worktree for.
    #[arg(long)]
    pub default_branch: Option<String>,

    /// The directory to place the worktrees into.
    #[arg()]
    pub destination: Option<Utf8PathBuf>,
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
    ///
    /// If this is the name of a unique remote branch, then a local branch with the same name will
    /// be created to track the remote branch.
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
    #[arg(
        long,
        short = 'b',
        visible_alias = "create",
        visible_short_alias = 'c',
        conflicts_with_all = ["force_branch", "detach"],
    )]
    pub branch: Option<String>,

    /// Create a new branch with the given name, overwriting any existing branch with the same
    /// name.
    #[arg(
        long,
        short = 'B',
        visible_alias = "force-create",
        visible_short_alias = 'C',
        conflicts_with_all = ["branch", "detach"],
    )]
    pub force_branch: Option<String>,

    /// Create the new worktree in detached mode, not checked out on any branch.
    #[arg(
        long,
        short = 'd',
        alias = "detached",
        conflicts_with_all = ["branch", "force_branch"],
    )]
    pub detach: bool,

    /// The new worktree's name or path.
    ///
    /// If the name contains a `/`, it's assumed to be a path. Otherwise, it's assumed to be a
    /// worktree name: it's used as a name in the same directory as the other worktrees, and (by
    /// default) a branch with that name is checked out or created. (When this is a path, only the
    /// last component of the path is used as the branch name.)
    #[arg()]
    pub name_or_path: Option<String>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigCommand {
    /// Initialize a default configuration file.
    Init(ConfigInitArgs),
}

#[derive(Args, Clone, Debug)]
pub struct ConfigInitArgs {
    /// The location to write the configuration file. Can be `-` for stdout. Defaults to
    /// `~/.config/git-prole/config.toml`.
    pub output: Option<Utf8PathBuf>,
}
