use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn add_branch_force() {
    let prole = GitProle::new().unwrap();
    prole.setup_worktree_repo("my-repo").unwrap();

    prole
        .sh("
        cd my-repo/main || exit
        git switch -c puppy
        echo 'softy pup' > README.md
        git commit -am 'cooler readme'
        git switch main
        ")
        .unwrap();

    // `-b` fails; the branch already exists.
    prole
        .cd_cmd("my-repo/main")
        .args(["add", "-b", "puppy"])
        .status_checked()
        .unwrap_err();

    // -B works though!
    prole
        .cd_cmd("my-repo/main")
        .args(["add", "-B", "puppy"])
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
            WorktreeState::new("puppy")
                .branch("puppy")
                .no_upstream()
                .file(
                    "README.md",
                    // Branch is reset, so we don't see the updated readme.
                    expect![[r#"
                        puppy doggy
                    "#]],
                ),
        ])
        .assert();
}
