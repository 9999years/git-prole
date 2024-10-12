use command_error::CommandExt;
use expect_test::expect;
use pretty_assertions::assert_eq;
use test_harness::GitProle;

#[test]
fn add_by_name_existing_remote() {
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
        .args(["add", "puppy"])
        .status_checked()
        .unwrap();

    // We get a checkout for the remote-tracking branch!
    prole.assert_contents(&[(
        "my-repo/puppy/README.md",
        expect![[r#"
            softy pup
        "#]],
    )]);

    assert_eq!(prole.current_branch_in("my-repo/puppy").unwrap(), "puppy");

    // We're tracking the remote branch we expect.
    assert_eq!(
        prole
            .upstream_for_branch_in("my-repo/puppy", "puppy")
            .unwrap(),
        "origin/puppy"
    );
}
