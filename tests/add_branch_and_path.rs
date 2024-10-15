use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn add_branch_and_path() {
    let prole = GitProle::new().unwrap();
    prole.setup_worktree_repo("my-repo").unwrap();

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "-c", "doggy", "../puppy"])
        .status_checked()
        .unwrap();

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("puppy")
                .branch("doggy")
                .upstream("main")
                .file(
                    "README.md",
                    expect![[r#"
                        puppy doggy
                    "#]],
                ),
        ])
        .assert();
}
