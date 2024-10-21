# git-prole

<a href="https://crates.io/crates/git-prole">
<img src="https://img.shields.io/crates/v/git-prole" alt="Crates.io">
</a>
<br>
<a href="https://repology.org/project/git-prole/versions">
<img src="https://repology.org/badge/vertical-allrepos/git-prole.svg?header=" alt="Packaging status">
</a>
<br>
<a href="https://9999years.github.io/git-prole/">
<img src="https://img.shields.io/badge/User%20manual-9999years.github.io%2Fgit--prole-blue" alt="User manual">
</a>
<br>

A [`git-worktree(1)`][git-worktree] manager.

[git-worktree]: https://git-scm.com/docs/git-worktree

A normal Git checkout looks like this:

```
my-repo/
  .git/
  README.md
  ...
```

Worktrees allow you to associate multiple checkouts with one `.git` directory,
like this:

```
my-repo/
  .git/      # A bare repository
  main/      # A checkout for the main branch
    README.md
  feature1/  # A checkout for work on a feature
    README.md
  ...
```

This makes it a lot easier to keep a handful of branches 'in flight' at the
same time, and it's often handy to be able to compare your work against a local
checkout of the main branch without switching branches.

Unfortunately, the built-in `git worktree` commands don't make it very easy to
set up repositories with this layout. `git-prole` exists to paper over these
deficiencies.

## Features

* Clone a repository into a worktree checkout with `git prole clone URL
  [DESTINATION]`.

* Convert an existing repository into a worktree checkout with `git prole
  convert`.

* Add a new worktree with `git prole add`.

  * `git prole add feature1` will create a `feature1` directory next to the
    rest of your worktrees; `git worktree add feature1`, in contrast, will
    create a `feature1` subdirectory nested under the current worktree.

  * Branches created with `git prole add` will start at and track the
    repository's main branch by default.

  * `git prole add` will copy untracked files to the new worktree by default,
    making it easy to start a new worktree with a warm build cache.

  * `git prole add` can run commands when a new worktree is created, so that
    you can warm up caches by running a command like `direnv allow`.

  * `git prole add` can perform regex substitutions on branch names to compute
    a directory name, so that you can run `git prole add -b
    myname/team-1234-my-ticket-with-a-very-long-title` and get a directory name
    like `my-ticket`.

  * `git prole add` respects the `-c`/`--create` option (to match `git
    switch`); `git worktree add` only allows `-b` (with no long-form option
    available).
