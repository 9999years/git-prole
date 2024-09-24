# git-prole

[![Crates.io](https://img.shields.io/crates/v/git-prole)](https://crates.io/crates/git-prole)

A [`git-worktree(1)`][git-worktree] manager.

[git-worktree]: https://git-scm.com/docs/git-worktree/en

## Features

(This is a TODO list.)

A normal Git checkout looks like this:

```
my-repo/
+ .git/
+ README.md
+ ...
```

A worktree checkout looks like this:

```
my-repo/
+ main/
  + .git/
  + README.md
  + ...
+ my-feature-branch/
  + ...
```

- [ ] Convert a Git checkout to a worktree checkout.
- [ ] Clone a repo into a worktree, using the main branch name from the remote.
- [ ] Add a worktree. The worktree should be associated with the main upstream
  branch, unless another is given (rather than the default of the
  currently-checked-out branch).
  - [ ] Copy over files, like `.envrc` or `.nvim.lua`.
- [ ] Remove a worktree. (This will just be an alias for `git worktree remove`.)
