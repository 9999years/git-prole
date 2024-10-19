use std::borrow::Cow;
use std::fmt::Display;

use camino::Utf8PathBuf;
use miette::miette;
use miette::IntoDiagnostic;
use owo_colors::OwoColorize;
use owo_colors::Stream;
use rustc_hash::FxHashSet as HashSet;
use tracing::instrument;

use crate::app_git::AppGit;
use crate::format_bulleted_list::format_bulleted_list;
use crate::format_bulleted_list_multiline;
use crate::fs;
use crate::git::BranchRef;
use crate::git::LocalBranchRef;
use crate::only_paths_in_parent_directory;
use crate::topological_sort::topological_sort;
use crate::utf8absolutize::Utf8Absolutize;
use crate::utf8tempdir::Utf8TempDir;
use crate::AddWorktreeOpts;
use crate::PathDisplay;
use crate::RenamedWorktree;
use crate::ResolveUniqueNameOpts;
use crate::Worktree;
use crate::WorktreeHead;
use crate::Worktrees;

#[derive(Debug)]
pub struct ConvertPlanOpts {
    pub default_branch: Option<String>,
    pub destination: Option<Utf8PathBuf>,
}

#[derive(Debug)]
pub struct ConvertPlan<'a> {
    /// A Git instance in the repository to convert.
    git: AppGit<'a>,
    /// A temporary directory where worktrees will be placed while the repository is rearranged.
    tempdir: Utf8PathBuf,
    /// The destination where the worktree container will be created.
    destination: Utf8PathBuf,
    /// The path of the repository to create.
    repo: Utf8PathBuf,
    /// The plan for converting the repo to a bare repo.
    ///
    /// If this is `Some`, the main worktree is not yet bare.
    make_bare: Option<MainWorktreePlan>,
    /// Plans for renaming the worktrees.
    ///
    /// These are ordered by a topological sort, to account for nested worktrees (if we move
    /// `/puppy` before `/puppy/doggy`, then `/puppy/doggy` will not be where we expect it after
    /// the first move).
    ///
    /// These contain unique names for each worktree, which are usually the name of the checked
    /// out branch.
    worktrees: Vec<WorktreePlan>,
    /// New worktrees to create.
    ///
    /// This contains the default branch, unless it's already checked out.
    new_worktrees: Vec<NewWorktreePlan>,
}

impl Display for ConvertPlan<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_no_op() {
            return write!(
                f,
                "{} is already a worktree repository",
                self.repo.display_path_cwd()
            );
        }

        write!(
            f,
            "Converting {} to a worktree repository{}.",
            self.repo.display_path_cwd(),
            if self.repo == self.destination {
                String::new()
            } else {
                format!(" at {}", self.destination.display_path_cwd())
            },
        )?;

        let moves = self
            .worktrees
            .iter()
            .filter(|worktree| worktree.worktree.path != worktree.destination(self))
            .map(|worktree| {
                format!(
                    "{} -> {}",
                    worktree.worktree.path.display_path_cwd(),
                    worktree.destination(self).display_path_cwd(),
                )
            })
            .collect::<Vec<_>>();

        if !moves.is_empty() {
            write!(
                f,
                "\nI'll move the following worktrees to new locations:\n\
                {}",
                format_bulleted_list_multiline(
                    self.worktrees
                        .iter()
                        .filter(|worktree| { worktree.worktree.path != worktree.destination(self) })
                        .map(|worktree| {
                            format!(
                                "{} -> {}",
                                worktree.worktree.path.display_path_cwd(),
                                worktree.destination(self).display_path_cwd(),
                            )
                        })
                )
            )?;
        }

        if !self.new_worktrees.is_empty() {
            write!(
                f,
                "\nI'll{} create new worktrees for the following branches:\n\
                {}",
                if moves.is_empty() { "" } else { " also" },
                format_bulleted_list_multiline(self.new_worktrees.iter().map(|worktree| {
                    format!(
                        "{} in {}",
                        worktree
                            .start_point
                            .qualified_branch_name()
                            .if_supports_color(Stream::Stdout, |text| text.cyan()),
                        worktree.destination(self).display_path_cwd(),
                    )
                }))
            )?;
        }

        if let Some(main_plan) = &self.make_bare {
            if main_plan.git_dir() != main_plan.git_destination(self) {
                write!(
                    f,
                    "\n{}I'll move the Git directory and convert the repository to a bare repository:\n\
                    {}",
                    if moves.is_empty() && self.new_worktrees.is_empty() {
                        ""
                    } else {
                        "Additionally, "
                    },
                    format_bulleted_list_multiline([
                        format!(
                            "{} -> {}",
                            main_plan.git_dir().display_path_cwd(),
                            main_plan.git_destination(self).display_path_cwd(),
                        )
                    ])
                )?;
            } else {
                write!(
                    f,
                    "\n{}I'll convert the repository to a bare repository.",
                    if moves.is_empty() && self.new_worktrees.is_empty() {
                        ""
                    } else {
                        "Additionally, "
                    },
                )?;
            }
        }

        Ok(())
    }
}

impl<'a> ConvertPlan<'a> {
    #[instrument(level = "trace")]
    pub fn new(git: AppGit<'a>, opts: ConvertPlanOpts) -> miette::Result<Self> {
        // Figuring out which worktrees to create is non-trivial:
        // - We might already have worktrees. (`convert_multiple_worktrees`)
        // - We might have any number of remotes.
        //   (`convert_multiple_remotes`)
        // - We might already have the default branch checked out.
        //   (`convert_default_branch_checked_out`)
        // - We might _not_ have the default branch checked out.
        //   (`convert_non_default_branch_checked_out`)
        // - We might have unstaged/uncommitted work.
        //   TODO: The `git reset` causes staged changes to be lost; bring back the
        //   `git status push`/`pop`?
        //   (`convert_uncommitted_changes`, `convert_unstaged_changes`)
        // - We might not be on _any_ branch.
        //   (`convert_detached_head`)
        // - There is no local branch for the default branch.
        //   (`config_default_branches`).
        //
        // Where do we want to place the resulting repo?
        // - If it's non-bare: in the default worktree's path
        // - If it's bare:
        //   - If the git dir is `.git`, then in its parent directory
        //   - If the git dir _ends with_ `.git`, then in the same directory, but with the `.git`
        //     suffix removed
        //   - Otherwise just use the git dir path.

        let tempdir = Utf8TempDir::new()?.into_path();
        let repo = git.path().repo_root_or_git_common_dir_if_bare()?;
        let repo = repo
            .parent()
            .ok_or_else(|| miette!("Repository path has no parent: {repo}"))?;
        let worktrees = git.worktree().list()?;

        let destination = Self::destination_plan(&worktrees, &opts)?;
        let destination_name = destination
            .file_name()
            .ok_or_else(|| miette!("Destination has no basename: {destination}"))?;
        tracing::debug!(%destination, "Destination determined");

        let default_branch = match opts.default_branch {
            // Tests:
            // - `convert_explicit_default_branch`
            // - `convert_explicit_default_branch_not_found`
            Some(default_branch) => git
                .refs()
                .rev_parse_symbolic_full_name(&default_branch)?
                .ok_or_else(|| miette!("`--default-branch` not found: {default_branch}"))?
                .try_into()?,
            None => git.preferred_branch()?,
        };
        tracing::debug!(%default_branch, "Default branch determined");

        // TODO: Check for branch with the default as an upstream as well?
        //
        // Tests:
        // - `convert_default_branch_checked_out`
        // - `convert_non_default_branch_checked_out`
        let has_worktree_for_default_branch =
            worktrees.for_branch(&default_branch.as_local()).is_some();
        let new_worktrees = if has_worktree_for_default_branch {
            Vec::new()
        } else {
            let name = git
                .worktree()
                .dirname_for(default_branch.branch_name())
                .to_owned();

            // If we're creating a worktree for a default branch from a
            // remote, we may not have a corresponding local branch
            // yet.
            let (create_branch, start_point) = match &default_branch {
                BranchRef::Local(_) => (None, default_branch),
                BranchRef::Remote(remote_branch) => {
                    if git.branch().exists_local(remote_branch.branch_name())? {
                        // Test: `convert_multiple_remotes`
                        (None, BranchRef::Local(remote_branch.as_local()))
                    } else {
                        // Test: `convert_no_local_default_branch`
                        tracing::warn!(
                            %remote_branch,
                            "Fetching the default branch"
                        );
                        git.remote().fetch(
                            remote_branch.remote(),
                            Some(&format!("{:#}:{remote_branch:#}", remote_branch.as_local())),
                        )?;
                        (Some(remote_branch.as_local()), default_branch)
                    }
                }
            };

            vec![NewWorktreePlan {
                name,
                create_branch,
                start_point,
            }]
        };

        // Tests:
        // - `convert_multiple_worktrees`
        //
        // Note: It's hard to write behavior tests for this because the tempdirs that tests run in
        // are randomly generated, so even though `rustc_hash` makes the `HashMap` iteration order
        // deterministic, the hashes of worktree paths aren't deterministic because they include
        // the tempdir paths. There's tests in `./git/worktree/resolve_unique_names.rs` though.
        let mut worktrees = git.worktree().resolve_unique_names(ResolveUniqueNameOpts {
            worktrees,
            names: new_worktrees
                .iter()
                .map(|plan| plan.name.to_owned())
                .collect(),
            directory_names: &HashSet::from_iter([destination_name]),
        })?;

        tracing::debug!(
            "Worktree names resolved:\n{}",
            format_bulleted_list(worktrees.iter().map(|(path, worktree)| {
                format!("{} → {}", path.display_path_cwd(), &worktree.name)
            }))
        );

        let mut make_bare = None;

        // Note: Worktrees may be nested in each other, so we have to move them in a
        // topologically-sorted order! E.g. if we have worktrees `/puppy` and
        // `/puppy/doggy`, if we move `/puppy` first then `/puppy/doggy` will no longer be
        // where we expect it!
        let worktree_plans = topological_sort(&worktrees.keys().collect::<Vec<_>>())?
            .into_iter()
            .map(|path| {
                let renamed = worktrees
                    .remove(&path)
                    .expect("Topological sort will not invent worktrees");

                let plan = WorktreePlan::from(renamed);

                // Test: `convert_default_branch_checked_out` (and many others)
                if plan.worktree.is_main && !plan.worktree.head.is_bare() {
                    make_bare = Some(MainWorktreePlan {
                        inner: plan.clone(),
                    });
                }

                plan
            })
            .collect::<Vec<_>>();

        let ret = Self {
            git,
            tempdir,
            destination,
            worktrees: worktree_plans,
            repo: repo.to_owned(),
            make_bare,
            new_worktrees,
        };

        tracing::debug!(
            "Worktree plans determined:\n{}",
            format_bulleted_list(ret.worktrees.iter().map(|plan| {
                format!(
                    "{} → {} →  {}",
                    plan.worktree.path.display_path_cwd(),
                    plan.temp_destination(&ret).display_path_cwd(),
                    plan.destination(&ret).display_path_cwd(),
                )
            }))
        );

        match &ret.make_bare {
            Some(make_bare) => {
                tracing::debug!(
                    git_dir=%make_bare.git_dir().display_path_cwd(),
                    temp_git_destination=%make_bare.temp_git_destination(&ret).display_path_cwd(),
                    git_destination=%make_bare.git_destination(&ret).display_path_cwd(),
                    worktree_temp_git_destination=%make_bare.worktree_temp_git_destination(&ret).display_path_cwd(),
                    worktree_git_destination=%make_bare.worktree_git_destination(&ret).display_path_cwd(),
                    worktree_plan=%format!(
                        "{} → {} →  {}",
                        make_bare.inner.worktree.path.display_path_cwd(),
                        make_bare.inner.temp_destination(&ret).display_path_cwd(),
                        make_bare.inner.destination(&ret).display_path_cwd(),
                    ),
                    "Plan for converting to a bare repository determined",
                );
            }
            None => {
                tracing::debug!("Repository is already bare");
            }
        }

        Ok(ret)
    }

    #[instrument(level = "trace")]
    fn destination_plan(
        worktrees: &Worktrees,
        opts: &ConvertPlanOpts,
    ) -> miette::Result<Utf8PathBuf> {
        if let Some(destination) = &opts.destination {
            // `convert_destination_explicit`
            return destination
                .clone()
                .absolutize()
                .map(Cow::into_owned)
                .into_diagnostic();
        }

        let main = worktrees.main();
        match main.head {
            WorktreeHead::Detached(_) | WorktreeHead::Branch(_, _) => {
                if worktrees.len() > 1 {
                    if let Some(common_parent) = only_paths_in_parent_directory(worktrees.keys()) {
                        // There's some common prefix all the worktrees belong to, let's put the
                        // new repo there.
                        //
                        // Tests:
                        // - `convert_common_prefix`
                        // - `convert_common_parent`
                        // - `convert_common_parent_extra_files`
                        // - `convert_common_parent_extra_dotfiles`
                        tracing::debug!(path = %common_parent, "Worktrees have a common parent");
                        return Ok(common_parent.to_owned());
                    }
                }
                // Tests:
                // - `convert_common_prefix`
                // - `convert_multiple_worktrees`
                // - `convert_detached_head`
                Ok(main.path.clone())
            }
            WorktreeHead::Bare => {
                let basename = main
                    .path
                    .file_name()
                    .ok_or_else(|| miette!("Git directory has no basename: {}", main.path))?;

                let parent = main
                    .path
                    .parent()
                    .ok_or_else(|| miette!("Git directory has no parent: {}", main.path))?;

                if basename == ".git" || basename.starts_with(".") {
                    // Tests:
                    // - `convert_bare_dot_git`
                    // - `convert_bare_starts_with_dot`
                    Ok(parent.to_owned())
                } else if let Some(stripped_basename) = basename.strip_suffix(".git") {
                    // `my-repo.git` -> `my-repo`
                    //
                    // Tests:
                    // - `convert_bare_ends_with_dot_git`
                    Ok(parent.join(stripped_basename))
                } else {
                    // Is this what you want? No clue!
                    //
                    // Tests:
                    // - `convert_bare_no_dot`
                    Ok(main.path.clone())
                }
            }
        }
    }

    #[instrument(level = "trace")]
    pub fn execute(&self) -> miette::Result<()> {
        tracing::info!("{self}");

        // Tests:
        // - `convert_no_op`
        if self.git.config.cli.dry_run || self.is_no_op() {
            return Ok(());
        }

        // TODO: Ask the user before we start messing around with their repo layout!

        // If the repository isn't already bare, separate the `.git` directory from its worktree
        // and make it bare.
        //
        // Test: (for all the `make_bare` behavior)
        // - `convert_default_branch_checked_out` (and many more)
        if let Some(make_bare) = &self.make_bare {
            fs::rename(make_bare.git_dir(), make_bare.temp_git_destination(self))?;
            self.git
                .with_directory(make_bare.temp_git_destination(self))
                .config()
                .set("core.bare", "true")?;
        }

        // Move worktrees to the tempdir.
        for plan in &self.worktrees {
            fs::rename(&plan.worktree.path, plan.temp_destination(self))?;
        }

        // Create the destination if it doesn't exist.
        if !self.destination.exists() {
            fs::create_dir_all(&self.destination)?;
        }

        // Move the `.git` directory to its new location.
        if let Some(make_bare) = &self.make_bare {
            fs::rename(
                make_bare.temp_git_destination(self),
                make_bare.git_destination(self),
            )?;

            // Make the main worktree into a real worktree, now that we've removed its `.git`
            // directory.
            self.git
                .with_directory(make_bare.git_destination(self))
                .worktree()
                .add(
                    &make_bare.inner.destination(self),
                    &AddWorktreeOpts {
                        checkout: false,
                        start_point: Some(&make_bare.inner.worktree.head.commitish()
                            .expect("If we're converting to a bare repository, the main worktree is never bare")
                            .to_string()),
                        ..Default::default()
                    },
                )?;

            self.git
                .with_directory(make_bare.inner.destination(self))
                .reset()?;
            fs::rename(
                make_bare.worktree_git_destination(self),
                make_bare.worktree_temp_git_destination(self),
            )?;
            fs::remove_dir(make_bare.inner.destination(self))?;
        }

        // Move worktrees back from the tempdir.
        for plan in &self.worktrees {
            fs::rename(plan.temp_destination(self), plan.destination(self))?;
        }

        // Repair worktrees with their new paths.
        let git = self.git.with_directory(self.destination.clone());
        git.worktree()
            .repair(self.worktrees.iter().map(|plan| plan.destination(self)))?;

        // Create new worktrees.
        for plan in &self.new_worktrees {
            git.worktree().add(
                &plan.destination(self),
                &AddWorktreeOpts {
                    track: plan.create_branch.is_some(),
                    create_branch: plan.create_branch.as_ref(),
                    start_point: Some(plan.start_point.qualified_branch_name()),
                    ..Default::default()
                },
            )?;
        }

        tracing::info!(
            "{} has been converted to a worktree checkout",
            self.destination.display_path_cwd()
        );
        tracing::info!("You may need to `cd .` to refresh your shell");

        Ok(())
    }

    pub fn is_no_op(&self) -> bool {
        self.make_bare.is_none()
            && self.new_worktrees.is_empty()
            && self
                .worktrees
                .iter()
                .all(|plan| plan.worktree.path == plan.destination(self))
    }
}

/// A plan for converting one worktree into a worktree repo.
///
/// **Note:** This is isomorphic to [`RenamedWorktree`].
#[derive(Debug, Clone)]
struct WorktreePlan {
    /// The name of the worktree; this is the last component of the destination path.
    name: String,
    /// The worktree itself.
    worktree: Worktree,
}

impl From<RenamedWorktree> for WorktreePlan {
    fn from(RenamedWorktree { name, worktree }: RenamedWorktree) -> Self {
        Self { name, worktree }
    }
}

impl WorktreePlan {
    /// Where we'll place the worktree in the temporary directory.
    fn temp_destination(&self, convert_plan: &ConvertPlan<'_>) -> Utf8PathBuf {
        convert_plan.tempdir.join(&self.name)
    }

    /// Where we'll place the worktree when we're done.
    fn destination(&self, convert_plan: &ConvertPlan<'_>) -> Utf8PathBuf {
        convert_plan.destination.join(&self.name)
    }
}

/// A plan for creating a new worktree for a worktree repo.
#[derive(Debug, Clone)]
struct NewWorktreePlan {
    /// The name of the worktree; this is the last component of the destination path.
    name: String,
    /// A local branch to create, if the `start_point` doesn't already exist.
    create_branch: Option<LocalBranchRef>,
    /// The branch the worktree will have checked out.
    start_point: BranchRef,
}

impl NewWorktreePlan {
    /// Where the new worktree will be created.
    fn destination(&self, convert_plan: &ConvertPlan<'_>) -> Utf8PathBuf {
        convert_plan.destination.join(&self.name)
    }
}

#[derive(Debug, Clone)]
struct MainWorktreePlan {
    /// The plan for the main worktree.
    inner: WorktreePlan,
}

impl MainWorktreePlan {
    /// The path of the `.git` directory before meddling.
    fn git_dir(&self) -> Utf8PathBuf {
        self.inner.worktree.path.join(".git")
    }

    /// Where we'll place the `.git` directory in the temporary directory.
    fn temp_git_destination(&self, convert_plan: &ConvertPlan<'_>) -> Utf8PathBuf {
        convert_plan.tempdir.join(".git")
    }

    /// Where we'll place the `.git` directory when we're done.
    fn git_destination(&self, convert_plan: &ConvertPlan<'_>) -> Utf8PathBuf {
        convert_plan.destination.join(".git")
    }

    /// Where we'll place the _worktree's_ `.git` symlink in the temporary directory.
    fn worktree_temp_git_destination(&self, convert_plan: &ConvertPlan<'_>) -> Utf8PathBuf {
        self.inner.temp_destination(convert_plan).join(".git")
    }

    /// Where we'll place the _worktree's_ `.git` symlink when we're done.
    fn worktree_git_destination(&self, convert_plan: &ConvertPlan<'_>) -> Utf8PathBuf {
        self.inner.destination(convert_plan).join(".git")
    }
}
