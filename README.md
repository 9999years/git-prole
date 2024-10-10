# git-prole

[![Crates.io](https://img.shields.io/crates/v/git-prole)](https://crates.io/crates/git-prole)

A [`git-worktree(1)`][git-worktree] manager.

[git-worktree]: https://git-scm.com/docs/git-worktree/en

## Features

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
