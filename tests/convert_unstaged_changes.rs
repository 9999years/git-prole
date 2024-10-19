use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn convert_unstaged_changes() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("my-repo")?;

    prole.sh("
        cd my-repo
        git switch -c puppy
        echo 'softie cutie' > README.md
        ")?;

    prole
        .repo_state("my-repo")
        .worktrees([WorktreeState::new("")
            .is_main(true)
            .status([" M README.md"])])
        .assert();

    prole.cd_cmd("my-repo").arg("convert").status_checked()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main")
                .branch("main")
                .commit("4023d080")
                .file(
                    "README.md",
                    expect![[r#"
                        puppy doggy
                    "#]],
                )
                .status([]),
            WorktreeState::new("puppy")
                .branch("puppy")
                .commit("4023d080")
                .file(
                    "README.md",
                    expect![[r#"
                        softie cutie
                    "#]],
                )
                .status([" M README.md"]),
        ])
        .assert();

    Ok(())
}
