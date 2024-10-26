use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn config_add_copy_ignored() -> miette::Result<()> {
    let prole = GitProle::new()?;

    prole.setup_worktree_repo("my-repo")?;

    prole.write_config(
        "
        [add]
        copy_ignored = false
        ",
    )?;

    prole.sh("
        cd my-repo/main || exit
        echo 'compiled-*' >> .gitignore
        git add .gitignore
        git commit -m 'Add .gitignore'

        echo 'puppy doggy' > compiled-animal-facts.txt
        echo 'puppy doggy' > animal-facts.txt
        ")?;

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "puppy"])
        .status_checked()?;

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
                .status(["?? animal-facts.txt", "!! compiled-animal-facts.txt"]),
            WorktreeState::new("puppy")
                .branch("puppy")
                .upstream("main")
                // The untracked file is not copied to the new worktree.
                .no_file("animal-facts.txt")
                // The ignored file is not copied to the new worktree.
                .no_file("compiled-animal-facts.txt")
                .status([]),
        ])
        .assert();

    Ok(())
}
