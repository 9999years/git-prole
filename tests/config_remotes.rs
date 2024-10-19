use command_error::CommandExt;
use expect_test::expect;
use test_harness::setup_repo_multiple_remotes;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn config_remotes() -> miette::Result<()> {
    let prole = GitProle::new()?;

    setup_repo_multiple_remotes(&prole, "my-remotes/my-repo", "my-repo")?;

    prole.write_config(
        r#"
        remotes = [
            "a"
        ]
        "#,
    )?;

    prole.cd_cmd("my-repo").arg("convert").status_checked()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main")
                .branch("main")
                .upstream("origin/main"),
            WorktreeState::new("a").branch("a").upstream("a/a").file(
                "README.md",
                expect![[r#"
                    I am on branch a
                "#]],
            ),
        ])
        .assert();

    Ok(())
}
