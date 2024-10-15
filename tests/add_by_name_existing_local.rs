use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn add_by_name_existing_local() {
    let prole = GitProle::new().unwrap();
    prole.setup_worktree_repo("my-repo").unwrap();

    // Set up an existing `puppy` branch.
    prole
        .sh("
        cd my-repo/main || exit
        git switch -c puppy
        echo 'softy pup' > README.md
        git commit -am 'cooler readme'
        git switch main
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
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("puppy")
                .branch("puppy")
                .upstream("main")
                .file(
                    "README.md",
                    expect![[r#"
                        softy pup
                    "#]],
                ),
        ])
        .assert();
}
