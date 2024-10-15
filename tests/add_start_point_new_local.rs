use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;
use test_harness::WorktreeState;

#[test]
fn add_start_point_new_local() {
    let prole = GitProle::new().unwrap();
    prole.setup_worktree_repo("my-repo").unwrap();

    prole
        .sh("
        cd my-repo/main || exit
        git switch -c puppy
        echo 'soft cutie' > README.md
        git commit -am 'Cooler readme'
        ")
        .unwrap();

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "doggy", "@"])
        .status_checked()
        .unwrap();

    prole
        .repo_state("my-repo")
        .worktrees([
            WorktreeState::new_bare(),
            WorktreeState::new("main").branch("puppy"),
            WorktreeState::new("doggy")
                .branch("doggy")
                // Should it have an upstream though...?
                .no_upstream()
                .file(
                    "README.md",
                    expect![[r#"
                        soft cutie
                    "#]],
                ),
        ])
        .assert();
}
