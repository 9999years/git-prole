use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn clone_simple() -> miette::Result<()> {
    let prole = GitProle::new()?;
    prole.setup_repo("remote/my-repo")?;
    prole
        .cmd()
        .args(["clone", "remote/my-repo"])
        .status_checked()
        .unwrap();

    prole.sh("ls -la && ls -la my-repo")?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main").file(
                "README.md",
                expect![[r#"
                        puppy doggy
                    "#]],
            ),
        ])
        .assert();

    Ok(())
}
