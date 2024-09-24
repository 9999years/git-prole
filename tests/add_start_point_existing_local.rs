use command_error::CommandExt;
use expect_test::expect;
use pretty_assertions::assert_eq;
use test_harness::GitProle;

#[test]
fn add_start_point_existing_local() {
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
        .args(["add", "puppy", "doggy"])
        .status_checked()
        .unwrap();

    prole.assert_contents(&[(
        "my-repo/puppy/README.md",
        expect![[r#"
            cutie puppy
        "#]],
    )]);

    assert_eq!(prole.current_branch_in("my-repo/puppy").unwrap(), "doggy");
}
