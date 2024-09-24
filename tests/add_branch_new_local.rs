use command_error::CommandExt;
use expect_test::expect;
use pretty_assertions::assert_eq;
use test_harness::GitProle;

#[test]
fn add_branch_new_local() {
    let prole = GitProle::new().unwrap();
    prole.setup_worktree_repo("my-repo").unwrap();
    prole
        .cd_cmd("my-repo/main")
        .args(["add", "-b", "puppy"])
        .status_checked()
        .unwrap();

    prole.assert_contents(&[(
        "my-repo/puppy/README.md",
        expect![[r#"
            puppy doggy
        "#]],
    )]);

    assert_eq!(prole.current_branch_in("my-repo/puppy").unwrap(), "puppy");
}
