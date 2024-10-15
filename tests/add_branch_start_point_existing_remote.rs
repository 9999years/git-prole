use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn add_branch_start_point_existing_remote() {
    let prole = GitProle::new().unwrap();
    prole.setup_repo("my-remote/my-repo").unwrap();
    // Set up a `puppy` branch in the remote.
    prole
        .sh("
        cd my-remote/my-repo || exit
        git switch -c puppy
        echo 'softy pup' > README.md
        git commit -am 'cooler readme'
        git switch main
        ")
        .unwrap();

    prole
        .cmd()
        .args(["clone", "my-remote/my-repo"])
        .status_checked()
        .unwrap();

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "-b", "softie", "doggy", "puppy"])
        .status_checked()
        .unwrap();

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("main"),
            WorktreeState::new("doggy")
                .branch("softie")
                .upstream("origin/puppy")
                .file(
                    "README.md",
                    expect![[r#"
                        softy pup
                    "#]],
                ),
        ])
        .assert();
}
