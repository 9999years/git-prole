# git-prole

[![Crates.io](https://img.shields.io/crates/v/git-prole)](https://crates.io/crates/git-prole)

A [`git-worktree(1)`][git-worktree] manager.

[git-worktree]: https://git-scm.com/docs/git-worktree/en

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

  * `git prole add` respects the `-c`/`--create` option (to match `git
    switch`); `git worktree add` only allows `-b` (with no long-form option
    available).
