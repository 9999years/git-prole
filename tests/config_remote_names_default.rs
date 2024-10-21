use command_error::CommandExt;
use expect_test::expect;
use test_harness::setup_repo_multiple_remotes;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn config_remote_names_default() -> miette::Result<()> {
    let prole = GitProle::new()?;
    setup_repo_multiple_remotes(&prole, "my-remotes/my-repo", "my-repo")?;

    prole.sh("
        cd my-repo || exit
        git remote rename a upstream
        ")?;

    // Okay, this leaves us with remotes `origin`, `upstream`, `b`, and `c`.
    //
    // The default config says `upstream` is more important than `origin`, so we use that!

    prole.cd_cmd("my-repo").arg("convert").status_checked()?;

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main")
                .branch("main")
                .upstream("origin/main"),
            WorktreeState::new("a")
                .branch("a")
                .upstream("upstream/a")
                .file(
                    "README.md",
                    expect![[r#"
                        I am on branch a
                    "#]],
                ),
        ])
        .assert();

    Ok(())
}
