use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn add_by_name_new_local() {
    let prole = GitProle::new().unwrap();
    prole.setup_worktree_repo("my-repo").unwrap();

    // Create a new branch and commit to show that when `git prole add` creates a branch, it's
    // based on the default branch by default.
    prole
        .sh("
        cd my-repo/main || exit
        git switch -c doggy
        echo 'cutie puppy' > README.md
        git commit -am 'Cooler README'
        ")
        .unwrap();

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "puppy"])
        .status_checked()
        .unwrap();

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("doggy"),
            WorktreeState::new("puppy")
                .branch("puppy")
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
