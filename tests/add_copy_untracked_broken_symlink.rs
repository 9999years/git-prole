use command_error::CommandExt;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn add_copy_untracked_broken_symlink() -> miette::Result<()> {
    let prole = GitProle::new()?;

    prole.setup_worktree_repo("my-repo")?;

    prole.sh("
        cd my-repo/main || exit
        ln -s does-not-exist my-cool-symlink
        mkdir untracked-dir
        ln -s does-not-exist untracked-dir/my-cooler-symlink
        ln -s untracked-dir symlink-to-directory
        ")?;

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "puppy"])
        .status_checked()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main").status([
                "?? my-cool-symlink",
                "?? symlink-to-directory",
                "?? untracked-dir/",
            ]),
            // Untracked files are not copied!
            WorktreeState::new("puppy")
                .branch("puppy")
                .upstream("main")
                .status([]),
        ])
        .assert();

    let link = prole.path("my-repo/puppy/my-cool-symlink");
    assert!(!link.exists());

    let link = prole.path("my-repo/puppy/symlink-to-directory");
    assert!(!link.exists());

    let link = prole.path("my-repo/puppy/untracked-dir/my-cooler-symlink");
    assert!(!link.exists());

    Ok(())
}
