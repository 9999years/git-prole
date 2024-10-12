use command_error::CommandExt;
use expect_test::expect;
use pretty_assertions::assert_eq;
use test_harness::GitProle;

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

    prole.assert_contents(&[(
        "my-repo/puppy/README.md",
        expect![[r#"
            puppy doggy
        "#]],
    )]);

    assert_eq!(prole.current_branch_in("my-repo/puppy").unwrap(), "puppy");

    assert_eq!(
        prole
            .upstream_for_branch_in("my-repo/puppy", "puppy")
            .unwrap(),
        "main"
    );
}
