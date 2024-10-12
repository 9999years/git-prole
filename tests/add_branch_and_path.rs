use command_error::CommandExt;
use expect_test::expect;
use pretty_assertions::assert_eq;
use test_harness::GitProle;

#[test]
fn add_branch_and_path() {
    let prole = GitProle::new().unwrap();
    prole.setup_worktree_repo("my-repo").unwrap();

    prole
        .cd_cmd("my-repo/main")
        .args(["add", "-c", "doggy", "../puppy"])
        .status_checked()
        .unwrap();

    prole.assert_contents(&[(
        "my-repo/puppy/README.md",
        expect![[r#"
            puppy doggy
        "#]],
    )]);

    assert_eq!(prole.current_branch_in("my-repo/puppy").unwrap(), "doggy");

    assert_eq!(
        prole
            .upstream_for_branch_in("my-repo/puppy", "doggy")
            .unwrap(),
        "main"
    );
}
