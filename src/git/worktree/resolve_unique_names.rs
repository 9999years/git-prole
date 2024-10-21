use std::borrow::Cow;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use tracing::instrument;

use crate::git::GitLike;
use crate::AppGit;

#[cfg(doc)]
use super::GitWorktree;
use super::Worktree;
use super::Worktrees;

/// Options for [`GitWorktree::resolve_unique_names`].
#[derive(Debug)]
pub struct ResolveUniqueNameOpts<'a> {
    /// The worktrees to resolve into unique names.
    pub worktrees: Worktrees,
    /// A starting set of unique names that the resolved names will not conflict with.
    pub names: FxHashSet<String>,
    /// A set of directory names that the resolved names will not include.
    ///
    /// This is used to prevent worktree paths like `my-repo/my-repo` for detached `HEAD`
    /// worktrees.
    pub directory_names: &'a FxHashSet<&'a str>,
}

/// When we convert a repository into a worktree checkout, we put all the worktrees in one
/// directory.
///
/// This means that we have to make sure all their names are unique, and we want their names to
/// match their branches as much as possible.
///
/// We try the following names in order:
///
/// - For a bare worktree, `.git` is always used.
/// - The last component of the worktree's branch.
/// - The worktree's branch, with `/` replaced with `-`.
/// - The worktree's directory name.
/// - The worktree's directory name with numbers appended (e.g. for `puppy`, this tries `puppy-2`,
///   `puppy-3`, etc.)
/// - For a worktree with a detached `HEAD`, we try `work`, `work-2`, `work-3`, etc.
///
/// Anyways, this function resolves a bunch of worktrees into unique names.
#[instrument(level = "trace")]
pub fn resolve_unique_worktree_names<C>(
    git: &AppGit<'_, C>,
    mut opts: ResolveUniqueNameOpts<'_>,
) -> miette::Result<FxHashMap<Utf8PathBuf, RenamedWorktree>>
where
    C: AsRef<Utf8Path>,
{
    let (mut resolved, worktrees) = handle_bare_main_worktree(&mut opts.names, opts.worktrees);

    for (path, worktree) in worktrees.into_iter() {
        let name = WorktreeNames::new(git, &worktree, opts.directory_names)
            .names()?
            .find(|name| !opts.names.contains(name.as_ref()))
            .expect("There are an infinite number of possible resolved names for any worktree")
            .into_owned();

        opts.names.insert(name.clone());
        resolved.insert(path, RenamedWorktree { name, worktree });
    }

    Ok(resolved)
}

/// If the main worktree is bare, we want to rename it to `.git`.
///
/// Otherwise, we want to convert the main worktree to a bare worktree, so we don't want anything
/// else to be named `.git`.
///
/// This removes a bare main worktree from `worktrees` if it exists, naming it `.git`, and reserves
/// the `.git` name otherwise.
///
/// Returns the set of resolved names and the remaining worktrees.
fn handle_bare_main_worktree(
    names: &mut FxHashSet<String>,
    mut worktrees: Worktrees,
) -> (
    FxHashMap<Utf8PathBuf, RenamedWorktree>,
    FxHashMap<Utf8PathBuf, Worktree>,
) {
    let mut resolved = FxHashMap::default();
    debug_assert!(
        !names.contains(".git"),
        "`.git` cannot be a reserved worktree name"
    );
    names.insert(".git".into());

    let worktrees = if worktrees.main().head.is_bare() {
        let (path, worktree) = worktrees
            .inner
            .remove_entry(&worktrees.main)
            .expect("There is always a main worktree");

        resolved.insert(
            path,
            RenamedWorktree {
                name: ".git".into(),
                worktree,
            },
        );

        worktrees.inner
    } else {
        worktrees.into_inner()
    };

    (resolved, worktrees)
}

/// A worktree with a new name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamedWorktree {
    /// The name of the worktree; this will be the last component of the destination path when the
    /// worktree is moved.
    pub name: String,
    /// The worktree itself.
    pub worktree: Worktree,
}

struct WorktreeNames<'a, C> {
    git: &'a AppGit<'a, C>,
    worktree: &'a Worktree,
    directory_names: &'a FxHashSet<&'a str>,
}

impl<'a, C> WorktreeNames<'a, C>
where
    C: AsRef<Utf8Path>,
{
    fn new(
        git: &'a AppGit<'a, C>,
        worktree: &'a Worktree,
        directory_names: &'a FxHashSet<&'a str>,
    ) -> Self {
        Self {
            git,
            worktree,
            directory_names,
        }
    }

    fn names(&self) -> miette::Result<impl Iterator<Item = Cow<'a, str>>> {
        Ok(self
            .branch_last_component()
            .chain(self.branch_full())
            .chain(self.bare_git_dir().into_iter().flatten())
            .chain(self.directory_name())
            .chain(self.directory_name_numbers().into_iter().flatten())
            .chain(self.detached_work_numbers().into_iter().flatten()))
    }

    fn maybe_directory_name(&self) -> Option<&'a str> {
        self.worktree
            .path
            .file_name()
            .filter(|name| !self.directory_names.contains(*name))
    }

    fn directory_name(&self) -> impl Iterator<Item = Cow<'a, str>> {
        self.maybe_directory_name().map(Into::into).into_iter()
    }

    fn directory_name_numbers(&self) -> Option<impl Iterator<Item = Cow<'a, str>>> {
        self.maybe_directory_name().map(|directory_name| {
            (2..).map(move |number| format!("{directory_name}-{number}").into())
        })
    }

    fn bare_git_dir(&self) -> Option<impl Iterator<Item = Cow<'a, str>>> {
        if self.worktree.head.is_bare() {
            Some(std::iter::once(".git".into()))
        } else {
            None
        }
    }

    fn detached_work_numbers(&self) -> Option<impl Iterator<Item = Cow<'a, str>>> {
        if self.worktree.head.is_detached() {
            Some(
                std::iter::once("work".into())
                    .chain((2..).map(|number| format!("work-{number}").into())),
            )
        } else {
            None
        }
    }

    fn branch_last_component(&self) -> impl Iterator<Item = Cow<'a, str>> {
        self.worktree
            .head
            .branch()
            .map(|branch| self.git.worktree().dirname_for(branch.branch_name()))
            .into_iter()
    }

    fn branch_full(&self) -> impl Iterator<Item = Cow<'a, str>> {
        self.worktree
            .head
            .branch()
            .map(|branch| branch.branch_name().replace('/', "-").into())
            .into_iter()
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use expect_test::Expect;
    use itertools::Itertools;

    use crate::CommitHash;
    use crate::Config;
    use crate::Git;

    use super::*;

    struct Opts<const WS: usize, N = Option<String>, D = Option<String>> {
        worktrees: [Worktree; WS],
        names: N,
        directory_names: D,
        expect: Expect,
    }

    impl<const WS: usize, N, D> Opts<WS, N, D>
    where
        N: IntoIterator<Item = &'static str>,
        D: IntoIterator<Item = &'static str>,
    {
        #[track_caller]
        fn assert(mut self) {
            let config = Config::test_stub();
            let git = Git::from_current_dir().unwrap().with_config(&config);

            self.worktrees[0].is_main = true;

            let worktrees = Worktrees {
                main: self.worktrees[0].path.clone(),
                inner: self
                    .worktrees
                    .into_iter()
                    .map(|worktree| (worktree.path.clone(), worktree))
                    .collect::<FxHashMap<_, _>>(),
            };

            let mut worktrees = resolve_unique_worktree_names(
                &git,
                ResolveUniqueNameOpts {
                    worktrees,
                    names: self.names.into_iter().map(|name| name.to_owned()).collect(),
                    directory_names: &self.directory_names.into_iter().collect(),
                },
            )
            .unwrap()
            .into_iter()
            .map(|(path, renamed)| (path, renamed.name))
            .collect::<Vec<_>>();

            worktrees.sort_by_key(|(path, _name)| path.clone());

            let mut worktrees_formatted = worktrees
                .iter()
                .map(|(path, name)| format!("{path} -> {name}"))
                .join("\n");

            if worktrees.len() > 1 {
                worktrees_formatted.push('\n');
            }

            self.expect.assert_eq(&worktrees_formatted);
        }
    }

    #[test]
    fn test_resolve_unique_names_branch_last_component() {
        Opts {
            worktrees: [Worktree::new_branch(
                "/softy",
                CommitHash::fake(),
                "doggy/puppy",
            )],
            expect: expect!["/softy -> puppy"],
            names: None,
            directory_names: None,
        }
        .assert();
    }

    #[test]
    fn test_resolve_unique_names_branch_full() {
        Opts {
            worktrees: [Worktree::new_branch(
                "/softy",
                CommitHash::fake(),
                "doggy/puppy",
            )],
            expect: expect!["/softy -> doggy-puppy"],
            names: ["puppy"],
            directory_names: None,
        }
        .assert();
    }

    #[test]
    fn test_resolve_unique_names_bare_git_dir() {
        Opts {
            worktrees: [Worktree::new_bare("/puppy")],
            expect: expect!["/puppy -> .git"],
            names: None,
            directory_names: None,
        }
        .assert();
    }

    #[test]
    fn test_resolve_unique_names_directory_name() {
        Opts {
            worktrees: [Worktree::new_detached("/puppy", CommitHash::fake())],
            expect: expect!["/puppy -> puppy"],
            names: None,
            directory_names: None,
        }
        .assert();
    }

    #[test]
    fn test_resolve_unique_names_directory_name_numbers() {
        Opts {
            worktrees: [Worktree::new_detached("/puppy", CommitHash::fake())],
            expect: expect!["/puppy -> puppy-2"],
            names: ["puppy"],
            directory_names: None,
        }
        .assert();
    }

    #[test]
    fn test_resolve_unique_names_directory_name_skips_directory_names() {
        Opts {
            worktrees: [Worktree::new_detached("/puppy", CommitHash::fake())],
            expect: expect!["/puppy -> work"],
            names: None,
            directory_names: ["puppy"],
        }
        .assert();
    }

    #[test]
    fn test_resolve_unique_names_detached_work_numbers() {
        Opts {
            worktrees: [Worktree::new_detached("/puppy", CommitHash::fake())],
            expect: expect!["/puppy -> work-2"],
            names: ["work"],
            directory_names: ["puppy"],
        }
        .assert();
    }

    #[test]
    fn test_resolve_unique_names_many() {
        Opts {
            worktrees: [
                Worktree::new_bare("/puppy.git"),
                Worktree::new_detached("/puppy", CommitHash::fake()),
                Worktree::new_detached("/silly/puppy", CommitHash::fake()),
                Worktree::new_detached("/my-repo", CommitHash::fake()),
                Worktree::new_detached("/silly/my-repo", CommitHash::fake()),
                Worktree::new_branch("/a", CommitHash::fake(), "puppy/doggy"),
                Worktree::new_branch("/b", CommitHash::fake(), "puppy/doggy"),
                Worktree::new_branch("/c", CommitHash::fake(), "puppy/doggy"),
                Worktree::new_branch("/d/c", CommitHash::fake(), "puppy/doggy"),
                Worktree::new_branch("/e/c", CommitHash::fake(), "puppy/doggy"),
                Worktree::new_branch("/f/c", CommitHash::fake(), "puppy/doggy"),
            ],
            expect: expect![[r#"
                /a -> puppy-doggy
                /b -> b
                /c -> c
                /d/c -> c-3
                /e/c -> doggy
                /f/c -> c-2
                /my-repo -> work
                /puppy -> puppy
                /puppy.git -> .git
                /silly/my-repo -> work-2
                /silly/puppy -> puppy-2
            "#]],
            names: ["main"],
            directory_names: ["my-repo"],
        }
        .assert();
    }
}
