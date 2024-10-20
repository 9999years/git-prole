use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn add_detached_start_point() -> miette::Result<()> {
    let prole = GitProle::new().unwrap();
    prole.setup_worktree_repo("my-repo").unwrap();

    prole.sh(r#"
        cd my-repo/main
        git switch -c silly
        echo "silly!!" > README.md
        git commit -am "Better(?) readme"
        git switch main
    "#)?;

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "--detached", "puppy", "silly"])
        .status_checked()
        .unwrap();

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("puppy").detached("c195bb76").file(
                "README.md",
                expect![[r#"
                silly!!
            "#]],
            ),
        ])
        .assert();

    Ok(())
}
