# Default branch/remote

Several parts of `git-prole` rely on a concept of a "default branch" or
"default remote":

- When using `git prole convert` to convert an existing repository into a
  worktree checkout, a worktree is created for the default branch.

- When using `git prole add` to create a new worktree, the created branch will
  start at the default branch.

Here's how a default branch is determined:

1. We attempt to find a default remote:

   1. If a remote matching Git's `checkout.defaultRemote` setting is found, we
      use that.

   2. Otherwise, we attempt to find a default remote by matching against the
      `remote_names` configuration setting, which defaults to `upstream` and
      `origin`.

2. If we find a default remote, we use `git ls-remote --symref "$REMOTE" HEAD`
   to determine the default branch for that remote.

3. If no default remote is found, we attempt to find a default local branch by
   matching against the `branch_names` configuration setting, which defaults to
  `main`, `master`, and `trunk`.
