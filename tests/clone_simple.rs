use command_error::CommandExt;
use expect_test::expect;
use test_harness::GitProle;

#[test]
fn clone_simple() {
    let prole = GitProle::new().unwrap();
    prole.setup_repo("remote/my-repo").unwrap();
    prole
        .cmd()
        .args(["clone", "remote/my-repo"])
        .status_checked()
        .unwrap();

    prole.assert_exists(&[
        "my-repo",
        "my-repo/.git",
        "my-repo/main",
        "my-repo/main/README.md",
    ]);

    assert_eq!(
        prole
            .git("my-repo/.git")
            .config()
            .get("core.bare")
            .unwrap()
            .unwrap(),
        "true"
    );

    prole.assert_contents(&[(
        "my-repo/main/README.md",
        expect![[r#"
            puppy doggy
        "#]],
    )]);
}
