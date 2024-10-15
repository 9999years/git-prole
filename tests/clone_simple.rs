use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn clone_simple() {
    let prole = GitProle::new().unwrap();
    prole.setup_repo("remote/my-repo").unwrap();
    prole
        .cmd()
        .args(["clone", "remote/my-repo"])
        .status_checked()
        .unwrap();

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
}
