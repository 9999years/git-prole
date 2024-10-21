use std::fmt::Debug;
use std::fmt::Display;
use std::str::FromStr;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use expect_test::Expect;
use git_prole::format_bulleted_list_multiline;
use git_prole::fs;
use git_prole::BranchRef;
use git_prole::Git;
use git_prole::GitLike;
use git_prole::LocalBranchRef;
use git_prole::Ref;
use git_prole::RemoteBranchRef;
use git_prole::Status;
use git_prole::StatusEntry;
use git_prole::Worktree;
use git_prole::WorktreeHead;
use itertools::Itertools;
use pretty_assertions::assert_eq;
use pretty_assertions::Comparison;
use rustc_hash::FxHashMap;

/// A repository state, which can be checked against a real repository.
#[derive(Debug)]
pub struct RepoState {
    git: Git<Utf8PathBuf>,
    git_dir: Option<Utf8PathBuf>,
    worktrees: Option<Vec<WorktreeState>>,
}

impl RepoState {
    /// Construct a new repository state with the given [`Git`] object.
    ///
    /// The [`Git`]'s directory is used as the repository root.
    pub fn new(git: Git<Utf8PathBuf>) -> Self {
        Self {
            git,
            git_dir: Default::default(),
            worktrees: Default::default(),
        }
    }

    /// Get the root of the repository.
    fn root(&self) -> &Utf8Path {
        self.git.get_current_dir().as_ref()
    }

    /// Expect the repository to have its `.git` directory at the given path, relative to the
    /// repository root.
    pub fn git_dir(mut self, path: &str) -> Self {
        self.git_dir = Some(self.root().join(path));
        self
    }

    /// Expect the repository to have the given worktrees.
    pub fn worktrees(mut self, worktrees: impl IntoIterator<Item = WorktreeState>) -> Self {
        self.worktrees = Some(worktrees.into_iter().collect());
        self
    }

    /// Assert that the repository state matches the actual repository.
    ///
    /// # Panics
    ///
    /// If the repository state doesn't match the actual repository.
    #[track_caller]
    pub fn assert(&self) {
        if let Some(git_dir) = &self.git_dir {
            assert_eq!(
                self.git.path().git_common_dir().unwrap(),
                // NOTE: Seems I have to canonicalize paths to avoid finicky problems with `/tmp`
                // vs. `/private/tmp` on macOS (the former being a symlink to the latter).
                git_dir.canonicalize_utf8().unwrap()
            );
        }

        let mut problems = Vec::new();

        if let Some(worktrees) = &self.worktrees {
            let actual_worktrees = self.git.worktree().list().unwrap();
            let mut expected_worktrees = worktrees
                .iter()
                .map(|worktree| {
                    let path = self.root().join(&worktree.path);

                    if !path.exists() {
                        panic!(
                            "Worktree {} doesn't exist. Worktrees:\n{}",
                            worktree.path, actual_worktrees
                        );
                    }

                    let path = path
                        .canonicalize_utf8()
                        .map_err(|err| format!("{err}: {}", worktree.path))
                        .expect("Worktree path should be canonicalize-able");

                    (path, worktree)
                })
                .collect::<FxHashMap<_, _>>();

            for (_, actual) in actual_worktrees.iter() {
                let expected = match expected_worktrees.remove(&actual.path) {
                    Some(expected) => expected,
                    None => {
                        problems.push(format!("Found unexpected worktree: {actual}"));
                        continue;
                    }
                };
                let worktree_problems = WorktreeState::check(&self.git, expected, actual);
                if !worktree_problems.is_empty() {
                    problems.push(format!(
                        "Worktree {}:\n{}",
                        expected.path,
                        format_bulleted_list_multiline(worktree_problems)
                    ));
                }
            }

            if !expected_worktrees.is_empty() {
                problems.push(format!(
                    "Worktrees not found:\n{}",
                    format_bulleted_list_multiline(
                        expected_worktrees
                            .values()
                            .map(|worktree| worktree.path.clone())
                    )
                ));
            }
        }

        if !problems.is_empty() {
            panic!("{}", format_bulleted_list_multiline(problems));
        }
    }
}

/// A Git worktree's state.
///
/// Used with [`RepoState`] to validate a worktree state against an actual worktree.
#[derive(Debug)]
pub struct WorktreeState {
    path: String,
    is_main: Option<bool>,
    head: Option<WorktreeHeadState>,
    files: Option<Vec<(String, Option<Expect>)>>,
    upstream: Option<Option<BranchRef>>,
    status: Option<Status>,
}

impl WorktreeState {
    /// Expect a worktree in the given path, relative to the repository root.
    pub fn new(path: &str) -> Self {
        Self {
            path: path.into(),
            is_main: Default::default(),
            head: Default::default(),
            files: Default::default(),
            upstream: Default::default(),
            status: Default::default(),
        }
    }

    /// A new bare worktree at the root of the containing repo.
    ///
    /// This is used if a bare worktree named `.git` is present at the repository root.
    pub fn new_bare() -> Self {
        Self::new(".git").bare()
    }

    /// This worktree is bare.
    pub fn bare(mut self) -> Self {
        self.is_main = Some(true);
        self.head = Some(WorktreeHeadState::Bare);
        self
    }

    /// This worktree is or is not a main worktree.
    pub fn is_main(mut self, is_main: bool) -> Self {
        self.is_main = Some(is_main);
        self
    }

    /// This worktree is detached at the given commit.
    ///
    /// The given commit must be a prefix of the actual commit; this lets you use either a full
    /// 40-character commit hash or an abbreviated hash.
    pub fn detached(mut self, commit: &str) -> Self {
        self.head = Some(WorktreeHeadState::Detached(commit.into()));
        self
    }

    /// This worktree is on the given branch.
    pub fn branch(mut self, branch: &str) -> Self {
        self.head = Some(WorktreeHeadState::Branch(
            None,
            LocalBranchRef::from(branch),
        ));
        self
    }

    /// This worktree is on the given commit.
    ///
    /// # Panics
    ///
    /// If [`WorktreeState::branch`] hasn't been called; use [`WorktreeState::detached`] to specify
    /// a detached `HEAD` commit.
    #[track_caller]
    pub fn commit(mut self, commit: &str) -> Self {
        self.head = match self.head {
            Some(WorktreeHeadState::Branch(_, branch)) => {
                Some(WorktreeHeadState::Branch(Some(commit.into()), branch))
            }
            _ => {
                panic!(".commit() can only be used on branch worktrees; use .detached() for detached worktrees")
            }
        };
        self
    }

    /// Expect the worktree's branch to have the given branch as its upstream.
    ///
    /// If the given branch contains a `/`, it's assumed to be a remote-tracking branch like
    /// `origin/main`.
    ///
    /// # Panics
    ///
    /// If [`WorktreeState::branch`] hasn't been called.
    pub fn upstream(mut self, branch: &str) -> Self {
        if !matches!(&self.head, Some(WorktreeHeadState::Branch(_, _))) {
            panic!(
                ".upstream() can only be used on branch worktrees; specify a branch with .branch()"
            );
        }

        self.upstream = Some(Some(if branch.contains('/') {
            RemoteBranchRef::try_from(Ref::new(Ref::REMOTES.into(), branch.into()))
                .unwrap()
                .into()
        } else {
            LocalBranchRef::try_from(Ref::new(Ref::HEADS.into(), branch.into()))
                .unwrap()
                .into()
        }));
        self
    }

    /// Expect the worktree's branch to have no upstream.
    ///
    /// # Panics
    ///
    /// If [`WorktreeState::branch`] hasn't been called.
    pub fn no_upstream(mut self) -> Self {
        if !matches!(&self.head, Some(WorktreeHeadState::Branch(_, _))) {
            panic!(".no_upstream() can only be used on branch worktrees; specify a branch with .branch()");
        }

        self.upstream = Some(None);
        self
    }

    /// Expect a file at the given path to have the given contents.
    pub fn file(mut self, path: &str, contents: Expect) -> Self {
        self.files = match self.files {
            Some(mut files) => {
                files.push((path.into(), Some(contents)));
                Some(files)
            }
            None => Some(vec![(path.into(), Some(contents))]),
        };
        self
    }

    /// Expect a file at the given path to _not_ exist.
    pub fn no_file(mut self, path: &str) -> Self {
        self.files = match self.files {
            Some(mut files) => {
                files.push((path.into(), None));
                Some(files)
            }
            None => Some(vec![(path.into(), None)]),
        };
        self
    }

    /// Expect the worktree's `git status` to have the given entries.
    #[track_caller]
    pub fn status<'a>(mut self, entries: impl IntoIterator<Item = &'a str>) -> Self {
        self.status = Some(Status {
            entries: entries
                .into_iter()
                // lol, lmao
                .map(|entry| StatusEntry::from_str(&format!("{entry}\0")))
                .collect::<Result<Vec<_>, _>>()
                .expect("All expected status entries parse succesfully"),
        });
        self
    }

    #[track_caller]
    fn check<C>(git: &Git<C>, expected: &Self, actual: &Worktree) -> Vec<String>
    where
        C: AsRef<Utf8Path>,
    {
        let mut problems = Vec::new();
        let git = git.with_current_dir(actual.path.as_path());

        Self::check_is_main(&mut problems, expected, actual);
        Self::check_head(&mut problems, expected, actual);
        Self::check_files(&mut problems, expected, actual);
        Self::check_status(&mut problems, &git, expected, actual);

        problems
    }

    #[track_caller]
    fn check_is_main(problems: &mut Vec<String>, expected: &Self, actual: &Worktree) {
        let expected_is_main = match expected.is_main {
            Some(expected_is_main) => expected_is_main,
            None => {
                return;
            }
        };

        if expected_is_main != actual.is_main {
            if expected_is_main {
                problems.push("Worktree is not main worktree, expected main worktree".into());
            } else {
                problems.push("Worktree is main worktree, expected non-main worktree".into());
            }
        }
    }

    #[track_caller]
    fn check_head(problems: &mut Vec<String>, expected: &Self, actual: &Worktree) {
        let expected_head = match &expected.head {
            Some(head) => head,
            None => {
                return;
            }
        };

        let actual_head = &actual.head;

        match expected_head {
            WorktreeHeadState::Bare => {
                if !actual.head.is_bare() {
                    problems.push(format!("Expected bare worktree: {actual_head}"));
                }
            }

            WorktreeHeadState::Detached(commit) => match &actual.head {
                WorktreeHead::Detached(actual_commit) => {
                    if !actual_commit.starts_with(commit) {
                        problems.push(format!("Expected detached HEAD at {commit}: {actual_head}"));
                    }
                }
                _ => {
                    problems.push(format!("Expected detached HEAD at {commit}: {actual_head}"));
                }
            },

            WorktreeHeadState::Branch(commit, branch) => match &actual.head {
                WorktreeHead::Branch(actual_commit, actual_branch) => {
                    if branch != actual_branch {
                        problems.push(format!(
                            "Expected branch {branch}, found {actual_branch}: {actual_head}"
                        ));
                    }

                    if let Some(commit) = commit {
                        if !actual_commit.starts_with(commit) {
                            problems.push(format!("Expected branch {branch} at {commit}, found {actual_commit}: {actual_head}"));
                        }
                    }
                }
                _ => {
                    problems.push(format!("Expected branch {branch}: {actual_head}"));
                }
            },
        }
    }

    #[track_caller]
    fn check_files(problems: &mut Vec<String>, expected: &Self, actual: &Worktree) {
        let expected_path = &expected.path;

        let expected_files = match &expected.files {
            Some(files) => files,
            None => {
                return;
            }
        };

        for (path, contents) in expected_files {
            let actual_path = actual.path.join(path);

            match contents {
                None => {
                    if actual_path.exists() {
                        problems.push(format!(
                            "Path exists in {expected_path}, but should not: {path}"
                        ));
                    }
                }
                Some(contents) => {
                    if !actual_path.exists() {
                        problems.push(format!(
                            "Expected path does not exist in {expected_path}, but should: {path}"
                        ));
                        continue;
                    }

                    match fs::read_to_string(&actual_path) {
                        Ok(actual_contents) => {
                            contents.assert_eq(&actual_contents);
                        }
                        Err(err) => {
                            problems.push(format!(
                                "Failed to read contents in worktree {expected_path}: {path}: {err}"
                            ));
                        }
                    }
                }
            }
        }
    }

    #[track_caller]
    fn check_status<C>(problems: &mut Vec<String>, git: &Git<C>, expected: &Self, actual: &Worktree)
    where
        C: AsRef<Utf8Path>,
    {
        let expected_path = &expected.path;

        let expected_status = match &expected.status {
            Some(expected_status) => expected_status,
            None => {
                return;
            }
        };

        match git.status().get() {
            Ok(actual_status) => {
                let sorted_entries = |status: &Status| -> Vec<String> {
                    status
                        .entries
                        .iter()
                        .map(|entry| entry.to_string())
                        .sorted()
                        .collect()
                };

                let actual_entries = sorted_entries(&actual_status);
                let expected_entries = sorted_entries(expected_status);

                if actual_entries != expected_entries {
                    problems.push(format!(
                        "Git status differs in {expected_path}:\n{}",
                        Comparison::new(&actual_entries, &expected_entries)
                    ));
                }
            }
            Err(err) => {
                problems.push(format!(
                    "Failed to get Git status in {}: {err}",
                    actual.path
                ));
            }
        };
    }
}

impl Display for WorktreeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)?;

        match &self.head {
            Some(head) => write!(f, " [{head}]")?,
            None => {}
        }

        Ok(())
    }
}

#[derive(Debug)]
enum WorktreeHeadState {
    Bare,
    Detached(String),
    Branch(Option<String>, LocalBranchRef),
}

impl Display for WorktreeHeadState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Bare => write!(f, "bare"),
            Self::Detached(commit) => write!(f, "detached at {commit}"),
            Self::Branch(commit, branch) => match commit {
                Some(commit) => write!(f, "{branch} at {commit}"),
                None => write!(f, "{branch}"),
            },
        }
    }
}
