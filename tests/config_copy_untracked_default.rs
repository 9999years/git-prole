use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn config_copy_untracked_default() -> miette::Result<()> {
    let prole = GitProle::new()?;

    prole.setup_worktree_repo("my-repo")?;

    prole.sh("
        cd my-repo/main || exit
        echo 'puppy doggy' > animal-facts.txt
        ")?;

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "puppy"])
        .status_checked()?;

    // The untracked file is copied to the new worktree.

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main")
                .branch("main")
                .file(
                    "animal-facts.txt",
                    expect![[r#"
                        puppy doggy
                    "#]],
                )
                .status(["?? animal-facts.txt"]),
            WorktreeState::new("puppy")
                .branch("puppy")
                .upstream("main")
                // The untracked file is copied to the new worktree.
                .file(
                    "animal-facts.txt",
                    expect![[r#"
                        puppy doggy
                    "#]],
                )
                .status(["?? animal-facts.txt"]),
        ])
        .assert();

    Ok(())
}
