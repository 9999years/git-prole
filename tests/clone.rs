use expect_test::expect;
use test_harness::GitProle;

#[test]
fn test_clone() {
    let prole = GitProle::new().unwrap();
    prole.setup_repo("remote/my-repo").unwrap();
    prole.output(["clone", "remote/my-repo"]).unwrap();

    prole.assert_exists(&["my-repo", "my-repo/.git", "my-repo/master"]);

    assert_eq!(
        prole
            .git("my-repo/.git")
            .config()
            .get("core.bare")
            .unwrap()
            .unwrap(),
        "true"
    );

    prole.assert_contents(&[("my-repo/master/README.md", expect!["puppy doggyy"])]);
}
