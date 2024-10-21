use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn add_from_bare_no_worktrees() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_worktree_repo("my-repo")?;

    prole.write_config(
        r#"
        branch_names = []
        "#,
    )?;

    prole.sh(r#"
        cd my-repo/main || exit
        git switch -c puppy
        git branch -D main
        git worktree remove .
        cd ../.git || exit
        # `git checkout` and `git switch` don't work in a bare repository.
        # See: https://stackoverflow.com/a/3302018
        git symbolic-ref HEAD refs/heads/puppy
    "#)?;

    // We can add a worktree from the container directory (outside of any working tree but
    // "within" the repo as far as Git is concerned).
    prole
        .cd_cmd("my-repo")
        .args(["add", "doggy", "HEAD"])
        .status_checked()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("doggy").branch("doggy"),
        ])
        .assert();

    Ok(())
}
