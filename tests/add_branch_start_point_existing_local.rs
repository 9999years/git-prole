use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn add_branch_start_point_existing_local() {
    let prole = GitProle::new().unwrap();
    prole.setup_worktree_repo("my-repo").unwrap();

    // Create a new branch and commit to base our new worktree off of.
    prole
        .sh("
        cd my-repo/main || exit
        git switch -c doggy
        echo 'cutie puppy' > README.md
        git commit -am 'Cooler README'
        git switch main
        ")
        .unwrap();

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "-b", "softy", "puppy", "doggy"])
        .status_checked()
        .unwrap();

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("puppy")
                .branch("softy")
                .upstream("doggy")
                .file(
                    "README.md",
                    expect![[r#"
                        cutie puppy
                    "#]],
                ),
        ])
        .assert();
}
